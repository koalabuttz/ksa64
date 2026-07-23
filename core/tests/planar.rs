use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_quantities::{
    DownrangeAngle, PlanarVelocity, Radius, SpecificAngularMomentum,
};
use ksa64_core::planar::{
    advance_vacuum_midpoint, advance_vacuum_semi_implicit, classify_orbit, evaluate_vacuum,
    OrbitClass, PlanarTruthState, PlanarWorld, StagePhase,
};
use ksa64_core::quantities::{Mass, Time};

const TIMESTEP_Q16: i32 = 8192;
const CIRCULAR_200KM_H_Q14: i32 = 838_958_125;

fn circular_state() -> PlanarTruthState {
    PlanarTruthState::new(
        0,
        Time::ZERO,
        Radius::from_raw(EARTH_RADIUS_Q12 + 819_200),
        DownrangeAngle::ZERO,
        PlanarVelocity::ZERO,
        SpecificAngularMomentum::from_raw(CIRCULAR_200KM_H_Q14),
        Mass::from_raw(4096),
        Mass::ZERO,
        0,
        StagePhase::Complete,
    )
}

#[test]
fn circular_200km_state_classifies_as_stable_and_near_circular() {
    let world = PlanarWorld::simple_earth(Time::from_raw(TIMESTEP_Q16));
    let state = circular_state();
    let mut status = NumericStatus::CLEAR;
    let forces = evaluate_vacuum(world, state, &mut status);
    assert!(status.is_clear());
    assert!((forces.radial_acceleration().raw() as i64).abs() < 512);
    let orbit = classify_orbit(world, state, &mut status).unwrap();
    assert!(status.is_clear());
    assert_eq!(orbit.class(), OrbitClass::StableOrbit);
    assert!(orbit.eccentricity().raw() <= 4);
    assert!((orbit.perigee().raw() - state.radius().raw()).abs() <= 4096);
    assert!((orbit.apogee().raw() - state.radius().raw()).abs() <= 4096);
}

#[test]
fn vacuum_integrators_preserve_angular_momentum_and_circular_radius() {
    let world = PlanarWorld::simple_earth(Time::from_raw(TIMESTEP_Q16));
    let initial = circular_state();
    let mut semi = initial;
    let mut midpoint = initial;
    let mut semi_status = NumericStatus::CLEAR;
    let mut midpoint_status = NumericStatus::CLEAR;
    for _ in 0..42_477 {
        semi = advance_vacuum_semi_implicit(world, semi, &mut semi_status).unwrap();
        midpoint = advance_vacuum_midpoint(world, midpoint, &mut midpoint_status).unwrap();
    }
    assert!(semi_status.is_clear());
    assert!(midpoint_status.is_clear());
    assert_eq!(semi, midpoint);
    assert_eq!(
        semi.specific_angular_momentum(),
        initial.specific_angular_momentum()
    );
    assert!((semi.radius().raw() - initial.radius().raw()).abs() <= 4096);
    assert!(semi.radial_velocity().raw().abs() <= 16_778);
}
#[test]
fn surface_corotation_has_zero_earth_relative_turn_rate_within_quantization() {
    let world = PlanarWorld::simple_earth(Time::from_raw(TIMESTEP_Q16));
    let state = PlanarTruthState::new(
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
    let mut status = NumericStatus::CLEAR;
    let forces = evaluate_vacuum(world, state, &mut status);
    assert!(status.is_clear());
    assert!((forces.tangential_velocity().raw() - 7_803_101).abs() <= 2);
    assert!(forces.earth_relative_turn_rate_q30().abs() <= 2);
}
