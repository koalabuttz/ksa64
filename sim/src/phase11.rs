//! Phase 11 ground-operations composition.
//!
//! The simulated avionics loop is intentionally absent from this link. A
//! ground blackout can suppress observations and uplinks, but cannot suppress
//! onboard sensors or actuator commands.

use ksa64_interface::phase10::GlobalFrameId;
use ksa64_interface::phase11::{GroundEstimate, GroundTrackingObservation};

pub const GROUND_TRACKING_SOURCE_ID: u32 = 0x11e0_0001;
pub const GROUND_ESTIMATOR_ID: u32 = 0x11e0_1001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundBlackout {
    pub first_epoch: u32,
    pub last_epoch: u32,
}

impl GroundBlackout {
    pub const fn contains(self, epoch: u32) -> bool {
        epoch >= self.first_epoch && epoch <= self.last_epoch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalGroundLink {
    blackout: Option<GroundBlackout>,
}

impl LogicalGroundLink {
    pub const fn new(blackout: Option<GroundBlackout>) -> Self {
        Self { blackout }
    }

    pub const fn available(self, epoch: u32) -> bool {
        match self.blackout {
            Some(window) => !window.contains(epoch),
            None => true,
        }
    }

    pub fn downlink<T: Copy>(self, epoch: u32, value: T) -> Option<T> {
        self.available(epoch).then_some(value)
    }

    pub fn uplink<T: Copy>(self, epoch: u32, value: T) -> Option<T> {
        self.available(epoch).then_some(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundTruthSample {
    pub epoch: u32,
    pub frame: GlobalFrameId,
    pub position_q12_km: [i32; 3],
    pub velocity_q24_km_s: [i32; 3],
}

pub fn synthesize_ground_observation(
    sample: GroundTruthSample,
    delay_releases: u32,
    seed: u32,
) -> GroundTrackingObservation {
    let mut position = sample.position_q12_km;
    let mut velocity = sample.velocity_q24_km_s;
    for axis in 0..3 {
        position[axis] =
            position[axis].saturating_add(keyed_noise(seed, sample.epoch, axis as u32, 8));
        velocity[axis] = velocity[axis].saturating_add(keyed_noise(
            seed ^ 0xa5a5_5a5a,
            sample.epoch,
            axis as u32,
            1_024,
        ));
    }
    let observation_identity = hash_words(&[
        GROUND_TRACKING_SOURCE_ID,
        sample.epoch,
        seed,
        sample.frame as u32,
    ]);
    let observation_checksum = hash_state(observation_identity, &position, &velocity);
    GroundTrackingObservation {
        source_identity: GROUND_TRACKING_SOURCE_ID,
        observation_identity,
        measurement_epoch: sample.epoch,
        receipt_epoch: sample.epoch.saturating_add(delay_releases),
        frame: sample.frame,
        validity: 3,
        position_q12_km: position,
        velocity_q24_km_s: velocity,
        uncertainty_q16: [131_072, 131_072, 196_608],
        observation_checksum,
    }
}

pub struct GroundEstimator {
    estimate: Option<GroundEstimate>,
    checksum: u32,
}

impl GroundEstimator {
    pub const fn new() -> Self {
        Self {
            estimate: None,
            checksum: 0x811c_9dc5,
        }
    }

    pub const fn estimate(&self) -> Option<GroundEstimate> {
        self.estimate
    }

    pub fn update(
        &mut self,
        observation: GroundTrackingObservation,
        production_epoch: u32,
    ) -> Option<GroundEstimate> {
        if observation.source_identity != GROUND_TRACKING_SOURCE_ID
            || observation.validity & 3 != 3
            || observation.receipt_epoch > production_epoch
            || observation.measurement_epoch > observation.receipt_epoch
        {
            return None;
        }
        let (position, velocity, residual) = match self.estimate {
            Some(previous) if previous.frame == observation.frame => {
                let mut position = [0; 3];
                let mut velocity = [0; 3];
                let mut residual = [0; 3];
                for axis in 0..3 {
                    residual[axis] = observation.position_q12_km[axis]
                        .saturating_sub(previous.position_q12_km[axis]);
                    position[axis] =
                        previous.position_q12_km[axis].saturating_add(residual[axis] / 2);
                    velocity[axis] = previous.velocity_q24_km_s[axis].saturating_add(
                        observation.velocity_q24_km_s[axis]
                            .saturating_sub(previous.velocity_q24_km_s[axis])
                            / 2,
                    );
                }
                (position, velocity, residual)
            }
            _ => (
                observation.position_q12_km,
                observation.velocity_q24_km_s,
                [0; 3],
            ),
        };
        self.checksum = hash_state(
            self.checksum ^ observation.observation_checksum,
            &position,
            &velocity,
        );
        let estimate_identity = hash_words(&[
            GROUND_ESTIMATOR_ID,
            observation.observation_identity,
            production_epoch,
            self.checksum,
        ]);
        let estimate = GroundEstimate {
            estimator_identity: GROUND_ESTIMATOR_ID,
            estimate_identity,
            source_observation_identity: observation.observation_identity,
            measurement_epoch: observation.measurement_epoch,
            production_epoch,
            frame: observation.frame,
            flags: u8::from(production_epoch > observation.receipt_epoch),
            position_q12_km: position,
            velocity_q24_km_s: velocity,
            confidence_q16: observation.uncertainty_q16,
            residual_q16: residual,
            estimator_checksum: self.checksum,
        };
        self.estimate = Some(estimate);
        Some(estimate)
    }
}

impl Default for GroundEstimator {
    fn default() -> Self {
        Self::new()
    }
}

fn keyed_noise(seed: u32, epoch: u32, axis: u32, amplitude: i32) -> i32 {
    let value = mix(seed ^ epoch.wrapping_mul(0x9e37_79b9) ^ axis.wrapping_mul(0x85eb_ca6b));
    (value % (amplitude as u32 * 2 + 1)) as i32 - amplitude
}

fn mix(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn hash_words(values: &[u32]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for value in values {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    hash.max(1)
}

fn hash_state(seed: u32, position: &[i32; 3], velocity: &[i32; 3]) -> u32 {
    let mut words = [0u32; 7];
    words[0] = seed;
    for axis in 0..3 {
        words[1 + axis] = position[axis] as u32;
        words[4 + axis] = velocity[axis] as u32;
    }
    hash_words(&words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blackout_affects_only_ground_delivery() {
        let link = LogicalGroundLink::new(Some(GroundBlackout {
            first_epoch: 10,
            last_epoch: 20,
        }));
        assert_eq!(link.downlink(9, 7), Some(7));
        assert_eq!(link.downlink(10, 7), None);
        assert_eq!(link.uplink(20, 9), None);
        assert_eq!(link.uplink(21, 9), Some(9));
    }

    #[test]
    fn tracking_and_estimation_are_repeatable_and_observation_only() {
        let sample = GroundTruthSample {
            epoch: 100,
            frame: GlobalFrameId::EarthInertialEciV1,
            position_q12_km: [26_900_000, 4_000, 8_000],
            velocity_q24_km_s: [0, 120_000_000, 10_000],
        };
        let first = synthesize_ground_observation(sample, 4, 0x4b53_4131);
        let second = synthesize_ground_observation(sample, 4, 0x4b53_4131);
        assert_eq!(first, second);
        let mut estimator = GroundEstimator::new();
        assert_eq!(estimator.update(first, 103), None);
        let estimate = estimator.update(first, 104).unwrap();
        assert_eq!(estimate.position_q12_km, first.position_q12_km);
        assert_eq!(estimate.velocity_q24_km_s, first.velocity_q24_km_s);
    }
}
