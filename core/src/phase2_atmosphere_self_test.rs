//! Target-executable Phase 2 atmosphere, guidance, and force checks.

use crate::aerodynamics::{evaluate_aerodynamics, AeroConfig, AeroTable};
use crate::guidance::{PitchKnot, PitchProgram};
use crate::numeric::NumericStatus;
use crate::phase2_numeric::EARTH_RADIUS_Q12;
use crate::phase2_quantities::{
    DownrangeAngle, PitchAngle, PlanarVelocity, Radius, ReferenceArea, SpecificAngularMomentum,
};
use crate::planar::{PlanarTruthState, PlanarWorld, StagePhase};
use crate::planar_environment::RotatingEarthEnvironment;
use crate::quantities::{Mass, Time};

pub fn run_phase2_atmosphere_self_tests() -> u8 {
    let mut failures = 0u8;
    let mut status = NumericStatus::CLEAR;
    let environment = RotatingEarthEnvironment::new();
    if !environment.tables_are_valid() {
        failures = failures.saturating_add(1);
    }
    let sea = environment.sample(Radius::from_raw(EARTH_RADIUS_Q12), &mut status);
    if sea.density().raw() != 328_833_434 || sea.sound_speed().raw() != 5_709_186 {
        failures = failures.saturating_add(1);
    }
    let knots = [
        PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
        PitchKnot::new(Time::from_raw(65_536), PitchAngle::from_raw(8192)),
        PitchKnot::new(Time::from_raw(131_072), PitchAngle::PROGRADE),
    ];
    let program = PitchProgram::new(&knots);
    if !program.is_valid(Time::from_raw(8192))
        || program.pitch_at(Time::from_raw(32_768), &mut status).raw() != 4096
    {
        failures = failures.saturating_add(1);
    }
    let truth = PlanarTruthState::new(
        0,
        Time::ZERO,
        Radius::from_raw(EARTH_RADIUS_Q12),
        DownrangeAngle::ZERO,
        PlanarVelocity::ZERO,
        SpecificAngularMomentum::from_raw(48_602_783),
        Mass::from_raw(4096),
        Mass::ZERO,
        0,
        StagePhase::Complete,
    );
    let aero = evaluate_aerodynamics(
        PlanarWorld::simple_earth(Time::from_raw(8192)),
        truth,
        sea,
        AeroConfig::new(
            ReferenceArea::from_raw(65_536),
            AeroTable::new(&[0, 65_536], &[4915, 4915]),
        ),
        &mut status,
    );
    if aero.dynamic_pressure().raw() != 0
        || aero.radial_drag().raw() != 0
        || aero.tangential_drag().raw() != 0
    {
        failures = failures.saturating_add(1);
    }
    if !status.is_clear() {
        failures = failures.saturating_add(1);
    }
    failures
}
