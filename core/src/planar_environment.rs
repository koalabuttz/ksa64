//! Generated Phase 2 density and speed-of-sound environment.

use crate::numeric::{interpolate_clamped_integral_q12, NumericFault, NumericStatus};
use crate::phase2_numeric::EARTH_RADIUS_Q12;
use crate::phase2_quantities::{PlanarAltitude, PlanarVelocity, Radius};
use crate::quantities::Density;

mod data {
    include!("../../phase2/generated/environment_v1.rs");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanarEnvironmentSample {
    altitude: PlanarAltitude,
    density: Density,
    sound_speed: PlanarVelocity,
}

impl PlanarEnvironmentSample {
    pub const fn altitude(self) -> PlanarAltitude {
        self.altitude
    }
    pub const fn density(self) -> Density {
        self.density
    }
    pub const fn sound_speed(self) -> PlanarVelocity {
        self.sound_speed
    }
    pub const fn with_density(self, density: Density) -> Self {
        Self { density, ..self }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RotatingEarthEnvironment;

impl RotatingEarthEnvironment {
    pub const fn new() -> Self {
        Self
    }

    pub fn tables_are_valid(self) -> bool {
        if data::TABLE_LENGTH == 0
            || data::ALTITUDE_KNOTS_Q12.len() != data::DENSITY_Q28.len()
            || data::ALTITUDE_KNOTS_Q12.len() != data::SOUND_SPEED_Q24.len()
        {
            return false;
        }
        let mut index = 0;
        while index < data::TABLE_LENGTH {
            if data::DENSITY_Q28[index] < 0 || data::SOUND_SPEED_Q24[index] <= 0 {
                return false;
            }
            if index != 0 {
                let span = data::ALTITUDE_KNOTS_Q12[index] - data::ALTITUDE_KNOTS_Q12[index - 1];
                if span <= 0 || span & 0x0fff != 0 {
                    return false;
                }
            }
            index += 1;
        }
        true
    }

    pub fn sample(self, radius: Radius, status: &mut NumericStatus) -> PlanarEnvironmentSample {
        let altitude = radius.raw() - EARTH_RADIUS_Q12;
        if altitude < -8192 {
            status.record(NumericFault::InvalidInput);
        }
        let density = interpolate_clamped_integral_q12(
            altitude,
            &data::ALTITUDE_KNOTS_Q12,
            &data::DENSITY_Q28,
            status,
        );
        let sound = interpolate_clamped_integral_q12(
            altitude,
            &data::ALTITUDE_KNOTS_Q12,
            &data::SOUND_SPEED_Q24,
            status,
        );
        PlanarEnvironmentSample {
            altitude: PlanarAltitude::from_raw(altitude),
            density: Density::from_raw(density),
            sound_speed: PlanarVelocity::from_raw(sound),
        }
    }
}
