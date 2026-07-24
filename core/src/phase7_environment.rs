//! Checked-in 250 m environment tables for the hobby vertical profile.

use crate::numeric::{add, multiply_scaled, NumericStatus};
use crate::phase7_numeric::{HobbyAcceleration, HobbyAltitude, HobbyDensity, HobbyVelocity};

include!("../../phase7/generated/environment_v1.rs");

const ALTITUDE_STEP_RAW: i32 = HOBBY_ENVIRONMENT_STEP_M << 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyEnvironmentSample {
    pub density: HobbyDensity,
    pub sound_speed: Option<HobbyVelocity>,
    pub gravity: HobbyAcceleration,
}

fn interpolate(values: &[i32], altitude_raw: i32, status: &mut NumericStatus) -> i32 {
    if altitude_raw <= 0 {
        return values[0];
    }
    let index = altitude_raw as usize / ALTITUDE_STEP_RAW as usize;
    if index >= values.len() - 1 {
        return values[values.len() - 1];
    }
    let remainder = altitude_raw - index as i32 * ALTITUDE_STEP_RAW;
    let fraction_q16 = crate::numeric::divide_scaled(remainder, ALTITUDE_STEP_RAW, 16, status);
    let delta = values[index + 1] - values[index];
    add(
        values[index],
        multiply_scaled(delta, fraction_q16, 16, status),
        status,
    )
}

pub fn sample_hobby_environment(
    altitude: HobbyAltitude,
    status: &mut NumericStatus,
) -> HobbyEnvironmentSample {
    let altitude_raw = altitude.raw().max(0);
    let atmosphere_top_raw = HOBBY_ATMOSPHERE_TOP_M << 13;
    let gravity_top_raw = HOBBY_GRAVITY_TOP_M << 13;
    let (density, sound_speed) = if altitude_raw > atmosphere_top_raw {
        (HobbyDensity::ZERO, None)
    } else {
        (
            HobbyDensity::from_raw(interpolate(&HOBBY_DENSITY_Q29, altitude_raw, status)),
            Some(HobbyVelocity::from_raw(interpolate(
                &HOBBY_SOUND_SPEED_Q19,
                altitude_raw,
                status,
            ))),
        )
    };
    let gravity = HobbyAcceleration::from_raw(interpolate(
        &HOBBY_GRAVITY_Q19,
        altitude_raw.min(gravity_top_raw),
        status,
    ));
    HobbyEnvironmentSample {
        density,
        sound_speed,
        gravity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_level_and_table_boundaries_are_frozen() {
        let mut status = NumericStatus::CLEAR;
        let sea = sample_hobby_environment(HobbyAltitude::ZERO, &mut status);
        assert_eq!(sea.density.raw(), 657_666_877);
        assert_eq!(sea.sound_speed.unwrap().raw(), 178_412_054);
        assert_eq!(sea.gravity.raw(), 5_141_509);
        let top = sample_hobby_environment(
            HobbyAltitude::from_raw(HOBBY_ATMOSPHERE_TOP_M << 13),
            &mut status,
        );
        assert!(top.density.raw() > 0);
        assert!(top.sound_speed.is_some());
        let above = sample_hobby_environment(
            HobbyAltitude::from_raw((HOBBY_ATMOSPHERE_TOP_M + 1) << 13),
            &mut status,
        );
        assert_eq!(above.density, HobbyDensity::ZERO);
        assert_eq!(above.sound_speed, None);
        assert!(status.is_clear());
    }

    #[test]
    fn midpoint_interpolation_is_bounded() {
        let mut status = NumericStatus::CLEAR;
        let low = sample_hobby_environment(HobbyAltitude::ZERO, &mut status);
        let midpoint = sample_hobby_environment(HobbyAltitude::from_raw(125 << 13), &mut status);
        let high = sample_hobby_environment(HobbyAltitude::from_raw(250 << 13), &mut status);
        assert!(midpoint.density < low.density);
        assert!(midpoint.density > high.density);
        assert!(status.is_clear());
    }
}
