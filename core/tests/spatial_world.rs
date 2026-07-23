use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::planar::OrbitClass;
use ksa64_core::spatial_numeric::{ForceVec, PositionVec, QuaternionQ30, VelocityVec};
use ksa64_core::spatial_world::{
    advance_spatial_state, classify_spatial_orbit, evaluate_spatial_aerodynamics,
    evaluate_spatial_environment, position_radius_q12, SpatialAeroConfig, SpatialState,
};

#[allow(dead_code)]
mod vectors {
    include!("../../phase5/generated/spatial_world_tables_v1.rs");
}

fn position(raw: [i32; 3]) -> PositionVec {
    PositionVec::new(raw[0], raw[1], raw[2])
}
fn velocity(raw: [i32; 3]) -> VelocityVec {
    VelocityVec::new(raw[0], raw[1], raw[2])
}

#[test]
fn launch_site_radius_gravity_and_corotation_are_physical() {
    let launch = position(vectors::LAUNCH_POSITION_Q12);
    let mut status = NumericStatus::CLEAR;
    assert!((position_radius_q12(launch, &mut status) - EARTH_RADIUS_Q12).abs() <= 1);
    let stationary = SpatialState::new(launch, VelocityVec::ZERO);
    let first = evaluate_spatial_environment(stationary, &mut status);
    let corotating = SpatialState::new(launch, first.atmosphere_velocity());
    let second = evaluate_spatial_environment(corotating, &mut status);
    assert_eq!(second.air_velocity(), VelocityVec::ZERO);
    assert_eq!(second.air_speed_q24(), 0);
    assert_eq!(second.dynamic_pressure().raw(), 0);
    let gravity = second.gravity();
    let dot = gravity.x() as i64 * launch.x() as i64
        + gravity.y() as i64 * launch.y() as i64
        + gravity.z() as i64 * launch.z() as i64;
    assert!(dot < 0);
    let magnitude = ((gravity.x() as f64).powi(2)
        + (gravity.y() as f64).powi(2)
        + (gravity.z() as f64).powi(2))
    .sqrt()
        / (1u64 << 28) as f64;
    assert!((magnitude - 0.009_798).abs() < 0.000_02);
    assert!(status.is_clear());
}

#[test]
fn circular_200km_state_classifies_with_target_inclination() {
    let state = SpatialState::new(
        position(vectors::CIRCULAR_POSITION_Q12),
        velocity(vectors::CIRCULAR_VELOCITY_Q24),
    );
    let mut status = NumericStatus::CLEAR;
    let orbit = classify_spatial_orbit(state, &mut status).unwrap();
    assert_eq!(orbit.class(), OrbitClass::StableOrbit);
    assert!((orbit.perigee().raw() - 26_944_049).abs() < 8_192);
    assert!((orbit.apogee().raw() - 26_944_049).abs() < 8_192);
    assert!((orbit.inclination_turn16() as i32 - 9_393).abs() <= 5);
    assert!(orbit.eccentricity().raw() < 128);
    assert!(status.is_clear());
}

#[test]
fn spatial_aerodynamics_opposes_axial_and_lateral_air_motion() {
    let surface = PositionVec::new(EARTH_RADIUS_Q12, 0, 0);
    let mut status = NumericStatus::CLEAR;
    let base =
        evaluate_spatial_environment(SpatialState::new(surface, VelocityVec::ZERO), &mut status);
    let atmosphere = base.atmosphere_velocity();
    let config = SpatialAeroConfig::new(28 << 16, 4_915, 32_768, 6 << 16);

    let axial_state = SpatialState::new(surface, VelocityVec::new(5_033_165, atmosphere.y(), 0));
    let axial_env = evaluate_spatial_environment(axial_state, &mut status);
    let axial =
        evaluate_spatial_aerodynamics(QuaternionQ30::IDENTITY, axial_env, config, &mut status);
    assert!(axial.force_eci().x() < 0);
    assert_eq!(
        axial.torque_body(),
        ksa64_core::spatial_numeric::TorqueVec::ZERO
    );
    assert_eq!(axial.angle_of_attack_sine_q16(), 0);

    let lateral_state =
        SpatialState::new(surface, VelocityVec::new(0, atmosphere.y() + 5_033_165, 0));
    let lateral_env = evaluate_spatial_environment(lateral_state, &mut status);
    let lateral =
        evaluate_spatial_aerodynamics(QuaternionQ30::IDENTITY, lateral_env, config, &mut status);
    assert!(lateral.force_eci().y() < 0);
    assert!(lateral.torque_body().z() > 0);
    assert!(lateral.angle_of_attack_sine_q16() > 65_000);
    assert!(status.is_clear());
}

#[test]
fn vacuum_translation_preserves_a_short_circular_arc() {
    let initial = SpatialState::new(
        position(vectors::CIRCULAR_POSITION_Q12),
        velocity(vectors::CIRCULAR_VELOCITY_Q24),
    );
    let mut state = initial;
    let mut status = NumericStatus::CLEAR;
    for _ in 0..128 {
        state = advance_spatial_state(state, ForceVec::ZERO, 100 << 12, 8_192, &mut status);
    }
    let orbit = classify_spatial_orbit(state, &mut status).unwrap();
    assert_eq!(orbit.class(), OrbitClass::StableOrbit);
    let initial_radius = position_radius_q12(initial.position(), &mut status);
    let final_radius = position_radius_q12(state.position(), &mut status);
    assert!((final_radius - initial_radius).abs() < 256);
    assert!((orbit.inclination_turn16() as i32 - 9_393).abs() <= 5);
    assert!(status.is_clear());
}

#[test]
fn exact_world_signature_is_frozen_for_target_comparison() {
    assert_eq!(ksa64_core::phase5_world_signature(), 0xcef8_9def);
}
