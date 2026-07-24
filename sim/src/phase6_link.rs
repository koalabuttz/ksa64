//! Phase 6 deterministic link, transcript, and exact paced endpoints.
use crate::phase5_sensors::{Phase5SensorFaults, Phase5SensorSuite};
use crate::phase5_vehicle::{
    GimbalCommandQ16, Phase5StagePhase, Phase5VehicleCommand, Phase5VehicleError,
    Phase5VehicleMachine, Phase5VehicleSnapshot,
};
use ksa64_core::spatial_numeric::FixedVec3;
use ksa64_flight::phase5_gnc::{
    AttitudeControllerGains, SpatialFlightComputer, SpatialFlightOutput,
};
use ksa64_flight::phase5_guidance::reference_guidance_target;
use ksa64_interface::phase5::{
    parse_spatial_actuator_command, parse_spatial_sensor_frame, write_spatial_actuator_command,
    write_spatial_sensor_frame, SpatialActuatorCommand, SPATIAL_ACTUATOR_COMMAND_LENGTH,
    SPATIAL_SENSOR_FRAME_LENGTH,
};
use ksa64_interface::phase6::{
    parse_link_frame, write_link_frame, EndpointRole, LinkCodecError, LinkFrame, LinkHeader,
    LinkRecordType, KLF6_MAX_DECODED, KLF6_MAX_ENCODED, KLF6_NONE, LINK_FLAG_ACK_REQUIRED,
};
use ksa64_interface::FlightMode;

pub const PHASE6_SESSION_ID: u32 = 0x4b53_4136;
pub const TRANSCRIPT_INITIAL: u32 = 2_166_136_261;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DeliveryDisposition {
    Delivered,
    Delayed,
    Dropped,
    Duplicated,
    Corrupted,
    Disconnected,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranscriptRecord {
    pub logical_time: u32,
    pub source: EndpointRole,
    pub destination: EndpointRole,
    pub disposition: DeliveryDisposition,
    pub frame_length: u16,
    pub frame_crc: u32,
    pub chain: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transcript {
    pub count: u32,
    pub chain: u32,
    pub last: Option<TranscriptRecord>,
}
impl Transcript {
    pub const fn new() -> Self {
        Self {
            count: 0,
            chain: TRANSCRIPT_INITIAL,
            last: None,
        }
    }
    pub fn record(
        &mut self,
        logical_time: u32,
        source: EndpointRole,
        destination: EndpointRole,
        disposition: DeliveryDisposition,
        frame: &[u8],
    ) {
        let crc = ksa64_interface::crc32_ieee(frame);
        let mut chain = self.chain;
        chain = hash(chain, logical_time);
        chain = hash(chain, source as u32);
        chain = hash(chain, destination as u32);
        chain = hash(chain, disposition as u32);
        chain = hash(chain, frame.len() as u32);
        chain = hash(chain, crc);
        self.count = self.count.wrapping_add(1);
        self.chain = chain;
        self.last = Some(TranscriptRecord {
            logical_time,
            source,
            destination,
            disposition,
            frame_length: frame.len() as u16,
            frame_crc: crc,
            chain,
        })
    }
}
impl Default for Transcript {
    fn default() -> Self {
        Self::new()
    }
}
fn hash(mut h: u32, v: u32) -> u32 {
    let mut s = 0;
    while s < 32 {
        h ^= (v >> s) & 255;
        h = h.wrapping_mul(16_777_619);
        s += 8
    }
    h
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImpairmentSchedule {
    pub delay_at: Option<u32>,
    pub drop_at: Option<u32>,
    pub duplicate_at: Option<u32>,
    pub corrupt_at: Option<u32>,
    pub disconnect_at: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokerError {
    Capacity,
    Disconnected,
}
pub struct DeterministicBroker {
    schedule: ImpairmentSchedule,
    logical_time: u32,
    connected: bool,
    pending: [u8; KLF6_MAX_ENCODED],
    pending_length: usize,
    pending_valid: bool,
    pub transcript: Transcript,
}
impl DeterministicBroker {
    pub const fn new(schedule: ImpairmentSchedule) -> Self {
        Self {
            schedule,
            logical_time: 0,
            connected: true,
            pending: [0; KLF6_MAX_ENCODED],
            pending_length: 0,
            pending_valid: false,
            transcript: Transcript::new(),
        }
    }
    pub fn route(
        &mut self,
        source: EndpointRole,
        destination: EndpointRole,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, bool), BrokerError> {
        let now = self.logical_time;
        self.logical_time = self.logical_time.wrapping_add(1);
        if !self.connected || self.schedule.disconnect_at == Some(now) {
            self.connected = false;
            self.transcript.record(
                now,
                source,
                destination,
                DeliveryDisposition::Disconnected,
                input,
            );
            return Err(BrokerError::Disconnected);
        }
        if self.pending_valid {
            if output.len() < self.pending_length {
                return Err(BrokerError::Capacity);
            }
            output[..self.pending_length].copy_from_slice(&self.pending[..self.pending_length]);
            let n = self.pending_length;
            self.pending_valid = false;
            self.transcript.record(
                now,
                source,
                destination,
                DeliveryDisposition::Delivered,
                &output[..n],
            );
            return Ok((n, false));
        }
        if self.schedule.drop_at == Some(now) {
            self.transcript.record(
                now,
                source,
                destination,
                DeliveryDisposition::Dropped,
                input,
            );
            return Ok((0, false));
        }
        if output.len() < input.len() {
            return Err(BrokerError::Capacity);
        }
        output[..input.len()].copy_from_slice(input);
        let mut disposition = DeliveryDisposition::Delivered;
        if self.schedule.corrupt_at == Some(now) && !input.is_empty() {
            output[input.len() / 2] ^= 0x40;
            disposition = DeliveryDisposition::Corrupted
        }
        if self.schedule.delay_at == Some(now) {
            self.pending[..input.len()].copy_from_slice(&output[..input.len()]);
            self.pending_length = input.len();
            self.pending_valid = true;
            self.transcript.record(
                now,
                source,
                destination,
                DeliveryDisposition::Delayed,
                input,
            );
            return Ok((0, false));
        }
        let duplicate = self.schedule.duplicate_at == Some(now);
        if duplicate {
            disposition = DeliveryDisposition::Duplicated
        }
        self.transcript.record(
            now,
            source,
            destination,
            disposition,
            &output[..input.len()],
        );
        Ok((input.len(), duplicate))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactLinkError {
    Vehicle(Phase5VehicleError),
    Link(LinkCodecError),
    Payload,
    Sequence,
}
pub struct ExactWorldEndpoint {
    vehicle: Phase5VehicleMachine,
    sensors: Phase5SensorSuite,
    latest: Phase5VehicleSnapshot,
    pending_sequence: Option<u32>,
    pending_sensor: [u8; SPATIAL_SENSOR_FRAME_LENGTH],
}
impl ExactWorldEndpoint {
    pub fn new(seed: u32) -> Result<Self, ExactLinkError> {
        let vehicle = Phase5VehicleMachine::new_ksa5a().map_err(ExactLinkError::Vehicle)?;
        let latest = vehicle
            .current_snapshot()
            .map_err(ExactLinkError::Vehicle)?;
        Ok(Self {
            vehicle,
            sensors: Phase5SensorSuite::new(seed, Phase5SensorFaults::default()),
            latest,
            pending_sequence: None,
            pending_sensor: [0; SPATIAL_SENSOR_FRAME_LENGTH],
        })
    }
    pub const fn latest(&self) -> Phase5VehicleSnapshot {
        self.latest
    }
    pub const fn sensor_checksum(&self) -> u32 {
        self.sensors.checksum()
    }
    pub fn sensor_message(&mut self, out: &mut [u8]) -> Result<usize, ExactLinkError> {
        if self.pending_sequence.is_none() {
            let sensor = self.sensors.sample(self.latest);
            write_spatial_sensor_frame(&sensor, &mut self.pending_sensor)
                .map_err(|_| ExactLinkError::Payload)?;
            self.pending_sequence = Some(sensor.sequence)
        }
        let sequence = self.pending_sequence.unwrap_or(0);
        let frame = LinkFrame {
            header: LinkHeader {
                record_type: LinkRecordType::CanonicalSensor,
                flags: LINK_FLAG_ACK_REQUIRED,
                session_id: PHASE6_SESSION_ID,
                sequence,
                acknowledgement: KLF6_NONE,
                measurement_epoch: sequence,
                production_epoch: sequence,
                effective_epoch: sequence,
            },
            payload: &self.pending_sensor,
        };
        let mut decoded = [0u8; KLF6_MAX_DECODED];
        write_link_frame(&frame, &mut decoded, out).map_err(ExactLinkError::Link)
    }
    pub fn accept_command(
        &mut self,
        input: &[u8],
        decode: &mut [u8],
    ) -> Result<Phase5VehicleSnapshot, ExactLinkError> {
        let frame = parse_link_frame(input, decode).map_err(ExactLinkError::Link)?;
        if frame.header.record_type != LinkRecordType::CanonicalCommand
            || frame.header.session_id != PHASE6_SESSION_ID
            || Some(frame.header.sequence) != self.pending_sequence
        {
            return Err(ExactLinkError::Sequence);
        }
        let command =
            parse_spatial_actuator_command(frame.payload).map_err(|_| ExactLinkError::Payload)?;
        if command.sequence != frame.header.sequence {
            return Err(ExactLinkError::Sequence);
        }
        self.latest = self
            .vehicle
            .step(map_command(command))
            .map_err(ExactLinkError::Vehicle)?;
        self.pending_sequence = None;
        Ok(self.latest)
    }
}

pub struct ExactFlightEndpoint {
    flight: SpatialFlightComputer,
    cached_sequence: Option<u32>,
    cached_command: [u8; SPATIAL_ACTUATOR_COMMAND_LENGTH],
    last_output: Option<SpatialFlightOutput>,
}
impl ExactFlightEndpoint {
    pub const fn new() -> Self {
        Self {
            flight: SpatialFlightComputer::new(),
            cached_sequence: None,
            cached_command: [0; SPATIAL_ACTUATOR_COMMAND_LENGTH],
            last_output: None,
        }
    }
    pub const fn navigation_checksum(&self) -> u32 {
        self.flight.navigation().checksum
    }
    pub const fn flight_checksum(&self) -> u32 {
        self.flight.status().checksum
    }
    pub const fn last_output(&self) -> Option<SpatialFlightOutput> {
        self.last_output
    }
    pub fn accept_sensor(
        &mut self,
        input: &[u8],
        decode: &mut [u8],
        out: &mut [u8],
    ) -> Result<usize, ExactLinkError> {
        let frame = parse_link_frame(input, decode).map_err(ExactLinkError::Link)?;
        if frame.header.record_type != LinkRecordType::CanonicalSensor
            || frame.header.session_id != PHASE6_SESSION_ID
        {
            return Err(ExactLinkError::Sequence);
        }
        if self.cached_sequence != Some(frame.header.sequence) {
            let sensor =
                parse_spatial_sensor_frame(frame.payload).map_err(|_| ExactLinkError::Payload)?;
            if sensor.sequence != frame.header.sequence {
                return Err(ExactLinkError::Sequence);
            }
            let gains = if sensor.active_stage == 0 {
                AttitudeControllerGains::REFERENCE_STAGE1
            } else {
                AttitudeControllerGains::REFERENCE_STAGE2
            };
            let output = self.flight.step_with_gains(
                &sensor,
                reference_guidance_target(sensor.sequence),
                gains,
            );
            write_spatial_actuator_command(&output.command, &mut self.cached_command)
                .map_err(|_| ExactLinkError::Payload)?;
            self.cached_sequence = Some(sensor.sequence);
            self.last_output = Some(output)
        }
        let response = LinkFrame {
            header: LinkHeader {
                record_type: LinkRecordType::CanonicalCommand,
                flags: LINK_FLAG_ACK_REQUIRED,
                session_id: PHASE6_SESSION_ID,
                sequence: frame.header.sequence,
                acknowledgement: frame.header.sequence,
                measurement_epoch: frame.header.measurement_epoch,
                production_epoch: frame.header.production_epoch,
                effective_epoch: frame.header.effective_epoch,
            },
            payload: &self.cached_command,
        };
        let mut scratch = [0u8; KLF6_MAX_DECODED];
        write_link_frame(&response, &mut scratch, out).map_err(ExactLinkError::Link)
    }
    pub fn acknowledge_commit(&mut self, sequence: u32) -> Result<(), ExactLinkError> {
        if self.cached_sequence != Some(sequence) {
            return Err(ExactLinkError::Sequence);
        }
        self.cached_sequence = None;
        Ok(())
    }
}
impl Default for ExactFlightEndpoint {
    fn default() -> Self {
        Self::new()
    }
}
fn map_command(c: SpatialActuatorCommand) -> Phase5VehicleCommand {
    Phase5VehicleCommand {
        gimbal: GimbalCommandQ16 {
            pitch: c.gimbal_q16[0],
            yaw: c.gimbal_q16[1],
        },
        rcs_q15: FixedVec3::new(c.rcs_q15[0], c.rcs_q15[1], c.rcs_q15[2]),
        engine_action: c.engine_action,
        separate: c.separate,
        abort_safeing: c.abort_safeing,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactRunEvidence {
    pub steps: u32,
    pub terminal_position_q12: [i32; 3],
    pub terminal_velocity_q24: [i32; 3],
    pub sensor_checksum: u32,
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
    pub transcript_checksum: u32,
}
pub fn run_exact_nominal() -> Result<ExactRunEvidence, ExactLinkError> {
    let mut world = ExactWorldEndpoint::new(0x5a00_0000)?;
    let mut flight = ExactFlightEndpoint::new();
    let mut broker = DeterministicBroker::new(ImpairmentSchedule::default());
    let mut a = [0u8; KLF6_MAX_ENCODED];
    let mut b = [0u8; KLF6_MAX_ENCODED];
    let mut c = [0u8; KLF6_MAX_ENCODED];
    let mut d = [0u8; KLF6_MAX_ENCODED];
    let mut decode_a = [0u8; KLF6_MAX_DECODED];
    let mut decode_b = [0u8; KLF6_MAX_DECODED];
    let mut step = 0;
    while step < ksa64_core::phase5_contract::PHASE5_MISSION_STEPS {
        let an = world.sensor_message(&mut a)?;
        let (bn, _) = broker
            .route(EndpointRole::World, EndpointRole::Flight, &a[..an], &mut b)
            .map_err(|_| ExactLinkError::Payload)?;
        let cn = flight.accept_sensor(&b[..bn], &mut decode_a, &mut c)?;
        let (dn, _) = broker
            .route(EndpointRole::Flight, EndpointRole::World, &c[..cn], &mut d)
            .map_err(|_| ExactLinkError::Payload)?;
        world.accept_command(&d[..dn], &mut decode_b)?;
        flight.acknowledge_commit(step)?;
        step += 1;
        if world.latest().truth.phase() == Phase5StagePhase::Complete
            || flight
                .last_output()
                .map(|o| o.mode == FlightMode::Abort)
                .unwrap_or(false)
        {
            break;
        }
    }
    let truth = world.latest().truth;
    let p = truth.spatial().position();
    let v = truth.spatial().velocity();
    Ok(ExactRunEvidence {
        steps: truth.step(),
        terminal_position_q12: [p.x(), p.y(), p.z()],
        terminal_velocity_q24: [v.x(), v.y(), v.z()],
        sensor_checksum: world.sensor_checksum(),
        navigation_checksum: flight.navigation_checksum(),
        flight_checksum: flight.flight_checksum(),
        transcript_checksum: broker.transcript.chain,
    })
}
