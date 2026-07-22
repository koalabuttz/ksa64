//! Accepted Phase 1 Earth environment and typed lookup results.

use crate::numeric::{interpolate_clamped_integral_q12, NumericStatus};
use crate::quantities::{Acceleration, Altitude, Density};
use crate::scenario::{Scenario, SIMPLE_EARTH_ENVIRONMENT_ID};

mod data {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase1/generated/environment_v1.rs"
    ));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironmentSample {
    density: Density,
    gravity: Acceleration,
}

impl EnvironmentSample {
    pub const fn density(self) -> Density {
        self.density
    }

    pub const fn gravity(self) -> Acceleration {
        self.gravity
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimpleEarthEnvironment;

impl SimpleEarthEnvironment {
    pub const ID: u32 = SIMPLE_EARTH_ENVIRONMENT_ID;

    pub const fn new() -> Self {
        Self
    }

    pub fn from_scenario(scenario: &Scenario) -> Self {
        debug_assert_eq!(scenario.environment_id(), Self::ID);
        Self
    }

    pub fn tables_are_valid(self) -> bool {
        if data::TABLE_LENGTH == 0
            || data::ALTITUDE_KNOTS_Q12.len() != data::DENSITY_Q28.len()
            || data::ALTITUDE_KNOTS_Q12.len() != data::GRAVITY_Q28.len()
            || data::ENVIRONMENT_ID != Self::ID
            || data::ENVIRONMENT_NAME != "earth.simple-atmosphere.v1"
        {
            return false;
        }

        let mut index = 0usize;
        while index < data::TABLE_LENGTH {
            if data::DENSITY_Q28[index] < 0 || data::GRAVITY_Q28[index] <= 0 {
                return false;
            }
            if index != 0 {
                let span = data::ALTITUDE_KNOTS_Q12[index] - data::ALTITUDE_KNOTS_Q12[index - 1];
                if span <= 0 || span & 0x0fff != 0 || (span >> 12) > u16::MAX as i32 {
                    return false;
                }
            }
            index += 1;
        }
        true
    }

    pub fn sample(self, altitude: Altitude, status: &mut NumericStatus) -> EnvironmentSample {
        let density = interpolate_clamped_integral_q12(
            altitude.raw(),
            &data::ALTITUDE_KNOTS_Q12,
            &data::DENSITY_Q28,
            status,
        );
        let gravity = interpolate_clamped_integral_q12(
            altitude.raw(),
            &data::ALTITUDE_KNOTS_Q12,
            &data::GRAVITY_Q28,
            status,
        );
        EnvironmentSample {
            density: Density::from_raw(density),
            gravity: Acceleration::from_raw(gravity),
        }
    }
}
