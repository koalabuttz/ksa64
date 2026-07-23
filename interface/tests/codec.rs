use ksa64_interface::*;

fn sample_sensor() -> SensorFrame {
    SensorFrame {
        sequence: 42,
        onboard_time_q16: 123,
        accel_radial_q28: -456,
        accel_tangential_q28: 789,
        gyro_rate_q24: -11,
        steering_pitch: 0x1234,
        validity: SENSOR_VALID_MASK,
        altitude_q12: 100,
        gps_radius_q12: 200,
        gps_downrange_q32: 300,
        gps_radial_velocity_q24: -400,
        gps_tangential_velocity_q24: 500,
        events: EVENT_IGNITION | EVENT_GPS_ACQUIRED,
        active_stage: 1,
        stage_phase: StagePhase::Burning,
        engine_on: true,
    }
}

#[test]
fn sensor_round_trip_and_corruption_fail_closed() {
    let frame = sample_sensor();
    let mut bytes = [0u8; SENSOR_FRAME_LENGTH];
    write_sensor_frame(&frame, &mut bytes).unwrap();
    assert_eq!(parse_sensor_frame(&bytes), Ok(frame));
    bytes[12] ^= 0x80;
    assert_eq!(parse_sensor_frame(&bytes), Err(CodecError::Checksum));
}

#[test]
fn reserved_sensor_bytes_fail_closed_even_with_valid_crc() {
    let mut bytes = [0u8; SENSOR_FRAME_LENGTH];
    write_sensor_frame(&sample_sensor(), &mut bytes).unwrap();
    bytes[50] = 1;
    let crc = crc32_ieee(&bytes[..52]).to_le_bytes();
    bytes[52..56].copy_from_slice(&crc);
    assert_eq!(parse_sensor_frame(&bytes), Err(CodecError::Reserved));
}

#[test]
fn command_and_output_round_trip() {
    let command = ActuatorCommand {
        sequence: 7,
        desired_pitch: 16_384,
        engine_action: EngineAction::Ignite,
        separate: true,
        abort_safeing: false,
        recovery_requested: true,
    };
    let mut bytes = [0u8; ACTUATOR_COMMAND_LENGTH];
    write_actuator_command(&command, &mut bytes).unwrap();
    assert_eq!(parse_actuator_command(&bytes), Ok(command));
    let output = FlightOutput {
        sequence: 7,
        nav_time_q16: 1,
        nav_radius_q12: 2,
        nav_downrange_q32: 3,
        nav_radial_velocity_q24: 4,
        nav_tangential_velocity_q24: 5,
        nav_pitch: 6,
        mode: FlightMode::Insertion,
        alarms: 0,
        command,
        nav_checksum: 8,
        flight_checksum: 9,
    };
    let mut output_bytes = [0u8; FLIGHT_OUTPUT_LENGTH];
    write_flight_output(&output, &mut output_bytes).unwrap();
    assert_eq!(parse_flight_output(&output_bytes), Ok(output));
}
