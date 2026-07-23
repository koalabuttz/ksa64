use ksa64_flight::navigation::*;
use ksa64_interface::*;

fn frame(sequence: u32) -> SensorFrame {
    SensorFrame {
        sequence,
        onboard_time_q20: (sequence as i32) * 131_072,
        accel_radial_q28: 2_628_000,
        accel_tangential_q28: 0,
        gyro_rate_q24: 0,
        steering_pitch: 0,
        validity: SENSOR_VALID_ACCEL
            | SENSOR_VALID_GYRO
            | SENSOR_VALID_STEERING
            | SENSOR_VALID_CLOCK,
        altitude_q12: 0,
        gps_radius_q12: 0,
        gps_downrange_q32: 0,
        gps_radial_velocity_q24: 0,
        gps_tangential_velocity_q24: 0,
        events: 0,
        active_stage: 0,
        stage_phase: StagePhase::Burning,
        engine_on: true,
    }
}

#[test]
fn navigation_rejects_missing_inertial_and_sequence_gaps() {
    let mut nav = Navigation::new();
    let mut bad = frame(0);
    bad.validity = 0;
    assert_eq!(nav.update(&bad), Err(NavigationError::MissingInertial));
    nav.update(&frame(0)).unwrap();
    assert_eq!(nav.update(&frame(2)), Err(NavigationError::Sequence));
}

#[test]
fn altimeter_alpha_beta_update_is_bounded_and_directional() {
    let mut nav = Navigation::new();
    nav.update(&frame(0)).unwrap();
    let mut aided = frame(1);
    aided.validity |= SENSOR_VALID_ALTIMETER;
    aided.altitude_q12 = 4096;
    let state = nav.update(&aided).unwrap();
    assert!(state.altitude_aided);
    assert!(state.radius_q12 > EARTH_RADIUS_Q12);
    assert!(state.radius_q12 < EARTH_RADIUS_Q12 + 4096);
    assert!(state.radial_velocity_q24 > 0);
}

#[test]
fn gps_pvt_update_moves_every_component_toward_measurement() {
    let mut nav = Navigation::new();
    nav.update(&frame(0)).unwrap();
    let prior = nav.state();
    let mut aided = frame(1);
    aided.validity |= SENSOR_VALID_GPS;
    aided.gps_radius_q12 = EARTH_RADIUS_Q12 + 200 * 4096;
    aided.gps_downrange_q32 = 100_000;
    aided.gps_radial_velocity_q24 = 10_000;
    aided.gps_tangential_velocity_q24 = 130_000_000;
    let state = nav.update(&aided).unwrap();
    assert!(state.gps_aided);
    assert!(state.radius_q12 > prior.radius_q12);
    assert!(state.downrange_q32 > 0);
    assert!(state.radial_velocity_q24 > 0);
    assert!(state.tangential_velocity_q24 > prior.tangential_velocity_q24);
}

#[test]
fn deterministic_inertial_bridge_advances_without_gps() {
    let mut a = Navigation::new();
    let mut b = Navigation::new();
    for sequence in 0..=480 {
        let f = frame(sequence);
        assert_eq!(a.update(&f), b.update(&f));
    }
    let state = a.state();
    assert_eq!(state, b.state());
    assert_ne!(state.checksum, 2_166_136_261);
    assert_eq!(state.sequence, 480);
}
