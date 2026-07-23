use ksa64_flight::phase5_gnc::*;
use ksa64_flight::phase5_navigation::*;
use ksa64_interface::phase5::*;
use ksa64_interface::{EngineAction, FlightMode, StagePhase, ALARM_SENSOR_FRAME};

fn frame(sequence: u32) -> SpatialSensorFrame {
    SpatialSensorFrame {
        sequence,
        onboard_time_q16: sequence as i32 * 8_192,
        validity: SENSOR_VALID_IMU | SENSOR_VALID_CLOCK | SENSOR_VALID_ACTUATOR,
        events: 0,
        accel_body_q28: [2_630_000, 0, 0],
        gyro_body_q24: [0; 3],
        baro_altitude_q12: 0,
        gps_position_q12: [0; 3],
        gps_velocity_q24: [0; 3],
        star_attitude_q30: [0; 4],
        gimbal_applied_q16: [0; 2],
        rcs_propellant_q12: 410,
        active_stage: 0,
        stage_phase: StagePhase::CoastBeforeIgnition,
        engine_on: false,
    }
}

#[test]
fn spatial_navigation_rejects_missing_imu_and_sequence_gaps() {
    let mut navigation = SpatialNavigation::new();
    let mut missing = frame(1);
    missing.validity = SENSOR_VALID_CLOCK;
    assert_eq!(
        navigation.update(&missing),
        Err(SpatialNavigationError::MissingInertial)
    );
    navigation.update(&frame(1)).unwrap();
    assert_eq!(
        navigation.update(&frame(3)),
        Err(SpatialNavigationError::Sequence)
    );
}

#[test]
fn gps_star_and_barometer_aiding_are_bounded_and_reported() {
    let mut navigation = SpatialNavigation::new();
    navigation.update(&frame(1)).unwrap();
    let prior = navigation.state();
    let mut aided = frame(2);
    aided.validity |= SENSOR_VALID_GPS | SENSOR_VALID_STAR_TRACKER | SENSOR_VALID_BAROMETER;
    aided.gps_position_q12 = [
        INITIAL_POSITION_Q12[0] + 4_096,
        INITIAL_POSITION_Q12[1] + 4_096,
        INITIAL_POSITION_Q12[2],
    ];
    aided.gps_velocity_q24 = INITIAL_VELOCITY_Q24;
    aided.baro_altitude_q12 = 4_096;
    aided.star_attitude_q30 = INITIAL_ATTITUDE_Q30;
    let state = navigation.update(&aided).unwrap();
    assert!(state.gps_aided && state.star_aided && state.barometer_aided);
    assert_ne!(state.position_q12, prior.position_q12);
    assert_eq!(state.attitude_q30, INITIAL_ATTITUDE_Q30);
}

#[test]
fn attitude_controller_is_zero_at_target_and_sequencer_ignites() {
    let mut flight = SpatialFlightComputer::new();
    let output = flight.step(&frame(1), SpatialGuidanceTarget::hold(INITIAL_ATTITUDE_Q30));
    assert_eq!(output.command.gimbal_q16, [0; 2]);
    assert_eq!(output.command.engine_action, EngineAction::Ignite);
    assert_eq!(output.mode, FlightMode::ProgrammedAscent);
}

#[test]
fn corrupt_spatial_transport_aborts_closed() {
    let source = frame(1);
    let mut bytes = [0u8; SPATIAL_SENSOR_FRAME_LENGTH];
    write_spatial_sensor_frame(&source, &mut bytes).unwrap();
    bytes[20] ^= 1;
    let mut flight = SpatialFlightComputer::new();
    let output = flight.step_serialized(&bytes, SpatialGuidanceTarget::hold(INITIAL_ATTITUDE_Q30));
    assert_eq!(output.mode, FlightMode::Abort);
    assert_eq!(output.command.engine_action, EngineAction::Cutoff);
    assert!(output.command.abort_safeing);
    assert!(output.alarms & ALARM_SENSOR_FRAME != 0);
}

#[test]
fn persistent_gimbal_tracking_error_latches_abort() {
    let mut flight = SpatialFlightComputer::new();
    let target = SpatialGuidanceTarget::hold([1_069_687_889, 0, 93_585_361, 0]);
    let mut output = flight.step(&frame(1), target);
    assert_ne!(output.command.gimbal_q16[0], 0);
    for sequence in 2..=20 {
        let mut sensor = frame(sequence);
        sensor.stage_phase = StagePhase::Burning;
        sensor.engine_on = true;
        output = flight.step(&sensor, target);
    }
    assert_eq!(output.mode, FlightMode::Abort);
    assert!(output.command.abort_safeing);
}
