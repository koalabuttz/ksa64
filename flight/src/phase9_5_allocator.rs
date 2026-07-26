//! Deterministic table-driven PriorityResidualV1 control allocation.

// Fixed indices mirror the compiled mixing matrices and keep MOS code generation auditable.
#![allow(clippy::needless_range_loop)]

use crate::phase9_5::{AdvancedFlightComputer, AdvancedFlightEvidence, AirDataSource};
use ksa64_interface::phase9_5::{
    AdvancedAidCell, AdvancedCommandCell, AdvancedFastSensorCell, AdvancedStatusCell,
    ADVANCED_COMMAND_FLAG_HOLD, ADVANCED_COMMAND_SAFE,
};

pub const GROUP_GIMBAL: u8 = 1;
pub const GROUP_CANARD: u8 = 2;
pub const GROUP_RCS: u8 = 3;
pub const AUTHORITY_GIMBAL: u16 = 1;
pub const AUTHORITY_CANARD: u16 = 2;
pub const AUTHORITY_RCS: u16 = 4;
pub const AUTHORITY_HANDOFF: u16 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedAllocatorConfig {
    pub priorities: [u8; 3],
    pub canard_enable_q10: i32,
    pub canard_full_q10: i32,
    pub canard_disable_q10: i32,
    pub reserve_q15: u16,
    pub propellant_wet_q21: i32,
    pub group_authority_q12: [[i32; 3]; 3],
    pub gimbal_mix_q15: [[i16; 2]; 3],
    pub canard_mix_q15: [[i16; 4]; 3],
    pub rcs_mix_q15: [[i16; 12]; 3],
    pub gimbal_limit_turn16: [i16; 2],
    pub canard_limit_turn16: [i16; 4],
    pub rcs_max_quanta: [u8; 12],
    pub has_gimbal: bool,
    pub has_canards: bool,
    pub has_rcs: bool,
}
impl AdvancedAllocatorConfig {
    pub fn is_valid(&self) -> bool {
        self.priorities.iter().all(|p| (1..=3).contains(p))
            && self.priorities[0] != self.priorities[1]
            && self.priorities[0] != self.priorities[2]
            && self.priorities[1] != self.priorities[2]
            && self.canard_disable_q10 >= 0
            && self.canard_disable_q10 < self.canard_enable_q10
            && self.canard_enable_q10 <= self.canard_full_q10
            && self.reserve_q15 <= 32768
            && (!self.has_rcs || self.propellant_wet_q21 > 0)
            && self.group_authority_q12.iter().flatten().all(|v| *v >= 0)
            && (!self.has_gimbal || self.gimbal_limit_turn16.iter().all(|v| *v > 0))
            && (!self.has_canards || self.canard_limit_turn16.iter().all(|v| *v > 0))
            && (!self.has_rcs || self.rcs_max_quanta.iter().all(|v| *v > 0 && *v <= 8))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocatorFeedback {
    pub on_rail: bool,
    pub powered: bool,
    pub recovery: bool,
    pub safe: bool,
    pub air_data_source: AirDataSource,
    pub dynamic_pressure_q10: i32,
    pub propellant_fraction_q15: u16,
    pub supply_valid: bool,
    pub gimbal_healthy_mask: u8,
    pub canard_healthy_mask: u8,
    pub rcs_healthy_mask: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationResult {
    pub gimbal: [i16; 2],
    pub canards: [i16; 4],
    pub rcs_pulse_quanta: [u8; 12],
    pub requested_q12: [i32; 3],
    pub achieved_q12: [i32; 3],
    pub residual_q12: [i32; 3],
    pub authority_state: u16,
    pub saturation_count: u16,
    pub canard_weight_q15: u16,
}
impl AllocationResult {
    pub const ZERO: Self = Self {
        gimbal: [0; 2],
        canards: [0; 4],
        rcs_pulse_quanta: [0; 12],
        requested_q12: [0; 3],
        achieved_q12: [0; 3],
        residual_q12: [0; 3],
        authority_state: 0,
        saturation_count: 0,
        canard_weight_q15: 0,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocatorError {
    InvalidConfig,
}

pub struct PriorityResidualAllocator {
    config: AdvancedAllocatorConfig,
    canard_enabled: bool,
    rcs_fraction_q15: [i32; 12],
}
impl PriorityResidualAllocator {
    pub fn new(config: AdvancedAllocatorConfig) -> Option<Self> {
        config.is_valid().then_some(Self {
            config,
            canard_enabled: false,
            rcs_fraction_q15: [0; 12],
        })
    }
    pub const fn config(&self) -> &AdvancedAllocatorConfig {
        &self.config
    }
    pub fn allocate(
        &mut self,
        demand: [i32; 3],
        feedback: AllocatorFeedback,
    ) -> Result<AllocationResult, AllocatorError> {
        if !self.config.is_valid() {
            return Err(AllocatorError::InvalidConfig);
        }
        let mut out = AllocationResult {
            requested_q12: demand,
            residual_q12: demand,
            ..AllocationResult::ZERO
        };
        if feedback.safe || feedback.on_rail || feedback.recovery {
            self.rcs_fraction_q15 = [0; 12];
            return Ok(out);
        }
        let canard_weight = self.canard_weight(feedback);
        out.canard_weight_q15 = canard_weight;
        let gimbal_available =
            self.config.has_gimbal && feedback.powered && feedback.gimbal_healthy_mask & 3 != 0;
        let canard_available =
            self.config.has_canards && canard_weight != 0 && feedback.canard_healthy_mask & 15 != 0;
        let rcs_available = self.config.has_rcs
            && feedback.supply_valid
            && feedback.propellant_fraction_q15 > self.config.reserve_q15
            && feedback.rcs_healthy_mask & 0x0fff != 0;
        out.authority_state = (if gimbal_available {
            AUTHORITY_GIMBAL
        } else {
            0
        }) | (if canard_available {
            AUTHORITY_CANARD
        } else {
            0
        }) | (if rcs_available { AUTHORITY_RCS } else { 0 });
        let mut used = 0u8;
        for priority in self.config.priorities {
            match priority {
                GROUP_GIMBAL if gimbal_available => {
                    let (a, c, s) = allocate_continuous(
                        out.residual_q12,
                        self.authority(0, 32768),
                        &self.config.gimbal_mix_q15,
                        self.config.gimbal_limit_turn16,
                        feedback.gimbal_healthy_mask,
                    );
                    out.gimbal = c;
                    apply_group(&mut out, a, s);
                    if a != [0; 3] {
                        used = used.saturating_add(1)
                    }
                }
                GROUP_CANARD if canard_available => {
                    let (a, c, s) = allocate_continuous(
                        out.residual_q12,
                        self.authority(1, canard_weight),
                        &self.config.canard_mix_q15,
                        self.config.canard_limit_turn16,
                        feedback.canard_healthy_mask,
                    );
                    out.canards = c;
                    apply_group(&mut out, a, s);
                    if a != [0; 3] {
                        used = used.saturating_add(1)
                    }
                }
                GROUP_RCS if rcs_available => {
                    let (a, c, s) = allocate_rcs(
                        out.residual_q12,
                        self.authority(2, 32768),
                        &self.config.rcs_mix_q15,
                        self.config.rcs_max_quanta,
                        feedback.rcs_healthy_mask,
                        &mut self.rcs_fraction_q15,
                    );
                    out.rcs_pulse_quanta = c;
                    apply_group(&mut out, a, s);
                    if a != [0; 3] {
                        used = used.saturating_add(1)
                    }
                }
                _ => {}
            }
        }
        if used > 1 {
            out.authority_state |= AUTHORITY_HANDOFF
        }
        if out.residual_q12 != [0; 3] {
            out.saturation_count = out.saturation_count.saturating_add(1)
        }
        Ok(out)
    }
    fn authority(&self, group: usize, weight_q15: u16) -> [i32; 3] {
        let mut out = [0; 3];
        for axis in 0..3 {
            out[axis] = scale_q15(
                self.config.group_authority_q12[axis][group],
                i32::from(weight_q15),
            )
        }
        out
    }
    fn canard_weight(&mut self, feedback: AllocatorFeedback) -> u16 {
        if feedback.air_data_source == AirDataSource::Unavailable
            || feedback.dynamic_pressure_q10 <= self.config.canard_disable_q10
        {
            self.canard_enabled = false;
            return 0;
        }
        if !self.canard_enabled && feedback.dynamic_pressure_q10 >= self.config.canard_enable_q10 {
            self.canard_enabled = true
        }
        if !self.canard_enabled || feedback.dynamic_pressure_q10 <= self.config.canard_enable_q10 {
            return 0;
        }
        if feedback.dynamic_pressure_q10 >= self.config.canard_full_q10 {
            return 32768;
        }
        let numerator =
            i64::from(feedback.dynamic_pressure_q10 - self.config.canard_enable_q10) * 32768;
        let denominator = i64::from(self.config.canard_full_q10 - self.config.canard_enable_q10);
        (numerator / denominator).clamp(0, 32768) as u16
    }
}
fn apply_group(out: &mut AllocationResult, achieved: [i32; 3], saturation: u16) {
    for axis in 0..3 {
        out.achieved_q12[axis] = out.achieved_q12[axis].saturating_add(achieved[axis]);
        out.residual_q12[axis] = out.requested_q12[axis].saturating_sub(out.achieved_q12[axis])
    }
    out.saturation_count = out.saturation_count.saturating_add(saturation)
}
fn round_shift(value: i64, shift: u8) -> i64 {
    if value >= 0 {
        (value + (1i64 << (shift - 1))) >> shift
    } else {
        -(((-value) + (1i64 << (shift - 1))) >> shift)
    }
}
fn scale_q15(value: i32, scale: i32) -> i32 {
    round_shift(i64::from(value) * i64::from(scale), 15)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
fn ratio_q15(value: i32, authority: i32) -> i32 {
    if authority <= 0 {
        0
    } else {
        ((i64::from(value) << 15) / i64::from(authority)).clamp(-32768, 32767) as i32
    }
}
fn synth<const N: usize>(
    allocated: [i32; 3],
    authority: [i32; 3],
    mix: &[[i16; N]; 3],
) -> [i32; N] {
    let ratios = [
        ratio_q15(allocated[0], authority[0]),
        ratio_q15(allocated[1], authority[1]),
        ratio_q15(allocated[2], authority[2]),
    ];
    let mut out = [0; N];
    for actuator in 0..N {
        let mut value = 0i64;
        for axis in 0..3 {
            value += i64::from(ratios[axis]) * i64::from(mix[axis][actuator])
        }
        out[actuator] = round_shift(value, 15).clamp(-32768, 32767) as i32
    }
    out
}
fn allocate_continuous<const N: usize>(
    residual: [i32; 3],
    authority: [i32; 3],
    mix: &[[i16; N]; 3],
    limits: [i16; N],
    healthy_mask: u8,
) -> ([i32; 3], [i16; N], u16) {
    let allocated = [
        residual[0].clamp(-authority[0], authority[0]),
        residual[1].clamp(-authority[1], authority[1]),
        residual[2].clamp(-authority[2], authority[2]),
    ];
    let normalized = synth(allocated, authority, mix);
    let mut commands = [0i16; N];
    let mut actual = [0i32; N];
    let mut saturation = 0u16;
    for index in 0..N {
        if healthy_mask & (1u8 << index) == 0 {
            if normalized[index] != 0 {
                saturation = saturation.saturating_add(1)
            }
            continue;
        }
        let physical = scale_q15(i32::from(limits[index]), normalized[index])
            .clamp(-i32::from(limits[index]), i32::from(limits[index]));
        commands[index] = physical as i16;
        actual[index] = ratio_q15(physical, i32::from(limits[index]));
    }
    let achieved = predict_continuous(actual, authority, mix);
    if achieved != allocated {
        saturation = saturation.saturating_add(1)
    }
    (achieved, commands, saturation)
}
fn predict_continuous<const N: usize>(
    actual: [i32; N],
    authority: [i32; 3],
    mix: &[[i16; N]; 3],
) -> [i32; 3] {
    let mut achieved = [0; 3];
    for axis in 0..3 {
        if authority[axis] == 0 {
            continue;
        }
        let mut dot = 0i64;
        let mut norm = 0i64;
        for index in 0..N {
            dot += i64::from(actual[index]) * i64::from(mix[axis][index]);
            norm += i64::from(mix[axis][index]) * i64::from(mix[axis][index])
        }
        if norm != 0 {
            let ratio = ((dot << 15) / norm).clamp(-32768, 32767) as i32;
            achieved[axis] = scale_q15(authority[axis], ratio)
        }
    }
    achieved
}
fn allocate_rcs(
    residual: [i32; 3],
    authority: [i32; 3],
    mix: &[[i16; 12]; 3],
    maximum: [u8; 12],
    healthy_mask: u16,
    fractional_q15: &mut [i32; 12],
) -> ([i32; 3], [u8; 12], u16) {
    let allocated = [
        residual[0].clamp(-authority[0], authority[0]),
        residual[1].clamp(-authority[1], authority[1]),
        residual[2].clamp(-authority[2], authority[2]),
    ];
    let normalized = synth(allocated, authority, mix);
    let mut pulses = [0u8; 12];
    let mut actual = [0i32; 12];
    let mut saturation = 0u16;
    for index in 0..12 {
        if healthy_mask & (1u16 << index) == 0 {
            if normalized[index] > 0 {
                saturation = saturation.saturating_add(1)
            }
            continue;
        }
        let positive = normalized[index].max(0);
        let desired_q15 = positive.saturating_mul(i32::from(maximum[index]));
        let accumulated = fractional_q15[index].saturating_add(desired_q15);
        let q = (accumulated >> 15).clamp(0, i32::from(maximum[index])) as u8;
        fractional_q15[index] = accumulated.saturating_sub(i32::from(q) << 15);
        pulses[index] = q;
        actual[index] = (i32::from(q) * 32768 / i32::from(maximum[index])).min(32767)
    }
    let achieved = predict_rcs(actual, authority, mix);
    if achieved != allocated {
        saturation = saturation.saturating_add(1)
    }
    (achieved, pulses, saturation)
}
fn predict_rcs(actual: [i32; 12], authority: [i32; 3], mix: &[[i16; 12]; 3]) -> [i32; 3] {
    let mut achieved = [0; 3];
    for axis in 0..3 {
        if authority[axis] == 0 {
            continue;
        }
        let mut numerator = 0i64;
        let mut positive = 0i64;
        for index in 0..12 {
            numerator += i64::from(actual[index]) * i64::from(mix[axis][index]);
            if mix[axis][index] > 0 {
                positive += i64::from(mix[axis][index])
            }
        }
        if positive != 0 {
            let ratio = (numerator / positive).clamp(-32768, 32767) as i32;
            achieved[axis] = scale_q15(authority[axis], ratio)
        }
    }
    achieved
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocatedFlightEvidence {
    pub base: AdvancedFlightEvidence,
    pub command: AdvancedCommandCell,
    pub status: Option<AdvancedStatusCell>,
    pub allocation: AllocationResult,
    pub allocator_checksum: u32,
}
pub struct AllocatedAdvancedFlightComputer {
    base: AdvancedFlightComputer,
    allocator: PriorityResidualAllocator,
    last_continuous: ([i16; 2], [i16; 4]),
    allocator_checksum: u32,
}
impl AllocatedAdvancedFlightComputer {
    pub fn new(base: AdvancedFlightComputer, allocator: AdvancedAllocatorConfig) -> Option<Self> {
        Some(Self {
            base,
            allocator: PriorityResidualAllocator::new(allocator)?,
            last_continuous: ([0; 2], [0; 4]),
            allocator_checksum: 0x811c9dc5,
        })
    }
    pub fn tick(
        &mut self,
        fast: Option<AdvancedFastSensorCell>,
        aid: Option<AdvancedAidCell>,
    ) -> AllocatedFlightEvidence {
        let feedback_cell = fast;
        let base = self.base.tick(fast, aid);
        let cell = feedback_cell.unwrap_or(AdvancedFastSensorCell {
            session: 0,
            measurement_epoch: 0,
            production_epoch: 0,
            validity: 0,
            platform_angle: [0; 3],
            angular_rate: [0; 3],
            delta_velocity: [0; 3],
            dynamic_pressure_q10: 0,
            mach_q12: 0,
            gimbal_applied: [0; 2],
            canard_applied: [0; 4],
            valve_open_mask: 0,
            propellant_q21: 0,
            supply_scale_q15: 0,
            vehicle_status: 0,
            actuator_feedback: 0,
            flags: 0,
        });
        let present = base.missing_fast_epochs == 0;
        let feedback = AllocatorFeedback {
            on_rail: cell.vehicle_status & 1 != 0,
            powered: cell.vehicle_status & 2 != 0,
            recovery: cell.vehicle_status & 4 != 0,
            safe: base.local.safe,
            air_data_source: base.air_data.source,
            dynamic_pressure_q10: base.air_data.dynamic_pressure_q10,
            propellant_fraction_q15: reserve_fraction(
                cell.propellant_q21,
                self.allocator.config().propellant_wet_q21,
                present,
            ),
            supply_valid: present
                && cell.validity & ksa64_interface::phase9_5::ADVANCED_VALID_SUPPLY != 0,
            gimbal_healthy_mask: 3,
            canard_healthy_mask: 15,
            rcs_healthy_mask: 0x0fff,
        };
        let allocation = self
            .allocator
            .allocate(base.command.torque_demand_q12, feedback)
            .unwrap_or(AllocationResult::ZERO);
        let mut command = base.command;
        command.gimbal = allocation.gimbal;
        command.canards = allocation.canards;
        command.rcs_pulse_quanta = allocation.rcs_pulse_quanta;
        command.authority_mode = allocation.authority_state as u8;
        if base.missing_fast_epochs > 0 && base.missing_fast_epochs <= 2 {
            command.gimbal = self.last_continuous.0;
            command.canards = self.last_continuous.1;
            command.rcs_pulse_quanta = [0; 12];
            command.flags |= ADVANCED_COMMAND_FLAG_HOLD
        } else if base.missing_fast_epochs >= 3 || base.local.safe {
            command.gimbal = [0; 2];
            command.canards = [0; 4];
            command.rcs_pulse_quanta = [0; 12];
            command.discrete = ADVANCED_COMMAND_SAFE
        } else {
            self.last_continuous = (command.gimbal, command.canards)
        }
        self.allocator_checksum = hash_alloc(self.allocator_checksum, &command, &allocation);
        command.command_checksum = self.allocator_checksum;
        let status = base.status.map(|mut value| {
            value.authority_state = allocation.authority_state;
            value.achieved_torque_q12 = allocation.achieved_q12.map(clamp_i16);
            value.residual_torque_q12 = allocation.residual_q12.map(clamp_i16);
            value.saturation_count = allocation.saturation_count;
            value.actuator_flags = allocation.canard_weight_q15;
            value
        });
        AllocatedFlightEvidence {
            base,
            command,
            status,
            allocation,
            allocator_checksum: self.allocator_checksum,
        }
    }
}
fn reserve_fraction(remaining: i32, wet: i32, present: bool) -> u16 {
    if !present || remaining <= 0 || wet <= 0 {
        0
    } else {
        ((i64::from(remaining) << 15) / i64::from(wet)).clamp(0, 32_768) as u16
    }
}
fn clamp_i16(v: i32) -> i16 {
    v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}
fn hash_alloc(mut h: u32, c: &AdvancedCommandCell, a: &AllocationResult) -> u32 {
    h = h.rotate_left(5).wrapping_add(0x9e3779b9) ^ u32::from(c.source_epoch);
    for v in a.achieved_q12 {
        h = h.rotate_left(5).wrapping_add(0x9e3779b9) ^ v as u32
    }
    for q in c.rcs_pulse_quanta {
        h = h.rotate_left(5).wrapping_add(0x9e3779b9) ^ u32::from(q)
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> AdvancedAllocatorConfig {
        let mut rcs = [[0i16; 12]; 3];
        for axis in 0..3 {
            let at = axis * 4;
            rcs[axis][at] = 32767;
            rcs[axis][at + 1] = 32767;
            rcs[axis][at + 2] = -32767;
            rcs[axis][at + 3] = -32767
        }
        AdvancedAllocatorConfig {
            priorities: [1, 2, 3],
            canard_enable_q10: 300 << 10,
            canard_full_q10: 2000 << 10,
            canard_disable_q10: 200 << 10,
            reserve_q15: 6554,
            propellant_wet_q21: 209_715,
            group_authority_q12: [[0, 1638, 2048], [2048, 2458, 2048], [2048, 2458, 2048]],
            gimbal_mix_q15: [[0, 0], [32767, 0], [0, 32767]],
            canard_mix_q15: [
                [16384, 16384, -16384, -16384],
                [32767, -32767, 0, 0],
                [0, 0, -32767, 32767],
            ],
            rcs_mix_q15: rcs,
            gimbal_limit_turn16: [910; 2],
            canard_limit_turn16: [1820; 4],
            rcs_max_quanta: [8; 12],
            has_gimbal: true,
            has_canards: true,
            has_rcs: true,
        }
    }
    fn feedback() -> AllocatorFeedback {
        AllocatorFeedback {
            on_rail: false,
            powered: true,
            recovery: false,
            safe: false,
            air_data_source: AirDataSource::Pitot,
            dynamic_pressure_q10: 2000 << 10,
            propellant_fraction_q15: 32768,
            supply_valid: true,
            gimbal_healthy_mask: 3,
            canard_healthy_mask: 15,
            rcs_healthy_mask: 0x0fff,
        }
    }
    #[cfg(feature = "fixtures")]
    #[allow(dead_code)]
    mod independent {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../phase9_5/generated/allocator_vectors_v1.rs"
        ));
    }
    #[cfg(feature = "fixtures")]
    fn vector_config(groups: [bool; 3]) -> AdvancedAllocatorConfig {
        let mut c = config();
        c.has_gimbal = groups[0];
        c.has_canards = groups[1];
        c.has_rcs = groups[2];
        c
    }
    #[cfg(feature = "fixtures")]
    #[allow(clippy::too_many_arguments)]
    fn check_vector(
        demand: [i32; 3],
        groups: [bool; 3],
        gimbal: [i16; 2],
        canards: [i16; 4],
        pulses: [u8; 12],
        achieved: [i32; 3],
        residual: [i32; 3],
        saturation: u16,
    ) {
        let mut a = PriorityResidualAllocator::new(vector_config(groups)).unwrap();
        let r = a.allocate(demand, feedback()).unwrap();
        assert_eq!(r.gimbal, gimbal);
        assert_eq!(r.canards, canards);
        assert_eq!(r.rcs_pulse_quanta, pulses);
        assert_eq!(r.achieved_q12, achieved);
        assert_eq!(r.residual_q12, residual);
        assert_eq!(
            r.saturation_count, saturation,
            "demand={demand:?} result={r:?}"
        );
    }
    #[cfg(feature = "fixtures")]
    #[test]
    fn independent_exact_vectors_match() {
        use independent::*;
        check_vector(
            [0, 1000, -500],
            [true, false, false],
            GIMBAL_GIMBAL,
            GIMBAL_CANARDS,
            GIMBAL_PULSES,
            GIMBAL_ACHIEVED,
            GIMBAL_RESIDUAL,
            GIMBAL_SATURATION,
        );
        check_vector(
            [1000, 1000, -1000],
            [false, true, false],
            CANARD_GIMBAL,
            CANARD_CANARDS,
            CANARD_PULSES,
            CANARD_ACHIEVED,
            CANARD_RESIDUAL,
            CANARD_SATURATION,
        );
        check_vector(
            [1000, 1000, -1000],
            [false, false, true],
            RCS_GIMBAL,
            RCS_CANARDS,
            RCS_PULSES,
            RCS_ACHIEVED,
            RCS_RESIDUAL,
            RCS_SATURATION,
        );
        check_vector(
            [4000, 7000, -6500],
            [true, true, true],
            MIXED_GIMBAL,
            MIXED_CANARDS,
            MIXED_PULSES,
            MIXED_ACHIEVED,
            MIXED_RESIDUAL,
            MIXED_SATURATION,
        );
    }
    #[test]
    fn rail_recovery_and_safe_states_inhibit_all_effectors() {
        let mut a = PriorityResidualAllocator::new(config()).unwrap();
        for f in [
            AllocatorFeedback {
                on_rail: true,
                ..feedback()
            },
            AllocatorFeedback {
                recovery: true,
                ..feedback()
            },
            AllocatorFeedback {
                safe: true,
                ..feedback()
            },
        ] {
            let r = a.allocate([1000; 3], f).unwrap();
            assert_eq!(r.gimbal, [0; 2]);
            assert_eq!(r.canards, [0; 4]);
            assert_eq!(r.rcs_pulse_quanta, [0; 12]);
        }
    }
    #[test]
    fn priority_passes_exact_residual_across_available_groups() {
        let mut a = PriorityResidualAllocator::new(config()).unwrap();
        let r = a.allocate([4000, 7000, -6500], feedback()).unwrap();
        for axis in 0..3 {
            assert_eq!(
                r.residual_q12[axis],
                r.requested_q12[axis] - r.achieved_q12[axis]
            )
        }
        assert!(r.authority_state & AUTHORITY_HANDOFF != 0);
        assert_ne!(r.gimbal, [0; 2]);
        assert_ne!(r.canards, [0; 4]);
        assert!(r.rcs_pulse_quanta.iter().any(|q| *q != 0));
    }
    #[test]
    fn mixed_authority_keeps_in_range_residual_below_ten_percent() {
        let mut a = PriorityResidualAllocator::new(config()).unwrap();
        let requested = [2_000, 4_000, -4_000];
        let result = a.allocate(requested, feedback()).unwrap();
        for axis in 0..3 {
            assert!(
                result.residual_q12[axis].unsigned_abs() * 10 <= requested[axis].unsigned_abs(),
                "axis {axis}: requested={} residual={}",
                requested[axis],
                result.residual_q12[axis]
            );
        }
    }

    #[test]
    fn airdata_and_reserve_handoffs_are_deterministic() {
        let mut a = PriorityResidualAllocator::new(config()).unwrap();
        let low = a
            .allocate(
                [1000; 3],
                AllocatorFeedback {
                    dynamic_pressure_q10: 100 << 10,
                    ..feedback()
                },
            )
            .unwrap();
        assert_eq!(low.canards, [0; 4]);
        let no_rcs = a
            .allocate(
                [1000; 3],
                AllocatorFeedback {
                    propellant_fraction_q15: 6554,
                    ..feedback()
                },
            )
            .unwrap();
        assert_eq!(no_rcs.rcs_pulse_quanta, [0; 12]);
        let unavailable = a
            .allocate(
                [1000; 3],
                AllocatorFeedback {
                    air_data_source: AirDataSource::Unavailable,
                    ..feedback()
                },
            )
            .unwrap();
        assert_eq!(unavailable.canards, [0; 4]);
    }
    #[test]
    fn failed_effectors_leave_a_measured_residual() {
        let mut a = PriorityResidualAllocator::new(config()).unwrap();
        let r = a
            .allocate(
                [1500, 0, 0],
                AllocatorFeedback {
                    gimbal_healthy_mask: 0,
                    canard_healthy_mask: 1,
                    rcs_healthy_mask: 0,
                    ..feedback()
                },
            )
            .unwrap();
        assert_ne!(r.residual_q12, [0; 3]);
        assert!(r.saturation_count > 0);
    }
}
