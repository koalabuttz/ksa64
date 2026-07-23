use ksa64_flight::phase5_gnc::{AttitudeControllerGains, SpatialFlightComputer};
use ksa64_flight::phase5_guidance::reference_guidance_target;
use ksa64_interface::phase5::{
    parse_spatial_actuator_command, write_spatial_actuator_command, write_spatial_sensor_frame,
    SPATIAL_ACTUATOR_COMMAND_LENGTH, SPATIAL_SENSOR_FRAME_LENGTH,
};
use ksa64_interface::{crc32_ieee, EngineAction};
use ksa64_sim::phase5_sensors::{Phase5SensorFaults, Phase5SensorSuite};
use ksa64_sim::phase5_telemetry::{
    encode_phase5_telemetry_observation, initial_frame, PHASE5_TELEMETRY_FRAME_LENGTH,
};
use ksa64_sim::phase5_vehicle::{Phase5VehicleCommand, Phase5VehicleMachine};

fn main() {
    let mut vehicle = Phase5VehicleMachine::new_ksa5a().unwrap();
    let v = vehicle
        .step(Phase5VehicleCommand {
            engine_action: EngineAction::Ignite,
            ..Phase5VehicleCommand::HOLD
        })
        .unwrap();
    let initial = Phase5VehicleMachine::new_ksa5a()
        .and_then(|m| m.current_snapshot())
        .unwrap();
    let mut sensors = Phase5SensorSuite::new(0x5a00_0000, Phase5SensorFaults::default());
    let mut flight = SpatialFlightComputer::new();
    let sensor = sensors.sample(initial);
    let mut sensor_bytes = [0; SPATIAL_SENSOR_FRAME_LENGTH];
    write_spatial_sensor_frame(&sensor, &mut sensor_bytes).unwrap();
    let output = flight.step_serialized_with_gains(
        &sensor_bytes,
        reference_guidance_target(0),
        AttitudeControllerGains::REFERENCE_STAGE1,
    );
    let mut command_bytes = [0; SPATIAL_ACTUATOR_COMMAND_LENGTH];
    write_spatial_actuator_command(&output.command, &mut command_bytes).unwrap();
    parse_spatial_actuator_command(&command_bytes).unwrap();
    let mut telemetry = [0; PHASE5_TELEMETRY_FRAME_LENGTH];
    let observation =
        encode_phase5_telemetry_observation(initial_frame(initial), 2_166_136_261, &mut telemetry)
            .unwrap();
    let p = v.truth.spatial().position();
    println!(
        concat!(
            "{{\"vehicle_step\":{},\"vehicle_position\":[{},{},{}],",
            "\"sensor_checksum\":{},\"navigation_checksum\":{},",
            "\"flight_checksum\":{},\"command_crc32\":{},",
            "\"observation_checksum\":{},\"telemetry_crc32\":{},",
            "\"telemetry_bytes\":{}}}"
        ),
        v.truth.step(),
        p.x(),
        p.y(),
        p.z(),
        sensors.checksum(),
        output.navigation.checksum,
        output.flight_checksum,
        crc32_ieee(&command_bytes[..SPATIAL_ACTUATOR_COMMAND_LENGTH - 4]),
        observation,
        crc32_ieee(&telemetry[..PHASE5_TELEMETRY_FRAME_LENGTH - 4]),
        PHASE5_TELEMETRY_FRAME_LENGTH
    );
}
