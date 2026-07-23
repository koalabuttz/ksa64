use ksa64_core::aerodynamics::{evaluate_aerodynamics, AeroConfig, AeroTable};
use ksa64_core::guidance::{PitchKnot, PitchProgram};
use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_quantities::{
    DownrangeAngle, PitchAngle, PlanarVelocity, Radius, ReferenceArea, SpecificAngularMomentum,
};
use ksa64_core::planar::{PlanarTruthState, PlanarWorld, StagePhase};
use ksa64_core::planar_environment::RotatingEarthEnvironment;
use ksa64_core::quantities::{Mass, Time};

fn state(radius: i32, radial_velocity: i32) -> PlanarTruthState {
    PlanarTruthState::new(
        0,
        Time::ZERO,
        Radius::from_raw(radius),
        DownrangeAngle::ZERO,
        PlanarVelocity::from_raw(radial_velocity),
        SpecificAngularMomentum::from_raw(48_602_783),
        Mass::from_raw(4096),
        Mass::ZERO,
        0,
        StagePhase::Complete,
    )
}

#[test]
fn rotating_environment_matches_frozen_endpoints() {
    let environment = RotatingEarthEnvironment::new();
    assert!(environment.tables_are_valid());
    let mut status = NumericStatus::CLEAR;
    let sea = environment.sample(Radius::from_raw(EARTH_RADIUS_Q12), &mut status);
    assert_eq!(sea.altitude().raw(), 0);
    assert_eq!(sea.density().raw(), 328_833_434);
    assert_eq!(sea.sound_speed().raw(), 5_709_186);
    let edge = environment.sample(Radius::from_raw(EARTH_RADIUS_Q12 + 491_520), &mut status);
    assert_eq!(edge.density().raw(), 0);
    assert!(status.is_clear());
}

#[test]
fn pitch_program_is_step_aligned_monotonic_and_interpolated() {
    let knots = [
        PitchKnot::new(Time::ZERO, PitchAngle::RADIAL),
        PitchKnot::new(Time::from_raw(65_536), PitchAngle::from_raw(8192)),
        PitchKnot::new(Time::from_raw(131_072), PitchAngle::PROGRADE),
    ];
    let program = PitchProgram::new(&knots);
    assert!(program.is_valid(Time::from_raw(8192)));
    let mut status = NumericStatus::CLEAR;
    assert_eq!(
        program.pitch_at(Time::from_raw(32_768), &mut status),
        PitchAngle::from_raw(4096)
    );
    assert_eq!(
        program.pitch_at(Time::from_raw(98_304), &mut status),
        PitchAngle::from_raw(12_288)
    );
    assert!(status.is_clear());
}

#[test]
fn corotating_surface_has_zero_dynamic_pressure_and_drag() {
    let world = PlanarWorld::simple_earth(Time::from_raw(8192));
    let environment = RotatingEarthEnvironment::new();
    let mut status = NumericStatus::CLEAR;
    let truth = state(EARTH_RADIUS_Q12, 0);
    let sample = environment.sample(truth.radius(), &mut status);
    let mach = [0, 65_536, 327_680];
    let cd = [4915, 8192, 3277];
    let aero = evaluate_aerodynamics(
        world,
        truth,
        sample,
        AeroConfig::new(ReferenceArea::from_raw(65_536), AeroTable::new(&mach, &cd)),
        &mut status,
    );
    assert!(status.is_clear());
    assert!(aero.air_speed().raw().abs() <= 4096);
    assert_eq!(aero.dynamic_pressure().raw(), 0);
    assert_eq!(aero.radial_drag().raw(), 0);
    assert_eq!(aero.tangential_drag().raw(), 0);
}

#[test]
fn drag_opposes_air_relative_motion_and_q_has_physical_scale() {
    let world = PlanarWorld::simple_earth(Time::from_raw(8192));
    let environment = RotatingEarthEnvironment::new();
    let mut status = NumericStatus::CLEAR;
    let truth = state(EARTH_RADIUS_Q12, 1 << 24);
    let sample = environment.sample(truth.radius(), &mut status);
    let mach = [0, 65_536, 327_680];
    let cd = [4915, 8192, 3277];
    let aero = evaluate_aerodynamics(
        world,
        truth,
        sample,
        AeroConfig::new(ReferenceArea::from_raw(65_536), AeroTable::new(&mach, &cd)),
        &mut status,
    );
    assert!(status.is_clear());
    assert!((aero.air_speed().raw() - (1 << 24)).abs() <= 4096);
    assert!((aero.dynamic_pressure().raw() - 40_140_800).abs() <= 1000);
    assert!(aero.radial_drag().raw() < 0);
    assert_eq!(aero.tangential_drag().raw(), 0);
}
