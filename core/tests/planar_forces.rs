use ksa64_core::aerodynamics::{evaluate_aerodynamics, AeroConfig, AeroTable};
use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_quantities::{
    DownrangeAngle, PitchAngle, PlanarVelocity, Radius, ReferenceArea, SpecificAngularMomentum,
};
use ksa64_core::planar::{PlanarTruthState, PlanarWorld, StagePhase};
use ksa64_core::planar_dynamics::{advance_planar_state, evaluate_planar_forces};
use ksa64_core::planar_environment::RotatingEarthEnvironment;
use ksa64_core::quantities::{Force, Mass, Time};

fn orbital_state() -> PlanarTruthState {
    PlanarTruthState::new(
        0,
        Time::ZERO,
        Radius::from_raw(EARTH_RADIUS_Q12 + 819_200),
        DownrangeAngle::ZERO,
        PlanarVelocity::ZERO,
        SpecificAngularMomentum::from_raw(838_958_125),
        Mass::from_raw(409_600),
        Mass::ZERO,
        0,
        StagePhase::Burning,
    )
}

fn vacuum_aero(
    world: PlanarWorld,
    truth: PlanarTruthState,
    status: &mut NumericStatus,
) -> ksa64_core::aerodynamics::AeroSnapshot {
    let environment = RotatingEarthEnvironment::new().sample(truth.radius(), status);
    evaluate_aerodynamics(
        world,
        truth,
        environment,
        AeroConfig::new(
            ReferenceArea::from_raw(65_536),
            AeroTable::new(&[0, 65_536], &[4_915, 4_915]),
        ),
        status,
    )
}

#[test]
fn radial_and_prograde_pitch_resolve_thrust_to_expected_axes() {
    let world = PlanarWorld::simple_earth(Time::from_raw(8192));
    let truth = orbital_state();
    let mut status = NumericStatus::CLEAR;
    let aero = vacuum_aero(world, truth, &mut status);
    let radial = evaluate_planar_forces(
        world,
        truth,
        Force::from_raw(4096),
        PitchAngle::RADIAL,
        aero,
        &mut status,
    )
    .unwrap();
    assert_eq!(radial.radial_thrust().raw(), 4096);
    assert_eq!(radial.tangential_thrust().raw(), 0);
    let prograde = evaluate_planar_forces(
        world,
        truth,
        Force::from_raw(4096),
        PitchAngle::PROGRADE,
        aero,
        &mut status,
    )
    .unwrap();
    assert_eq!(prograde.radial_thrust().raw(), 0);
    assert_eq!(prograde.tangential_thrust().raw(), 4096);
    assert!(status.is_clear());
}

#[test]
fn prograde_force_increases_specific_angular_momentum() {
    let world = PlanarWorld::simple_earth(Time::from_raw(8192));
    let truth = orbital_state();
    let mut status = NumericStatus::CLEAR;
    let aero = vacuum_aero(world, truth, &mut status);
    let forces = evaluate_planar_forces(
        world,
        truth,
        Force::from_raw(4096),
        PitchAngle::PROGRADE,
        aero,
        &mut status,
    )
    .unwrap();
    let successor = advance_planar_state(world, truth, forces, &mut status).unwrap();
    assert!(successor.specific_angular_momentum().raw() > truth.specific_angular_momentum().raw());
    assert_eq!(successor.step(), 1);
    assert_eq!(successor.time().raw(), 8192);
    assert!(status.is_clear());
}
