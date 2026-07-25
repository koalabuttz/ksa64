//! Deterministic 32 Hz pitot and Mach sensor synthesis for Phase 9.5.

use crate::numeric::{add, multiply_scaled, NumericStatus};

const HISTORY: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitotFaultConfig {
    pub dynamic_pressure_bias_q10: i32,
    pub dynamic_pressure_noise_q10: i32,
    pub dynamic_pressure_quantum_q10: i32,
    pub dynamic_pressure_saturation_q10: i32,
    pub mach_bias_q12: i16,
    pub mach_noise_q12: i16,
    pub mach_quantum_q12: i16,
    pub delay_epochs: u8,
    pub dropout_start_epoch: u16,
    pub dropout_epochs: u16,
}
impl PitotFaultConfig {
    pub const NOMINAL: Self = Self {
        dynamic_pressure_bias_q10: 0,
        dynamic_pressure_noise_q10: 0,
        dynamic_pressure_quantum_q10: 1,
        dynamic_pressure_saturation_q10: 20_000 << 10,
        mach_bias_q12: 0,
        mach_noise_q12: 0,
        mach_quantum_q12: 1,
        delay_epochs: 0,
        dropout_start_epoch: 0,
        dropout_epochs: 0,
    };
    pub const fn is_valid(self) -> bool {
        self.dynamic_pressure_noise_q10 >= 0
            && self.dynamic_pressure_quantum_q10 > 0
            && self.dynamic_pressure_saturation_q10 > 0
            && self.mach_noise_q12 >= 0
            && self.mach_quantum_q12 > 0
            && self.delay_epochs < HISTORY as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitotReading {
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub dynamic_pressure_q10: i32,
    pub mach_q12: i16,
    pub valid: bool,
    pub saturated: bool,
}
impl PitotReading {
    const EMPTY: Self = Self {
        measurement_epoch: 0,
        production_epoch: 0,
        dynamic_pressure_q10: 0,
        mach_q12: 0,
        valid: false,
        saturated: false,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitotSensor {
    scenario_identity: u32,
    sensor_identity: u32,
    config: PitotFaultConfig,
    history: [PitotReading; HISTORY],
}
impl PitotSensor {
    pub fn new(
        scenario_identity: u32,
        sensor_identity: u32,
        config: PitotFaultConfig,
    ) -> Option<Self> {
        if scenario_identity == 0 || sensor_identity == 0 || !config.is_valid() {
            return None;
        }
        Some(Self {
            scenario_identity,
            sensor_identity,
            config,
            history: [PitotReading::EMPTY; HISTORY],
        })
    }
    pub fn sample(
        &mut self,
        epoch: u16,
        truth_dynamic_pressure_q13: i32,
        truth_mach_q24: i32,
        status: &mut NumericStatus,
    ) -> PitotReading {
        let key = mix32(
            self.scenario_identity
                ^ self.sensor_identity.rotate_left(11)
                ^ u32::from(epoch).wrapping_mul(0x9e37_79b9),
        );
        let q_noise = multiply_scaled(
            self.config.dynamic_pressure_noise_q10,
            (key & 0xffff) as i32 - 32768,
            15,
            status,
        );
        let m_noise = multiply_scaled(
            i32::from(self.config.mach_noise_q12),
            ((key >> 16) & 0xffff) as i32 - 32768,
            15,
            status,
        );
        let q_truth = truth_dynamic_pressure_q13.saturating_add(4) >> 3;
        let m_truth = truth_mach_q24.saturating_add(1 << 11) >> 12;
        let q_unbounded = add(
            add(q_truth, self.config.dynamic_pressure_bias_q10, status),
            q_noise,
            status,
        );
        let m_unbounded = m_truth
            .saturating_add(i32::from(self.config.mach_bias_q12))
            .saturating_add(m_noise);
        let saturated = q_unbounded > self.config.dynamic_pressure_saturation_q10
            || q_unbounded < 0
            || m_unbounded > i16::MAX as i32
            || m_unbounded < 0;
        let q = quantize(
            q_unbounded.clamp(0, self.config.dynamic_pressure_saturation_q10),
            self.config.dynamic_pressure_quantum_q10,
        );
        let m = quantize(
            m_unbounded.clamp(0, i16::MAX as i32),
            i32::from(self.config.mach_quantum_q12),
        ) as i16;
        let dropout = self.config.dropout_epochs != 0
            && epoch.wrapping_sub(self.config.dropout_start_epoch) < self.config.dropout_epochs;
        self.history[usize::from(epoch) & (HISTORY - 1)] = PitotReading {
            measurement_epoch: epoch,
            production_epoch: epoch,
            dynamic_pressure_q10: q,
            mach_q12: m,
            valid: !dropout && !saturated && status.is_clear(),
            saturated,
        };
        let delay = u16::from(self.config.delay_epochs);
        if epoch < delay {
            return PitotReading {
                production_epoch: epoch,
                ..PitotReading::EMPTY
            };
        }
        let measurement = epoch - delay;
        let mut out = self.history[usize::from(measurement) & (HISTORY - 1)];
        if out.measurement_epoch != measurement {
            return PitotReading {
                production_epoch: epoch,
                ..PitotReading::EMPTY
            };
        }
        out.production_epoch = epoch;
        out
    }
}
fn quantize(value: i32, quantum: i32) -> i32 {
    let half = quantum / 2;
    if value >= 0 {
        value.saturating_add(half) / quantum * quantum
    } else {
        value.saturating_sub(half) / quantum * quantum
    }
}
fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_noise_is_keyed_by_identity_and_epoch() {
        let cfg = PitotFaultConfig {
            dynamic_pressure_noise_q10: 100,
            mach_noise_q12: 20,
            ..PitotFaultConfig::NOMINAL
        };
        let mut a = PitotSensor::new(1, 2, cfg).unwrap();
        let mut b = a;
        for e in 0..64 {
            let mut sa = NumericStatus::CLEAR;
            let mut sb = NumericStatus::CLEAR;
            assert_eq!(
                a.sample(e, 1000 << 13, 1 << 23, &mut sa),
                b.sample(e, 1000 << 13, 1 << 23, &mut sb)
            );
        }
    }
    #[test]
    fn delay_dropout_quantization_and_saturation_are_explicit() {
        let cfg = PitotFaultConfig {
            dynamic_pressure_quantum_q10: 16,
            mach_quantum_q12: 8,
            delay_epochs: 2,
            dropout_start_epoch: 3,
            dropout_epochs: 2,
            dynamic_pressure_saturation_q10: 1500 << 10,
            ..PitotFaultConfig::NOMINAL
        };
        let mut p = PitotSensor::new(3, 4, cfg).unwrap();
        let mut s = NumericStatus::CLEAR;
        assert!(!p.sample(0, 1000 << 13, 1 << 23, &mut s).valid);
        assert!(!p.sample(1, 1000 << 13, 1 << 23, &mut s).valid);
        let r = p.sample(2, 1000 << 13, 1 << 23, &mut s);
        assert!(r.valid);
        assert_eq!(r.measurement_epoch, 0);
        p.sample(3, 1000 << 13, 1 << 23, &mut s);
        p.sample(4, 1000 << 13, 1 << 23, &mut s);
        assert!(!p.sample(5, 1000 << 13, 1 << 23, &mut s).valid);
        p.sample(6, 2000 << 13, 1 << 23, &mut s);
        p.sample(7, 1000 << 13, 1 << 23, &mut s);
        let sat = p.sample(8, 1000 << 13, 1 << 23, &mut s);
        assert_eq!(sat.measurement_epoch, 6);
        assert!(!sat.valid);
        assert!(sat.saturated);
    }
}
