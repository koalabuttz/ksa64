//! KLF6-framed Phase 9.5 host-world / externally paced flight placement.
use crate::phase8_5::checked_in_reference;
use crate::phase9_5_workbench::{
    materialize_advanced_candidate, AdvancedReference, AdvancedStudyId,
};
use ksa64_core::phase8_5_contract::ActuatorCapabilityPack;
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_core::phase8_pack::{parse_spatial_vehicle_pack, SpatialVehiclePack};
use ksa64_core::phase9_5_contract::{
    parse_allocator_pack, parse_effector_pack, AdvancedEffectorPack, PriorityResidualAllocatorPack,
};
use ksa64_core::phase9_contract::{DesignVector, SearchManifest};
use ksa64_flight::phase9_5::AdvancedFlightComputer;
use ksa64_flight::phase9_5_allocator::{AllocatedAdvancedFlightComputer, AllocatedFlightEvidence};
use ksa64_interface::phase6::{
    parse_capabilities, parse_link_frame, write_capabilities, write_link_frame, EndpointRole,
    LinkCapabilities, LinkFrame, LinkHeader, LinkMode, LinkRecordType, CAPABILITY_PAYLOAD_LENGTH,
    CAP_EXACT_PACED, CAP_MISSION_CONTROL, CAP_TRANSCRIPT, KLF6_MAX_DECODED, KLF6_MAX_ENCODED,
    KLF6_NONE, PHASE6_LINK_CONTRACT_ID,
};
use ksa64_interface::phase9_5::{
    parse_advanced_aid, parse_advanced_command, parse_advanced_fast_sensor, parse_advanced_status,
    write_advanced_aid, write_advanced_command, write_advanced_fast_sensor, write_advanced_status,
    AdvancedAidCell, AdvancedCommandCell, AdvancedStatusCell, ADVANCED_AID_LENGTH,
    ADVANCED_COMMAND_LENGTH, ADVANCED_FAST_SENSOR_LENGTH, ADVANCED_STATUS_LENGTH, KLR9_CONTRACT_ID,
};
use ksa64_sim::phase8_5::{reference_avionics_profile, reference_gimbal_capability};
use ksa64_sim::phase9_5::{advanced_flight_config, allocator_config};
use ksa64_sim::phase9_5_bootstrap::{
    parse_flight_bootstrap, write_flight_bootstrap, AdvancedFlightBootstrap, KFB9_CONTRACT_ID,
    KFB9_LENGTH,
};
use ksa64_sim::phase9_5_mission::{
    reference_capability, AdvancedMissionFaults, AdvancedWorldEndpoint,
};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

const LINK_SESSION: u32 = 0x4b4c_5239;
const FNV_OFFSET: u32 = 0x811c_9dc5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase95Placement {
    HostHost,
    HostExternalFlight,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phase95Update {
    pub epoch: u16,
    pub time_s: f64,
    pub phase: u8,
    pub events: u16,
    pub truth_position_m: [f64; 3],
    pub truth_velocity_mps: [f64; 3],
    pub onboard_position_m: [f64; 3],
    pub onboard_velocity_mps: [f64; 3],
    pub ground_position_m: [f64; 3],
    pub ground_velocity_mps: [f64; 3],
    pub attitude: [f64; 4],
    pub angular_rate_rad_s: [f64; 3],
    pub requested_torque_nm: [f64; 3],
    pub achieved_torque_nm: [f64; 3],
    pub residual_torque_nm: [f64; 3],
    pub commanded_gimbal: [i16; 2],
    pub applied_gimbal: [i16; 2],
    pub commanded_canards: [i16; 4],
    pub applied_canards: [i16; 4],
    pub rcs_pulse_quanta: [u8; 12],
    pub valve_open_mask: u16,
    pub authority_state: u16,
    pub saturation_count: u16,
    pub air_data_source: u8,
    pub sensor_validity: u16,
    pub aid_validity: u16,
    pub command_flags: u8,
    pub command_discrete: u8,
    pub alarms: u16,
    pub deadline_misses: u16,
    pub safe: bool,
    pub armed: bool,
    pub drogue_latched: bool,
    pub main_latched: bool,
    pub mass_kg: f64,
    pub cg_from_nose_m: f64,
    pub propellant_kg: f64,
    pub supply_scale: f64,
    pub supply_pressure_pa: f64,
    pub mach: f64,
    pub dynamic_pressure_pa: f64,
    pub angle_of_attack_deg: f64,
    pub static_margin: f64,
    pub wind_mps: [f64; 3],
    pub hinge_moment_nm: [f64; 4],
    pub rcs_force_body_n: [f64; 3],
    pub rcs_torque_body_nm: [f64; 3],
    pub valve_edge_count: u32,
    pub checksums: [u32; 8],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kmr9Recording {
    pub schema: String,
    pub placement: Phase95Placement,
    pub releases: u32,
    pub updates: Vec<Phase95Update>,
    pub terminal_checksums: [u32; 8],
}

pub trait Phase95Sink {
    fn publish(&mut self, update: &Phase95Update);
    fn finish(&mut self, _evidence: &Phase95SplitEvidence) {}
}

pub struct Phase95RecordingSink {
    placement: Phase95Placement,
    updates: Vec<Phase95Update>,
}
impl Phase95RecordingSink {
    pub const fn new(placement: Phase95Placement) -> Self {
        Self {
            placement,
            updates: Vec::new(),
        }
    }
    pub fn recording(&self, evidence: &Phase95SplitEvidence) -> Kmr9Recording {
        let terminal = self
            .updates
            .last()
            .map_or([0; 8], |update| update.checksums);
        Kmr9Recording {
            schema: "ksa64.kmr9-v1".into(),
            placement: self.placement,
            releases: evidence.releases,
            updates: self.updates.clone(),
            terminal_checksums: terminal,
        }
    }
    pub fn updates(&self) -> &[Phase95Update] {
        &self.updates
    }
}
impl Phase95Sink for Phase95RecordingSink {
    fn publish(&mut self, update: &Phase95Update) {
        self.updates.push(update.clone())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase95SplitEvidence {
    pub placement: Phase95Placement,
    pub releases: u32,
    pub sensor_checksum: u32,
    pub command_checksum: u32,
    pub status_checksum: u32,
    pub truth_checksum: u32,
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
    pub allocator_checksum: u32,
}

#[derive(Debug)]
pub enum Phase95LinkError {
    Io(io::Error),
    Codec,
    Protocol,
    CommandMismatch(Box<AdvancedCommandCell>, Box<AdvancedCommandCell>),
    StatusMismatch(Box<AdvancedStatusCell>, Box<AdvancedStatusCell>),
    World,
}
impl From<io::Error> for Phase95LinkError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

struct OwnedFrame {
    record: LinkRecordType,
    sequence: u32,
    measurement: u32,
    production: u32,
    effective: u32,
    payload: Vec<u8>,
}

fn hash_bytes(mut hash: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        hash = (hash ^ u32::from(*byte)).wrapping_mul(16_777_619);
    }
    hash
}

fn send_frame<S: Write>(
    stream: &mut S,
    record: LinkRecordType,
    sequence: u32,
    measurement: u32,
    production: u32,
    effective: u32,
    payload: &[u8],
) -> Result<(), Phase95LinkError> {
    let frame = LinkFrame {
        header: LinkHeader {
            record_type: record,
            flags: 0,
            session_id: LINK_SESSION,
            sequence,
            acknowledgement: KLF6_NONE,
            measurement_epoch: measurement,
            production_epoch: production,
            effective_epoch: effective,
        },
        payload,
    };
    let mut decoded = [0; KLF6_MAX_DECODED];
    let mut encoded = [0; KLF6_MAX_ENCODED];
    let length = write_link_frame(&frame, &mut decoded, &mut encoded)
        .map_err(|_| Phase95LinkError::Codec)?;
    stream.write_all(&encoded[..length])?;
    stream.flush()?;
    Ok(())
}

fn receive_frame<S: Read>(stream: &mut S) -> Result<OwnedFrame, Phase95LinkError> {
    let mut encoded = [0; KLF6_MAX_ENCODED];
    let mut length = 0usize;
    loop {
        if length == encoded.len() {
            return Err(Phase95LinkError::Codec);
        }
        stream.read_exact(&mut encoded[length..length + 1])?;
        let done = encoded[length] == 0;
        length += 1;
        if done {
            break;
        }
    }
    let mut decoded = [0; KLF6_MAX_DECODED];
    let frame =
        parse_link_frame(&encoded[..length], &mut decoded).map_err(|_| Phase95LinkError::Codec)?;
    if frame.header.session_id != LINK_SESSION {
        return Err(Phase95LinkError::Protocol);
    }
    Ok(OwnedFrame {
        record: frame.header.record_type,
        sequence: frame.header.sequence,
        measurement: frame.header.measurement_epoch,
        production: frame.header.production_epoch,
        effective: frame.header.effective_epoch,
        payload: frame.payload.to_vec(),
    })
}

fn reference_packs() -> Result<
    (
        SpatialVehiclePack,
        AdvancedEffectorPack,
        PriorityResidualAllocatorPack,
    ),
    Phase95LinkError,
> {
    let vehicle =
        parse_spatial_vehicle_pack(include_bytes!("../../phase9_5/examples/firestorm-m9.kvp8"))
            .map_err(|_| Phase95LinkError::Protocol)?;
    let effectors =
        parse_effector_pack(include_bytes!("../../phase9_5/examples/firestorm-m9.kpe9"))
            .map_err(|_| Phase95LinkError::Protocol)?;
    let allocator =
        parse_allocator_pack(include_bytes!("../../phase9_5/examples/firestorm-m9.kpa9"))
            .map_err(|_| Phase95LinkError::Protocol)?;
    Ok((vehicle, effectors, allocator))
}

fn capabilities() -> Result<LinkCapabilities, Phase95LinkError> {
    let (vehicle, _, _) = reference_packs()?;
    Ok(LinkCapabilities {
        role: EndpointRole::Flight,
        mode: LinkMode::ExactPaced,
        flags: CAP_EXACT_PACED | CAP_MISSION_CONTROL | CAP_TRANSCRIPT,
        link_contract_id: PHASE6_LINK_CONTRACT_ID,
        vehicle_contract_id: vehicle.identity,
        avionics_contract_id: KLR9_CONTRACT_ID,
        max_payload: 512,
        fast_hz: 32,
        navigation_hz: 8,
        guidance_hz: 1,
    })
}

fn finalist_capabilities() -> LinkCapabilities {
    LinkCapabilities {
        role: EndpointRole::Flight,
        mode: LinkMode::ExactPaced,
        flags: CAP_EXACT_PACED | CAP_MISSION_CONTROL | CAP_TRANSCRIPT,
        link_contract_id: PHASE6_LINK_CONTRACT_ID,
        vehicle_contract_id: KFB9_CONTRACT_ID,
        avionics_contract_id: KLR9_CONTRACT_ID,
        max_payload: 512,
        fast_hz: 32,
        navigation_hz: 8,
        guidance_hz: 1,
    }
}

struct AdvancedSplitScenario {
    reference: AdvancedReference,
    capability: ActuatorCapabilityPack,
    faults: AdvancedMissionFaults,
    variation: SpatialMissionVariation,
    bootstrap: AdvancedFlightBootstrap,
}

fn bootstrap_for_reference(
    reference: &AdvancedReference,
    manifest_identity: u32,
    study_identity: u32,
    candidate_identity: u32,
) -> Result<AdvancedFlightBootstrap, Phase95LinkError> {
    let flight = advanced_flight_config(
        reference.vehicle.identity,
        &reference.motor,
        reference_avionics_profile(false),
        &reference.effectors,
        &reference.allocator,
    )
    .map_err(|_| Phase95LinkError::Protocol)?;
    let allocator = allocator_config(&reference.allocator, &reference.effectors, [910; 2])
        .map_err(|_| Phase95LinkError::Protocol)?;
    let mut numeric = ksa64_core::numeric::NumericStatus::CLEAR;
    let axis = ksa64_core::phase8_world::rail_axis_from_mission(reference.mission, &mut numeric)
        .map_err(|_| Phase95LinkError::Protocol)?;
    let attitude = ksa64_core::phase8_world::attitude_from_rail_axis(axis, &mut numeric)
        .map_err(|_| Phase95LinkError::Protocol)?;
    Ok(AdvancedFlightBootstrap {
        manifest_identity,
        study_identity,
        candidate_identity,
        vehicle_identity: reference.vehicle.identity,
        effector_identity: reference.effectors.identity,
        allocator_identity: reference.allocator.identity,
        flight,
        allocator,
        initial_position_q13: [0; 3],
        attitude_target: [
            (attitude.x() >> 15) as i16,
            (attitude.y() >> 15) as i16,
            (attitude.z() >> 15) as i16,
        ],
    })
}

fn reference_split_scenario() -> Result<AdvancedSplitScenario, Phase95LinkError> {
    let accepted = checked_in_reference(false).map_err(|_| Phase95LinkError::Protocol)?;
    let (vehicle, effectors, allocator) = reference_packs()?;
    let mut mission = accepted.mission;
    mission.vehicle_identity = vehicle.identity;
    let reference = AdvancedReference {
        vehicle,
        motor: accepted.motor,
        mission,
        wind: accepted.wind,
        effectors,
        allocator,
    };
    let bootstrap = bootstrap_for_reference(&reference, 1, 1, 1)?;
    Ok(AdvancedSplitScenario {
        capability: reference_gimbal_capability(reference.vehicle.identity),
        reference,
        faults: AdvancedMissionFaults::NOMINAL,
        variation: SpatialMissionVariation::NOMINAL,
        bootstrap,
    })
}

fn finalist_split_scenario(
    manifest: &SearchManifest,
    design: &DesignVector,
    study: AdvancedStudyId,
) -> Result<AdvancedSplitScenario, Phase95LinkError> {
    let reference = materialize_advanced_candidate(manifest, design, study)
        .map_err(|_| Phase95LinkError::Protocol)?;
    let bootstrap =
        bootstrap_for_reference(&reference, manifest.identity, study.raw(), design.identity)?;
    Ok(AdvancedSplitScenario {
        capability: reference_capability(reference.vehicle.identity, &reference.allocator),
        reference,
        faults: AdvancedMissionFaults::NOMINAL,
        variation: SpatialMissionVariation::NOMINAL,
        bootstrap,
    })
}

fn flight_from_bootstrap(
    bootstrap: &AdvancedFlightBootstrap,
) -> Result<AllocatedAdvancedFlightComputer, Phase95LinkError> {
    let base = AdvancedFlightComputer::new(
        bootstrap.flight,
        bootstrap.initial_position_q13,
        bootstrap.attitude_target,
    )
    .ok_or(Phase95LinkError::Protocol)?;
    AllocatedAdvancedFlightComputer::new(base, bootstrap.allocator)
        .ok_or(Phase95LinkError::Protocol)
}

fn new_flight() -> Result<AllocatedAdvancedFlightComputer, Phase95LinkError> {
    let scenario = reference_split_scenario()?;
    flight_from_bootstrap(&scenario.bootstrap)
}

fn serve_flight_endpoint<S: Read + Write>(
    stream: &mut S,
    mut flight: AllocatedAdvancedFlightComputer,
) -> Result<(), Phase95LinkError> {
    let mut aid: Option<AdvancedAidCell> = None;
    loop {
        let frame = receive_frame(stream)?;
        match frame.record {
            LinkRecordType::CanonicalSensor
                if frame.payload.len() == ADVANCED_AID_LENGTH
                    && frame.payload.get(3) == Some(&3) =>
            {
                aid =
                    Some(parse_advanced_aid(&frame.payload).map_err(|_| Phase95LinkError::Codec)?);
            }
            LinkRecordType::CanonicalSensor
                if frame.payload.len() == ADVANCED_FAST_SENSOR_LENGTH
                    && frame.payload.get(3) == Some(&1) =>
            {
                let fast = parse_advanced_fast_sensor(&frame.payload)
                    .map_err(|_| Phase95LinkError::Codec)?;
                let out = flight.tick(Some(fast), aid.take());
                let mut command = [0; ADVANCED_COMMAND_LENGTH];
                write_advanced_command(&out.command, &mut command)
                    .map_err(|_| Phase95LinkError::Codec)?;
                let epoch = u32::from(fast.measurement_epoch);
                send_frame(
                    stream,
                    LinkRecordType::CanonicalCommand,
                    epoch * 2 + 1,
                    epoch,
                    epoch,
                    epoch + 1,
                    &command,
                )?;
                if let Some(status) = out.status {
                    let mut bytes = [0; ADVANCED_STATUS_LENGTH];
                    write_advanced_status(&status, &mut bytes)
                        .map_err(|_| Phase95LinkError::Codec)?;
                    send_frame(
                        stream,
                        LinkRecordType::CanonicalTelemetry,
                        epoch * 2 + 2,
                        epoch,
                        epoch,
                        KLF6_NONE,
                        &bytes,
                    )?;
                }
            }
            LinkRecordType::Stop => return Ok(()),
            _ => return Err(Phase95LinkError::Protocol),
        }
    }
}

/// Native protocol oracle for the frozen stock C64 flight endpoint.
pub fn run_native_flight_endpoint<S: Read + Write>(stream: &mut S) -> Result<(), Phase95LinkError> {
    let mut cap_bytes = [0; CAPABILITY_PAYLOAD_LENGTH];
    write_capabilities(&capabilities()?, &mut cap_bytes).map_err(|_| Phase95LinkError::Codec)?;
    send_frame(stream, LinkRecordType::Capabilities, 0, 0, 0, 0, &cap_bytes)?;
    let start = receive_frame(stream)?;
    if start.record != LinkRecordType::Start || !start.payload.is_empty() {
        return Err(Phase95LinkError::Protocol);
    }
    serve_flight_endpoint(stream, new_flight()?)
}

/// Native oracle for the additive KFB9 selected-finalist endpoint.
pub fn run_native_finalist_flight_endpoint<S: Read + Write>(
    stream: &mut S,
) -> Result<(), Phase95LinkError> {
    let mut cap_bytes = [0; CAPABILITY_PAYLOAD_LENGTH];
    write_capabilities(&finalist_capabilities(), &mut cap_bytes)
        .map_err(|_| Phase95LinkError::Codec)?;
    send_frame(stream, LinkRecordType::Capabilities, 0, 0, 0, 0, &cap_bytes)?;
    let start = receive_frame(stream)?;
    if start.record != LinkRecordType::Start || start.payload.len() != KFB9_LENGTH {
        return Err(Phase95LinkError::Protocol);
    }
    let bootstrap = parse_flight_bootstrap(&start.payload).map_err(|_| Phase95LinkError::Codec)?;
    serve_flight_endpoint(stream, flight_from_bootstrap(&bootstrap)?)
}

fn q13(value: i32) -> f64 {
    f64::from(value) / 8_192.0
}
fn q19(value: i32) -> f64 {
    f64::from(value) / 524_288.0
}
fn q12(value: i32) -> f64 {
    f64::from(value) / 4_096.0
}
fn make_update(
    release: &ksa64_sim::phase9_5_mission::AdvancedWorldRelease,
    out: &AllocatedFlightEvidence,
    aid_validity: u16,
    valve_edge_count: u32,
    checksums: [u32; 8],
) -> Phase95Update {
    let snapshot = release.director.snapshot;
    let state = snapshot.state;
    let truth_position = [
        q13(state.position.x()),
        q13(state.position.y()),
        q13(state.position.z()),
    ];
    let truth_velocity = [
        q19(state.velocity.x()),
        q19(state.velocity.y()),
        q19(state.velocity.z()),
    ];
    let ground_position = truth_position.map(|value| (value * 10.0).round() / 10.0);
    let ground_velocity = truth_velocity.map(|value| (value * 100.0).round() / 100.0);
    let status = out.status;
    Phase95Update {
        epoch: release.fast.measurement_epoch,
        time_s: f64::from(state.time.raw()) / 262_144.0,
        phase: snapshot.phase as u8,
        events: snapshot.events,
        truth_position_m: truth_position,
        truth_velocity_mps: truth_velocity,
        onboard_position_m: out.base.local.navigation.position_q13.map(q13),
        onboard_velocity_mps: out.base.local.navigation.velocity_q19.map(q19),
        ground_position_m: ground_position,
        ground_velocity_mps: ground_velocity,
        attitude: [
            state.attitude.w(),
            state.attitude.x(),
            state.attitude.y(),
            state.attitude.z(),
        ]
        .map(|value| f64::from(value) / 1_073_741_824.0),
        angular_rate_rad_s: [
            state.angular_rate.x(),
            state.angular_rate.y(),
            state.angular_rate.z(),
        ]
        .map(|value| f64::from(value) / 16_777_216.0),
        requested_torque_nm: out.allocation.requested_q12.map(q12),
        achieved_torque_nm: out.allocation.achieved_q12.map(q12),
        residual_torque_nm: out.allocation.residual_q12.map(q12),
        commanded_gimbal: out.command.gimbal,
        applied_gimbal: release.fast.gimbal_applied,
        commanded_canards: out.command.canards,
        applied_canards: release.fast.canard_applied,
        rcs_pulse_quanta: out.command.rcs_pulse_quanta,
        valve_open_mask: release.fast.valve_open_mask,
        authority_state: out.allocation.authority_state,
        saturation_count: out.allocation.saturation_count,
        air_data_source: out.base.air_data.source as u8,
        sensor_validity: release.fast.validity,
        aid_validity,
        command_flags: out.command.flags,
        command_discrete: out.command.discrete,
        alarms: out.base.local.alarms,
        deadline_misses: status.map_or(out.base.local.deadline_misses, |value| {
            value.deadline_misses
        }),
        safe: out.base.local.safe,
        armed: out.base.local.armed,
        drogue_latched: out.base.local.drogue_latched,
        main_latched: out.base.local.main_latched,
        mass_kg: f64::from(snapshot.mass.mass.raw()) / 2_097_152.0,
        cg_from_nose_m: f64::from(snapshot.mass.cg_from_nose.raw()) / 268_435_456.0,
        propellant_kg: f64::from(release.fast.propellant_q21) / 2_097_152.0,
        supply_scale: f64::from(release.fast.supply_scale_q15) / 32_768.0,
        supply_pressure_pa: f64::from(release.director.physical_feedback.rcs_pressure_q8) / 256.0,
        mach: f64::from(release.fast.mach_q12) / 4_096.0,
        dynamic_pressure_pa: f64::from(release.fast.dynamic_pressure_q10) / 1_024.0,
        angle_of_attack_deg: f64::from(snapshot.aero.angle_of_attack_q28) / 268_435_456.0 * 180.0
            / std::f64::consts::PI,
        static_margin: f64::from(snapshot.aero.static_margin_q24) / 16_777_216.0,
        wind_mps: snapshot
            .wind_q22
            .map(|value| f64::from(value) / 4_194_304.0),
        hinge_moment_nm: release
            .director
            .physical_feedback
            .canard_hinge_q24
            .map(|value| f64::from(value) / 16_777_216.0),
        rcs_force_body_n: release
            .director
            .physical_feedback
            .rcs_force_body_q23
            .map(|value| f64::from(value) / 8_388_608.0),
        rcs_torque_body_nm: release
            .director
            .physical_feedback
            .rcs_torque_body_q12
            .map(q12),
        valve_edge_count,
        checksums,
    }
}

/// Run a finite or complete host-world mission against an externally paced flight endpoint.
fn run_host_split_scenario<S: Read + Write>(
    stream: &mut S,
    max_releases: u32,
    mut sink: Option<&mut dyn Phase95Sink>,
    placement: Phase95Placement,
    scenario: AdvancedSplitScenario,
    expected_cap: LinkCapabilities,
    start_payload: &[u8],
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    let cap_frame = receive_frame(stream)?;
    if cap_frame.record != LinkRecordType::Capabilities || cap_frame.sequence != 0 {
        return Err(Phase95LinkError::Protocol);
    }
    let cap = parse_capabilities(&cap_frame.payload).map_err(|_| Phase95LinkError::Codec)?;
    if cap != expected_cap {
        return Err(Phase95LinkError::Protocol);
    }
    send_frame(stream, LinkRecordType::Start, 0, 0, 0, 0, start_payload)?;

    let reference = &scenario.reference;
    let mut world = AdvancedWorldEndpoint::new(
        &reference.vehicle,
        &reference.motor,
        reference.mission,
        &reference.wind,
        scenario.variation,
        scenario.capability,
        &reference.effectors,
        scenario.faults,
    )
    .map_err(|_| Phase95LinkError::World)?;
    let mut shadow = flight_from_bootstrap(&scenario.bootstrap)?;
    let mut releases = 0u32;
    let mut sensor_checksum = FNV_OFFSET;
    let mut command_checksum = FNV_OFFSET;
    let mut status_checksum = FNV_OFFSET;
    let mut navigation_checksum = FNV_OFFSET;
    let mut flight_checksum = FNV_OFFSET;
    let mut allocator_checksum = FNV_OFFSET;
    let mut truth_checksum = FNV_OFFSET;

    while !world.is_complete() && releases < max_releases {
        let Some(release) = world.release().map_err(|_| Phase95LinkError::World)? else {
            break;
        };
        let epoch = u32::from(release.fast.measurement_epoch);
        if let Some(aid) = release.aid {
            let mut bytes = [0; ADVANCED_AID_LENGTH];
            write_advanced_aid(&aid, &mut bytes).map_err(|_| Phase95LinkError::Codec)?;
            sensor_checksum = hash_bytes(sensor_checksum, &bytes);
            send_frame(
                stream,
                LinkRecordType::CanonicalSensor,
                epoch * 2,
                epoch,
                epoch,
                KLF6_NONE,
                &bytes,
            )?;
        }
        let mut fast_bytes = [0; ADVANCED_FAST_SENSOR_LENGTH];
        write_advanced_fast_sensor(&release.fast, &mut fast_bytes)
            .map_err(|_| Phase95LinkError::Codec)?;
        sensor_checksum = hash_bytes(sensor_checksum, &fast_bytes);
        send_frame(
            stream,
            LinkRecordType::CanonicalSensor,
            epoch * 2 + 1,
            epoch,
            epoch,
            KLF6_NONE,
            &fast_bytes,
        )?;

        let expected = shadow.tick(Some(release.fast), release.aid);
        let command_frame = receive_frame(stream)?;
        if command_frame.record != LinkRecordType::CanonicalCommand
            || command_frame.sequence != epoch * 2 + 1
            || command_frame.measurement != epoch
            || command_frame.production != epoch
            || command_frame.effective != epoch + 1
        {
            return Err(Phase95LinkError::Protocol);
        }
        let command =
            parse_advanced_command(&command_frame.payload).map_err(|_| Phase95LinkError::Codec)?;
        if command != expected.command {
            return Err(Phase95LinkError::CommandMismatch(
                Box::new(command),
                Box::new(expected.command),
            ));
        }
        command_checksum = hash_bytes(command_checksum, &command_frame.payload);

        if let Some(expected_status) = expected.status {
            let status_frame = receive_frame(stream)?;
            if status_frame.record != LinkRecordType::CanonicalTelemetry
                || status_frame.sequence != epoch * 2 + 2
                || status_frame.measurement != epoch
                || status_frame.production != epoch
                || status_frame.effective != KLF6_NONE
            {
                return Err(Phase95LinkError::Protocol);
            }
            let status = parse_advanced_status(&status_frame.payload)
                .map_err(|_| Phase95LinkError::Codec)?;
            if status != expected_status {
                return Err(Phase95LinkError::StatusMismatch(
                    Box::new(status),
                    Box::new(expected_status),
                ));
            }
            status_checksum = hash_bytes(status_checksum, &status_frame.payload);
        }
        let checksums = [
            release.director.truth_checksum,
            sensor_checksum,
            expected.base.local.navigation.checksum,
            expected.base.demand_checksum,
            command_checksum,
            expected.allocator_checksum,
            status_checksum,
            expected.base.command_checksum,
        ];
        let update = make_update(
            &release,
            &expected,
            release.aid.map_or(0, |aid| aid.validity),
            world.valve_edge_count(),
            checksums,
        );
        if let Some(target) = sink.as_deref_mut() {
            target.publish(&update);
        }
        world
            .accept_command(command)
            .map_err(|_| Phase95LinkError::World)?;
        truth_checksum = release.director.truth_checksum;
        navigation_checksum = expected.base.local.navigation.checksum;
        flight_checksum = expected.base.local.flight_checksum;
        allocator_checksum = expected.allocator_checksum;
        releases = releases.saturating_add(1);
    }
    send_frame(
        stream,
        LinkRecordType::Stop,
        u32::MAX - 1,
        KLF6_NONE,
        KLF6_NONE,
        KLF6_NONE,
        &[],
    )?;
    let evidence = Phase95SplitEvidence {
        placement,
        releases,
        sensor_checksum,
        command_checksum,
        status_checksum,
        truth_checksum,
        navigation_checksum,
        flight_checksum,
        allocator_checksum,
    };
    if let Some(target) = sink {
        target.finish(&evidence);
    }
    Ok(evidence)
}

pub fn run_host_external_with_limit_observed<S: Read + Write>(
    stream: &mut S,
    max_releases: u32,
    sink: Option<&mut dyn Phase95Sink>,
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    run_host_split_scenario(
        stream,
        max_releases,
        sink,
        Phase95Placement::HostExternalFlight,
        reference_split_scenario()?,
        capabilities()?,
        &[],
    )
}

pub fn run_host_native_with_limit_observed(
    max_releases: u32,
    sink: Option<&mut dyn Phase95Sink>,
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let endpoint = thread::spawn(move || -> Result<(), Phase95LinkError> {
        let (mut stream, _) = listener.accept()?;
        run_native_flight_endpoint(&mut stream)
    });
    let mut stream = TcpStream::connect(address)?;
    stream.set_nodelay(true)?;
    let result = run_host_split_scenario(
        &mut stream,
        max_releases,
        sink,
        Phase95Placement::HostHost,
        reference_split_scenario()?,
        capabilities()?,
        &[],
    );
    let endpoint_result = endpoint.join().map_err(|_| Phase95LinkError::Protocol)?;
    endpoint_result?;
    result
}

fn run_finalist_split<S: Read + Write>(
    stream: &mut S,
    manifest: &SearchManifest,
    design: &DesignVector,
    study: AdvancedStudyId,
    max_releases: u32,
    sink: Option<&mut dyn Phase95Sink>,
    placement: Phase95Placement,
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    let scenario = finalist_split_scenario(manifest, design, study)?;
    let mut payload = [0; KFB9_LENGTH];
    write_flight_bootstrap(&scenario.bootstrap, &mut payload)
        .map_err(|_| Phase95LinkError::Codec)?;
    run_host_split_scenario(
        stream,
        max_releases,
        sink,
        placement,
        scenario,
        finalist_capabilities(),
        &payload,
    )
}

pub fn run_host_external_finalist_with_limit_observed<S: Read + Write>(
    stream: &mut S,
    manifest: &SearchManifest,
    design: &DesignVector,
    study: AdvancedStudyId,
    max_releases: u32,
    sink: Option<&mut dyn Phase95Sink>,
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    run_finalist_split(
        stream,
        manifest,
        design,
        study,
        max_releases,
        sink,
        Phase95Placement::HostExternalFlight,
    )
}

pub fn run_host_native_finalist_with_limit_observed(
    manifest: &SearchManifest,
    design: &DesignVector,
    study: AdvancedStudyId,
    max_releases: u32,
    sink: Option<&mut dyn Phase95Sink>,
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let endpoint = thread::spawn(move || -> Result<(), Phase95LinkError> {
        let (mut stream, _) = listener.accept()?;
        run_native_finalist_flight_endpoint(&mut stream)
    });
    let mut stream = TcpStream::connect(address)?;
    stream.set_nodelay(true)?;
    let result = run_finalist_split(
        &mut stream,
        manifest,
        design,
        study,
        max_releases,
        sink,
        Phase95Placement::HostHost,
    );
    let endpoint_result = endpoint.join().map_err(|_| Phase95LinkError::Protocol)?;
    endpoint_result?;
    result
}

pub fn run_host_external_with_limit<S: Read + Write>(
    stream: &mut S,
    max_releases: u32,
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    run_host_external_with_limit_observed(stream, max_releases, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn native_external_split_is_exact_for_bounded_releases() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            run_native_flight_endpoint(&mut stream).unwrap();
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let evidence = run_host_external_with_limit(&mut stream, 32).unwrap();
        endpoint.join().unwrap();
        assert_eq!(evidence.releases, 32);
        assert_ne!(evidence.sensor_checksum, FNV_OFFSET);
        assert_ne!(evidence.command_checksum, FNV_OFFSET);
        assert_ne!(evidence.status_checksum, FNV_OFFSET);
        assert_ne!(evidence.allocator_checksum, FNV_OFFSET);
    }
    struct CountSink(u32);
    impl Phase95Sink for CountSink {
        fn publish(&mut self, _: &Phase95Update) {
            self.0 = self.0.saturating_add(1);
        }
    }
    #[test]
    fn passive_observation_cannot_change_split_results() {
        let plain = run_host_native_with_limit_observed(64, None).unwrap();
        let mut sink = CountSink(0);
        let observed = run_host_native_with_limit_observed(64, Some(&mut sink)).unwrap();
        assert_eq!(plain, observed);
        assert_eq!(sink.0, observed.releases);
        assert_eq!(observed.placement, Phase95Placement::HostHost);
    }
    #[test]
    fn selected_canard_rcs_and_mixed_finalists_match_native_endpoint() {
        use crate::phase9_5_archive::AdvancedFinalistPackage;
        use crate::phase9_5_workbench::built_in_advanced_manifest;
        use ksa64_core::phase9_contract::SearchEngineId;
        let cases: [(&[u8], AdvancedStudyId); 3] = [
            (
                include_bytes!("../../phase9_5/evidence/workbench/canard-nsga2.kfe9"),
                AdvancedStudyId::Canard,
            ),
            (
                include_bytes!("../../phase9_5/evidence/workbench/rcs-nsga2.kfe9"),
                AdvancedStudyId::Rcs,
            ),
            (
                include_bytes!("../../phase9_5/evidence/workbench/mixed-nsga2.kfe9"),
                AdvancedStudyId::Mixed,
            ),
        ];
        for (bytes, study) in cases {
            let package = AdvancedFinalistPackage::parse(bytes).unwrap();
            let design = package.record(0).unwrap().design;
            let manifest = built_in_advanced_manifest(study, SearchEngineId::Nsga2V1);
            assert_eq!(manifest.identity, package.manifest_identity);
            let evidence =
                run_host_native_finalist_with_limit_observed(&manifest, &design, study, 8, None)
                    .unwrap();
            assert_eq!(evidence.releases, 8);
            assert_ne!(evidence.command_checksum, FNV_OFFSET);
        }
    }
}
