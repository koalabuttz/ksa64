//! Phase 8 small-angle aerodynamics and declared-envelope enforcement.

use crate::numeric::{
    add, divide_scaled, magnitude3_floor, multiply_scaled, subtract, NumericStatus,
};
use crate::phase8_numeric::{
    SpatialAngle, SpatialCoefficient, SpatialMomentArm, SpatialStaticMargin,
    SPATIAL_ANGLE_FRACTIONAL_BITS,
};
use crate::phase8_pack::SpatialVehiclePack;

pub const HOBBY_SPATIAL_MAX_MACH_Q24: i32 = 13_421_773;
pub const HOBBY_SPATIAL_MAX_AOA_Q28: i32 = 70_276_238;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpatialAeroError {
    InvalidPack,
    ModelEnvelopeExceeded,
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialAeroSample {
    pub axial_cd: SpatialCoefficient,
    pub cp_from_nose: SpatialMomentArm,
    pub normal_force_slope: SpatialCoefficient,
    pub static_margin: SpatialStaticMargin,
}

fn interpolate(a: i32, b: i32, fraction_q16: i32, status: &mut NumericStatus) -> i32 {
    add(
        a,
        multiply_scaled(subtract(b, a, status), fraction_q16, 16, status),
        status,
    )
}

pub fn sample_spatial_aerodynamics(
    vehicle: &SpatialVehiclePack,
    mach_q24: i32,
    cg_from_nose_q28: i32,
    status: &mut NumericStatus,
) -> Result<SpatialAeroSample, SpatialAeroError> {
    if !vehicle.is_valid() {
        return Err(SpatialAeroError::InvalidPack);
    }
    if !(0..=HOBBY_SPATIAL_MAX_MACH_Q24).contains(&mach_q24) {
        return Err(SpatialAeroError::ModelEnvelopeExceeded);
    }
    let count = vehicle.aero_knot_count as usize;
    let (low, high, fraction_q16) = if mach_q24 <= vehicle.aero_knots[0].mach.raw() {
        (vehicle.aero_knots[0], vehicle.aero_knots[0], 0)
    } else {
        let mut index = 0usize;
        while index + 1 < count && mach_q24 > vehicle.aero_knots[index + 1].mach.raw() {
            index += 1;
        }
        if index + 1 >= count {
            let knot = vehicle.aero_knots[count - 1];
            (knot, knot, 0)
        } else {
            let a = vehicle.aero_knots[index];
            let b = vehicle.aero_knots[index + 1];
            let span = subtract(b.mach.raw(), a.mach.raw(), status);
            let remainder = subtract(mach_q24, a.mach.raw(), status);
            (a, b, divide_scaled(remainder, span, 16, status))
        }
    };
    let axial_cd = interpolate(
        low.axial_cd.raw(),
        high.axial_cd.raw(),
        fraction_q16,
        status,
    );
    let normal_force_slope = interpolate(
        low.normal_force_slope.raw(),
        high.normal_force_slope.raw(),
        fraction_q16,
        status,
    );
    let cp = interpolate(
        low.cp_from_nose.raw(),
        high.cp_from_nose.raw(),
        fraction_q16,
        status,
    );
    let margin = divide_scaled(
        subtract(cp, cg_from_nose_q28, status),
        vehicle.diameter.raw(),
        9,
        status,
    );
    if !status.is_clear() {
        return Err(SpatialAeroError::Numeric);
    }
    Ok(SpatialAeroSample {
        axial_cd: SpatialCoefficient::from_raw(axial_cd),
        cp_from_nose: SpatialMomentArm::from_raw(cp),
        normal_force_slope: SpatialCoefficient::from_raw(normal_force_slope),
        static_margin: SpatialStaticMargin::from_raw(margin),
    })
}

/// Returns the unsigned small angle between body +X and air-relative velocity.
///
/// The validated envelope is narrow enough for `atan(r) = r - r^3/3`; the
/// approximation error stays far below the Q28 quantization and 0.5-degree
/// acceptance budget across 0-15 degrees.
pub fn small_angle_of_attack_q28(
    body_velocity_q19: [i32; 3],
    status: &mut NumericStatus,
) -> Result<SpatialAngle, SpatialAeroError> {
    let lateral = magnitude3_floor(0, body_velocity_q19[1], body_velocity_q19[2], status);
    if body_velocity_q19[0] == 0 && lateral == 0 {
        return Ok(SpatialAngle::ZERO);
    }
    if body_velocity_q19[0] <= 0 || lateral > i32::MAX as u32 {
        return Err(SpatialAeroError::ModelEnvelopeExceeded);
    }
    let ratio_q28 = divide_scaled(lateral as i32, body_velocity_q19[0], 28, status);
    let squared = multiply_scaled(ratio_q28, ratio_q28, SPATIAL_ANGLE_FRACTIONAL_BITS, status);
    let cubed = multiply_scaled(squared, ratio_q28, SPATIAL_ANGLE_FRACTIONAL_BITS, status);
    let angle = subtract(ratio_q28, cubed / 3, status);
    if !status.is_clear() {
        Err(SpatialAeroError::Numeric)
    } else if angle > HOBBY_SPATIAL_MAX_AOA_Q28 {
        Err(SpatialAeroError::ModelEnvelopeExceeded)
    } else {
        Ok(SpatialAngle::from_raw(angle))
    }
}

pub fn enforce_spatial_aero_envelope(
    mach_q24: i32,
    angle_of_attack: SpatialAngle,
) -> Result<(), SpatialAeroError> {
    if !(0..=HOBBY_SPATIAL_MAX_MACH_Q24).contains(&mach_q24)
        || angle_of_attack.raw().unsigned_abs() > HOBBY_SPATIAL_MAX_AOA_Q28 as u32
    {
        Err(SpatialAeroError::ModelEnvelopeExceeded)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase8_pack::parse_spatial_vehicle_pack;

    mod vectors {
        include!("../../phase8/generated/aero_vectors_v1.rs");
    }

    #[test]
    fn fixed_small_angle_vectors_match_independent_values() {
        for vector in vectors::AOA_VECTORS {
            let mut status = NumericStatus::CLEAR;
            let angle = small_angle_of_attack_q28(vector.velocity_q19, &mut status).unwrap();
            assert_eq!(angle.raw(), vector.expected_q28);
            assert!(status.is_clear());
        }
    }

    #[test]
    fn firestorm_table_interpolates_and_reports_stability() {
        let vehicle =
            parse_spatial_vehicle_pack(include_bytes!("../../phase8/examples/firestorm54.kvp8"))
                .unwrap();
        let mut status = NumericStatus::CLEAR;
        let sample = sample_spatial_aerodynamics(
            &vehicle,
            vectors::MACH_02_Q24,
            vehicle.dry_cg_from_nose.raw(),
            &mut status,
        )
        .unwrap();
        assert_eq!(sample.axial_cd.raw(), vectors::CD_02_Q24);
        assert!(sample.static_margin.raw() > 0);
        assert!(status.is_clear());
    }

    #[test]
    fn model_envelope_fails_closed() {
        let mut status = NumericStatus::CLEAR;
        assert_eq!(
            small_angle_of_attack_q28(vectors::AOA_16_VELOCITY_Q19, &mut status),
            Err(SpatialAeroError::ModelEnvelopeExceeded)
        );
        assert_eq!(
            enforce_spatial_aero_envelope(HOBBY_SPATIAL_MAX_MACH_Q24 + 1, SpatialAngle::ZERO),
            Err(SpatialAeroError::ModelEnvelopeExceeded)
        );
    }
}
