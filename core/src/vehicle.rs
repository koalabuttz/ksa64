//! Immutable vertical truth-state construction for validated starts and checked successors.

use crate::quantities::{Acceleration, Altitude, Mass, Time, Velocity};
use crate::scenario::Scenario;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalTruthState {
    step: u32,
    time: Time,
    altitude: Altitude,
    velocity: Velocity,
    acceleration: Acceleration,
    total_mass: Mass,
    propellant: Mass,
}

impl VerticalTruthState {
    pub fn initial(scenario: &Scenario) -> Self {
        Self {
            step: 0,
            time: Time::ZERO,
            altitude: scenario.initial().altitude(),
            velocity: scenario.initial().velocity(),
            acceleration: Acceleration::ZERO,
            total_mass: scenario.initial().total_mass(),
            propellant: scenario.initial().propellant(),
        }
    }

    pub const fn step(self) -> u32 {
        self.step
    }

    pub const fn time(self) -> Time {
        self.time
    }

    pub const fn altitude(self) -> Altitude {
        self.altitude
    }

    pub const fn velocity(self) -> Velocity {
        self.velocity
    }

    pub const fn acceleration(self) -> Acceleration {
        self.acceleration
    }

    pub const fn total_mass(self) -> Mass {
        self.total_mass
    }

    pub const fn propellant(self) -> Mass {
        self.propellant
    }

    pub(crate) const fn successor(
        step: u32,
        time: Time,
        altitude: Altitude,
        velocity: Velocity,
        acceleration: Acceleration,
        total_mass: Mass,
        propellant: Mass,
    ) -> Self {
        Self {
            step,
            time,
            altitude,
            velocity,
            acceleration,
            total_mass,
            propellant,
        }
    }

    #[cfg(any(test, feature = "fixtures"))]
    pub(crate) const fn fixture(
        step: u32,
        time: Time,
        altitude: Altitude,
        velocity: Velocity,
        total_mass: Mass,
        propellant: Mass,
    ) -> Self {
        Self::successor(
            step,
            time,
            altitude,
            velocity,
            Acceleration::ZERO,
            total_mass,
            propellant,
        )
    }
}
