//! Forward WGS 84 conversion and local-frame construction for Phase 10.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericStatus};
use crate::phase10_contract::WGS84_SEMI_MAJOR_Q12_KM;
use crate::phase10_environment::{cordic_sin_cos_q30, EnvironmentError, GeodeticState};
use crate::phase10_numeric::GlobalPositionVec;
use crate::spatial_numeric::{FixedVec3, QuaternionQ30};

const HALF_PI_Q28: i32 = 421_657_428;
const PI_Q28: i32 = 843_314_857;
const WGS84_E2_Q30: i32 = 7_188_036;

/// Converts WGS 84 geodetic coordinates to Earth-fixed Cartesian position.
pub fn geodetic_to_ecef(geodetic: GeodeticState) -> Result<GlobalPositionVec, EnvironmentError> {
    if geodetic.latitude_q28_rad.abs() > HALF_PI_Q28 || geodetic.longitude_q28_rad.abs() > PI_Q28 {
        return Err(EnvironmentError::Range);
    }
    let mut status = NumericStatus::CLEAR;
    let (sin_lat, cos_lat) = cordic_sin_cos_q30(geodetic.latitude_q28_rad, &mut status);
    let (sin_lon, cos_lon) = cordic_sin_cos_q30(geodetic.longitude_q28_rad, &mut status);
    let sin2 = multiply_scaled(sin_lat, sin_lat, 30, &mut status);
    let root_term = subtract(
        1 << 30,
        multiply_scaled(WGS84_E2_Q30, sin2, 30, &mut status),
        &mut status,
    );
    let root = crate::numeric::sqrt_floor_scaled_u32(root_term.max(0) as u32, 30, &mut status);
    if root == 0 || root > i32::MAX as u32 {
        return Err(EnvironmentError::Numeric);
    }
    let normal_radius = divide_scaled(WGS84_SEMI_MAJOR_Q12_KM, root as i32, 30, &mut status);
    let radial = add(normal_radius, geodetic.height_q12_km, &mut status);
    let polar_radius = add(
        multiply_scaled(normal_radius, (1 << 30) - WGS84_E2_Q30, 30, &mut status),
        geodetic.height_q12_km,
        &mut status,
    );
    let horizontal = multiply_scaled(radial, cos_lat, 30, &mut status);
    let position = GlobalPositionVec::new(
        multiply_scaled(horizontal, cos_lon, 30, &mut status),
        multiply_scaled(horizontal, sin_lon, 30, &mut status),
        multiply_scaled(polar_radius, sin_lat, 30, &mut status),
    );
    if status.is_clear() {
        Ok(position)
    } else {
        Err(EnvironmentError::Numeric)
    }
}

/// Active rotation from local east/north/up vectors into ECEF.
pub fn enu_to_ecef_rotation(
    latitude_q28_rad: i32,
    longitude_q28_rad: i32,
) -> Result<QuaternionQ30, EnvironmentError> {
    if latitude_q28_rad.abs() > HALF_PI_Q28 || longitude_q28_rad.abs() > PI_Q28 {
        return Err(EnvironmentError::Range);
    }
    let mut status = NumericStatus::CLEAR;
    let (sin_half_lon, cos_half_lon) = cordic_sin_cos_q30(longitude_q28_rad / 2, &mut status);
    let (sin_half_lat, cos_half_lat) = cordic_sin_cos_q30(-latitude_q28_rad / 2, &mut status);
    let qz = QuaternionQ30::new(cos_half_lon, 0, 0, sin_half_lon);
    let qy = QuaternionQ30::new(cos_half_lat, 0, sin_half_lat, 0);
    let base = QuaternionQ30::new(1 << 29, 1 << 29, 1 << 29, 1 << 29);
    let rotation = qz
        .hamilton(qy, &mut status)
        .hamilton(base, &mut status)
        .normalized(&mut status);
    if status.is_clear() {
        Ok(rotation)
    } else {
        Err(EnvironmentError::Numeric)
    }
}

/// Unit launch direction in local ENU for azimuth clockwise from north.
pub fn launch_direction_enu(
    azimuth_q28_rad: i32,
    elevation_q28_rad: i32,
) -> Result<FixedVec3<30>, EnvironmentError> {
    if azimuth_q28_rad.abs() > PI_Q28 || !(0..=HALF_PI_Q28).contains(&elevation_q28_rad) {
        return Err(EnvironmentError::Range);
    }
    let mut status = NumericStatus::CLEAR;
    let (sin_azimuth, cos_azimuth) = cordic_sin_cos_q30(azimuth_q28_rad, &mut status);
    let (sin_elevation, cos_elevation) = cordic_sin_cos_q30(elevation_q28_rad, &mut status);
    let direction = FixedVec3::new(
        multiply_scaled(cos_elevation, sin_azimuth, 30, &mut status),
        multiply_scaled(cos_elevation, cos_azimuth, 30, &mut status),
        sin_elevation,
    );
    if status.is_clear() {
        Ok(direction)
    } else {
        Err(EnvironmentError::Numeric)
    }
}

/// Quaternion rotating body +X onto a unit vector in the current frame.
pub fn body_x_attitude(direction_q30: FixedVec3<30>) -> Result<QuaternionQ30, EnvironmentError> {
    let mut status = NumericStatus::CLEAR;
    let half_sum = (add(1 << 30, direction_q30.x(), &mut status) + 1) >> 1;
    if half_sum <= 0 {
        return Ok(QuaternionQ30::new(0, 0, 0, 1 << 30));
    }
    let w = crate::numeric::sqrt_floor_scaled_u32(half_sum as u32, 30, &mut status);
    if w == 0 || w > i32::MAX as u32 {
        return Err(EnvironmentError::Numeric);
    }
    let denominator = (w as i32).saturating_mul(2);
    let quaternion = QuaternionQ30::new(
        w as i32,
        0,
        divide_scaled(-direction_q30.z(), denominator, 30, &mut status),
        divide_scaled(direction_q30.y(), denominator, 30, &mut status),
    )
    .normalized(&mut status);
    if status.is_clear() {
        Ok(quaternion)
    } else {
        Err(EnvironmentError::Numeric)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase10_contract::{WGS84_SEMI_MAJOR_Q12_KM, WGS84_SEMI_MINOR_Q12_KM};
    use crate::phase10_environment::ecef_to_geodetic;
    use crate::spatial_numeric::FixedVec3;

    #[test]
    fn forward_and_inverse_geodesy_agree_at_difficult_locations() {
        for geodetic in [
            GeodeticState {
                latitude_q28_rad: 0,
                longitude_q28_rad: 0,
                height_q12_km: 0,
            },
            GeodeticState {
                latitude_q28_rad: 133_517_519,
                longitude_q28_rad: -377_650_950,
                height_q12_km: 12,
            },
            GeodeticState {
                latitude_q28_rad: HALF_PI_Q28,
                longitude_q28_rad: 0,
                height_q12_km: 0,
            },
        ] {
            let ecef = geodetic_to_ecef(geodetic).unwrap();
            let round_trip = ecef_to_geodetic(ecef).unwrap();
            assert!((round_trip.latitude_q28_rad - geodetic.latitude_q28_rad).abs() <= 128);
            assert!((round_trip.height_q12_km - geodetic.height_q12_km).abs() <= 8);
        }
        assert_eq!(
            geodetic_to_ecef(GeodeticState {
                latitude_q28_rad: 0,
                longitude_q28_rad: 0,
                height_q12_km: 0,
            })
            .unwrap(),
            GlobalPositionVec::new(WGS84_SEMI_MAJOR_Q12_KM, 0, 0)
        );
        assert!(
            (geodetic_to_ecef(GeodeticState {
                latitude_q28_rad: HALF_PI_Q28,
                longitude_q28_rad: 0,
                height_q12_km: 0,
            })
            .unwrap()
            .z() - WGS84_SEMI_MINOR_Q12_KM)
                .abs()
                <= 8
        );
    }

    #[test]
    fn enu_rotation_maps_up_to_surface_normal_at_equator() {
        let q = enu_to_ecef_rotation(0, 0).unwrap();
        let mut status = NumericStatus::CLEAR;
        let up = q.rotate(FixedVec3::<30>::new(0, 0, 1 << 30), &mut status);
        assert!(status.is_clear());
        assert!((up.x() - (1 << 30)).abs() <= 16);
        assert!(up.y().abs() <= 16);
        assert!(up.z().abs() <= 16);
    }
}
