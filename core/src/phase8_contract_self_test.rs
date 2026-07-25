//! Finite target probe for the Phase 8 generated numeric contract.

use crate::numeric::NumericStatus;
use crate::phase8_aero::{
    sample_spatial_aerodynamics, small_angle_of_attack_q28, SpatialAeroError,
};
use crate::phase8_format::KWP8_MAX_WIND_KNOTS;
use crate::phase8_numeric::*;
use crate::phase8_pack::{parse_spatial_vehicle_pack, WindKnot, WindProfilePack};
use crate::phase8_world::sample_spatial_wind;
use crate::spatial_numeric::FixedVec3;

mod aero_vectors {
    include!("../../phase8/generated/aero_vectors_v1.rs");
}
mod wind_vectors {
    include!("../../phase8/generated/wind_vectors_v1.rs");
}

fn target_wind() -> WindProfilePack {
    let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
    knots[0] = WindKnot {
        altitude: SpatialPosition::ZERO,
        east: SpatialWind::from_raw(1 << 22),
        north: SpatialWind::from_raw(-2 << 22),
    };
    knots[1] = WindKnot {
        altitude: SpatialPosition::from_raw(1_000 << 13),
        east: SpatialWind::from_raw(5 << 22),
        north: SpatialWind::from_raw(2 << 22),
    };
    WindProfilePack {
        identity: wind_vectors::IDENTITY,
        gust_seed: wind_vectors::GUST_SEED,
        gust_cadence: SpatialTime::from_raw(1 << 18),
        gust_amplitude_east: SpatialWind::from_raw(3 << 22),
        gust_amplitude_north: SpatialWind::from_raw(2 << 22),
        max_gust: SpatialWind::from_raw(4 << 22),
        knot_count: 2,
        knots,
    }
}
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
    for vector in aero_vectors::AOA_VECTORS {
        let angle = small_angle_of_attack_q28(vector.velocity_q19, &mut status)
            .map_or(u32::MAX, |value| value.raw() as u32);
        hash = mix(hash, angle);
    }
    let exceeded = small_angle_of_attack_q28(aero_vectors::AOA_16_VELOCITY_Q19, &mut status);
    hash = mix(
        hash,
        u32::from(exceeded == Err(SpatialAeroError::ModelEnvelopeExceeded)),
    );
    let vehicle =
        parse_spatial_vehicle_pack(include_bytes!("../../phase8/examples/firestorm54.kvp8"));
    if let Ok(vehicle) = vehicle {
        let sample = sample_spatial_aerodynamics(
            &vehicle,
            aero_vectors::MACH_02_Q24,
            vehicle.dry_cg_from_nose.raw(),
            &mut status,
        );
        if let Ok(sample) = sample {
            hash = mix(hash, sample.axial_cd.raw() as u32);
            hash = mix(hash, sample.cp_from_nose.raw() as u32);
            hash = mix(hash, sample.normal_force_slope.raw() as u32);
            hash = mix(hash, sample.static_margin.raw() as u32);
        } else {
            hash = mix(hash, u32::MAX);
        }
    } else {
        hash = mix(hash, u32::MAX);
    }
    let wind = target_wind();
    for vector in wind_vectors::WIND_VECTORS {
        match sample_spatial_wind(
            &wind,
            SpatialPosition::from_raw(vector.altitude_q13),
            SpatialTime::from_raw(vector.time_q18),
            wind_vectors::CASE_SEED,
            &mut status,
        ) {
            Ok(sample) => {
                for value in [
                    sample.mean.x(),
                    sample.mean.y(),
                    sample.gust.x(),
                    sample.gust.y(),
                    sample.total.x(),
                    sample.total.y(),
                ] {
                    hash = mix(hash, value as u32);
                }
            }
            Err(_) => hash = mix(hash, u32::MAX),
        }
    }
    mix(hash, status.bits() as u32)
}

pub fn run_phase8_contract_self_tests() -> u32 {
    let mut failures = u32::from(!hobby_spatial_numeric_contract_is_valid());
    let mut status = NumericStatus::CLEAR;
    for vector in aero_vectors::AOA_VECTORS {
        match small_angle_of_attack_q28(vector.velocity_q19, &mut status) {
            Ok(angle) if angle.raw() == vector.expected_q28 => {}
            _ => failures += 1,
        }
    }
    if small_angle_of_attack_q28(aero_vectors::AOA_16_VELOCITY_Q19, &mut status)
        != Err(SpatialAeroError::ModelEnvelopeExceeded)
    {
        failures += 1;
    }
    let vehicle =
        parse_spatial_vehicle_pack(include_bytes!("../../phase8/examples/firestorm54.kvp8"));
    match vehicle {
        Ok(vehicle) => match sample_spatial_aerodynamics(
            &vehicle,
            aero_vectors::MACH_02_Q24,
            vehicle.dry_cg_from_nose.raw(),
            &mut status,
        ) {
            Ok(sample) if sample.axial_cd.raw() == aero_vectors::CD_02_Q24 => {}
            _ => failures += 1,
        },
        Err(_) => failures += 1,
    }
    let wind = target_wind();
    for vector in wind_vectors::WIND_VECTORS {
        match sample_spatial_wind(
            &wind,
            SpatialPosition::from_raw(vector.altitude_q13),
            SpatialTime::from_raw(vector.time_q18),
            wind_vectors::CASE_SEED,
            &mut status,
        ) {
            Ok(sample)
                if [sample.mean.x(), sample.mean.y(), sample.mean.z()] == vector.mean_q22
                    && [sample.gust.x(), sample.gust.y(), sample.gust.z()] == vector.gust_q22
                    && [sample.total.x(), sample.total.y(), sample.total.z()]
                        == vector.total_q22 => {}
            _ => failures += 1,
        }
    }
    failures + u32::from(!status.is_clear())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_signature_is_frozen() {
        assert_eq!(run_phase8_contract_self_tests(), 0);
        assert_eq!(phase8_contract_signature(), 0xbeeb_d9b1);
    }
}
