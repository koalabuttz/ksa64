//! Finite target probe for the Phase 8 generated numeric contract.

use crate::numeric::NumericStatus;
use crate::phase8_numeric::*;
use crate::spatial_numeric::FixedVec3;

fn mix(mut hash: u32, value: u32) -> u32 {
    hash ^= value;
    hash = hash.wrapping_mul(0x0100_0193);
    hash.rotate_left(5)
}

pub fn phase8_contract_signature() -> u32 {
    let mut hash = 0x811c_9dc5;
    for value in [
        HOBBY_SPATIAL_NUMERIC_CONTRACT_ID,
        HOBBY_SPATIAL_ENVIRONMENT_ID,
        SPATIAL_TIME_FRACTIONAL_BITS as u32,
        SPATIAL_POSITION_FRACTIONAL_BITS as u32,
        SPATIAL_VELOCITY_FRACTIONAL_BITS as u32,
        SPATIAL_ACCELERATION_FRACTIONAL_BITS as u32,
        SPATIAL_MASS_FRACTIONAL_BITS as u32,
        SPATIAL_AREA_FRACTIONAL_BITS as u32,
        SPATIAL_FORCE_FRACTIONAL_BITS as u32,
        SPATIAL_MOMENT_ARM_FRACTIONAL_BITS as u32,
        SPATIAL_INERTIA_FRACTIONAL_BITS as u32,
        SPATIAL_TORQUE_FRACTIONAL_BITS as u32,
        SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS as u32,
        SPATIAL_COEFFICIENT_FRACTIONAL_BITS as u32,
        SPATIAL_STATIC_MARGIN_FRACTIONAL_BITS as u32,
        SPATIAL_ANGLE_FRACTIONAL_BITS as u32,
        SPATIAL_WIND_FRACTIONAL_BITS as u32,
        SPATIAL_QUATERNION_FRACTIONAL_BITS as u32,
        SPATIAL_POWERED_STEP_RAW as u32,
        SPATIAL_COAST_TRANSLATION_STEP_RAW as u32,
        SPATIAL_COAST_ATTITUDE_STEP_RAW as u32,
        SPATIAL_RECOVERY_STEP_RAW as u32,
    ] {
        hash = mix(hash, value);
    }

    let mut status = NumericStatus::CLEAR;
    let a = FixedVec3::<SPATIAL_VELOCITY_FRACTIONAL_BITS>::new(123_456, -234_567, 345_678);
    let b = FixedVec3::<SPATIAL_VELOCITY_FRACTIONAL_BITS>::new(-44_444, 55_555, 66_666);
    let cross = a.cross_scaled::<SPATIAL_VELOCITY_FRACTIONAL_BITS>(b, &mut status);
    hash = mix(hash, cross.x() as u32);
    hash = mix(hash, cross.y() as u32);
    hash = mix(hash, cross.z() as u32);
    mix(hash, status.bits() as u32)
}

pub fn run_phase8_contract_self_tests() -> u32 {
    u32::from(!hobby_spatial_numeric_contract_is_valid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_signature_is_frozen() {
        assert_eq!(run_phase8_contract_self_tests(), 0);
        assert_eq!(phase8_contract_signature(), 0x7455_7844);
    }
}
