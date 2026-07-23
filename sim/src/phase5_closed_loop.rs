//! Phase 5 sensor/flight/actuator composition through strict wire contracts.

use crate::phase5_sensors::{Phase5SensorFaults, Phase5SensorParameters, Phase5SensorSuite};
use crate::phase5_vehicle::{
    GimbalCommandQ16, Phase5VehicleCommand, Phase5VehicleError, Phase5VehicleMachine,
    Phase5VehicleSnapshot,
};
use ksa64_core::spatial_numeric::FixedVec3;
use ksa64_flight::phase5_gnc::{
    AttitudeControllerGains, SpatialFlightComputer, SpatialFlightOutput, SpatialGuidanceTarget,
};
use ksa64_interface::phase5::{
    parse_spatial_actuator_command, write_spatial_actuator_command, write_spatial_sensor_frame,
    SpatialActuatorCommand, SpatialSensorFrame, SPATIAL_ACTUATOR_COMMAND_LENGTH,
    SPATIAL_SENSOR_FRAME_LENGTH,
};
use ksa64_interface::CodecError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5ClosedLoopError {
    Vehicle(Phase5VehicleError),
    SensorCodec(CodecError),
    CommandCodec(CodecError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5ClosedLoopStep {
    pub sensor: SpatialSensorFrame,
    pub flight: SpatialFlightOutput,
    pub vehicle: Phase5VehicleSnapshot,
    pub sensor_checksum: u32,
}

pub struct Phase5ClosedLoop {
    vehicle: Phase5VehicleMachine,
    sensors: Phase5SensorSuite,
    flight: SpatialFlightComputer,
    latest: Phase5VehicleSnapshot,
}

impl Phase5ClosedLoop {
    pub fn new(seed: u32, faults: Phase5SensorFaults) -> Result<Self, Phase5ClosedLoopError> {
        Self::new_parameterized(seed, faults, Phase5SensorParameters::DEFAULT)
    }

    pub fn new_parameterized(
        seed: u32,
        faults: Phase5SensorFaults,
        parameters: Phase5SensorParameters,
    ) -> Result<Self, Phase5ClosedLoopError> {
        let vehicle = Phase5VehicleMachine::new_ksa5a().map_err(Phase5ClosedLoopError::Vehicle)?;
        let latest = vehicle
            .current_snapshot()
            .map_err(Phase5ClosedLoopError::Vehicle)?;
        Ok(Self {
            vehicle,
            sensors: Phase5SensorSuite::new_parameterized(seed, faults, parameters),
            flight: SpatialFlightComputer::new(),
            latest,
        })
    }

    pub const fn latest(&self) -> Phase5VehicleSnapshot {
        self.latest
    }
    pub const fn flight(&self) -> &SpatialFlightComputer {
        &self.flight
    }
    pub const fn vehicle(&self) -> &Phase5VehicleMachine {
        &self.vehicle
    }
    pub fn vehicle_mut(&mut self) -> &mut Phase5VehicleMachine {
        &mut self.vehicle
    }

    pub fn step(
        &mut self,
        target: SpatialGuidanceTarget,
    ) -> Result<Phase5ClosedLoopStep, Phase5ClosedLoopError> {
        self.step_with_gains(target, AttitudeControllerGains::GATE7)
    }

    pub fn step_with_gains(
        &mut self,
        target: SpatialGuidanceTarget,
        gains: AttitudeControllerGains,
    ) -> Result<Phase5ClosedLoopStep, Phase5ClosedLoopError> {
        let sensor = self.sensors.sample(self.latest);
        let mut sensor_bytes = [0u8; SPATIAL_SENSOR_FRAME_LENGTH];
        write_spatial_sensor_frame(&sensor, &mut sensor_bytes)
            .map_err(Phase5ClosedLoopError::SensorCodec)?;
        let flight = self
            .flight
            .step_serialized_with_gains(&sensor_bytes, target, gains);
        let mut command_bytes = [0u8; SPATIAL_ACTUATOR_COMMAND_LENGTH];
        write_spatial_actuator_command(&flight.command, &mut command_bytes)
            .map_err(Phase5ClosedLoopError::CommandCodec)?;
        let decoded = parse_spatial_actuator_command(&command_bytes)
            .map_err(Phase5ClosedLoopError::CommandCodec)?;
        let vehicle_command = map_command(decoded);
        self.latest = self
            .vehicle
            .step(vehicle_command)
            .map_err(Phase5ClosedLoopError::Vehicle)?;
        Ok(Phase5ClosedLoopStep {
            sensor,
            flight,
            vehicle: self.latest,
            sensor_checksum: self.sensors.checksum(),
        })
    }
}

fn map_command(command: SpatialActuatorCommand) -> Phase5VehicleCommand {
    Phase5VehicleCommand {
        gimbal: GimbalCommandQ16 {
            pitch: command.gimbal_q16[0],
            yaw: command.gimbal_q16[1],
        },
        rcs_q15: FixedVec3::new(command.rcs_q15[0], command.rcs_q15[1], command.rcs_q15[2]),
        engine_action: command.engine_action,
        separate: command.separate,
        abort_safeing: command.abort_safeing,
    }
}
