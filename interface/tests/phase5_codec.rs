use ksa64_interface::phase5::*;
use ksa64_interface::{crc32_ieee, CodecError, EngineAction, StagePhase};

fn frame() -> SpatialSensorFrame {
    SpatialSensorFrame {
        sequence: 17,
        onboard_time_q16: 139_264,
        validity: SENSOR_VALID_MASK,
        events: EVENT_IGNITION | EVENT_STAR_ACQUIRED | EVENT_GIMBAL_JAMMED,
        accel_body_q28: [-1, 2, -3],
        gyro_body_q24: [4, -5, 6],
        baro_altitude_q12: 7,
        gps_position_q12: [8, -9, 10],
        gps_velocity_q24: [-11, 12, -13],
        star_attitude_q30: [1 << 30, 14, -15, 16],
        gimbal_applied_q16: [17, -18],
        rcs_propellant_q12: 19,
        active_stage: 1,
        stage_phase: StagePhase::Burning,
        engine_on: true,
    }
}

#[test]
fn spatial_sensor_round_trip_and_strict_failures() {
    let expected = frame();
    let mut bytes = [0u8; SPATIAL_SENSOR_FRAME_LENGTH];
    write_spatial_sensor_frame(&expected, &mut bytes).unwrap();
    assert_eq!(parse_spatial_sensor_frame(&bytes), Ok(expected));
    bytes[32] ^= 1;
    assert_eq!(
        parse_spatial_sensor_frame(&bytes),
        Err(CodecError::Checksum)
    );
    write_spatial_sensor_frame(&expected, &mut bytes).unwrap();
    bytes[100] = 1;
    let crc = crc32_ieee(&bytes[..124]).to_le_bytes();
    bytes[124..128].copy_from_slice(&crc);
    assert_eq!(
        parse_spatial_sensor_frame(&bytes),
        Err(CodecError::Reserved)
    );
}

#[test]
fn spatial_command_round_trip_and_safe_default() {
    let command = SpatialActuatorCommand {
        sequence: 4,
        gimbal_q16: [123, -456],
        rcs_q15: [1, -2, 3],
        engine_action: EngineAction::Ignite,
        separate: true,
        abort_safeing: false,
    };
    let mut bytes = [0u8; SPATIAL_ACTUATOR_COMMAND_LENGTH];
    write_spatial_actuator_command(&command, &mut bytes).unwrap();
    assert_eq!(parse_spatial_actuator_command(&bytes), Ok(command));
    assert_eq!(
        SpatialActuatorCommand::SAFE.engine_action,
        EngineAction::Cutoff
    );
    bytes[24] = 99;
    let crc = crc32_ieee(&bytes[..28]).to_le_bytes();
    bytes[28..32].copy_from_slice(&crc);
    assert_eq!(
        parse_spatial_actuator_command(&bytes),
        Err(CodecError::Enum)
    );
}
