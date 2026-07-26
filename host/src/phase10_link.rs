//! KLF6-framed Phase 10 host-world / externally paced flight placement.

use crate::phase10::GlobalFixtureSet;
use ksa64_flight::phase10::{
    ksa_g10r_reference_flight_config, GlobalFlightComputer, GlobalFlightEvidence,
};
use ksa64_interface::phase10::{
    parse_global_command, parse_global_status, write_global_aid_frame, write_global_command,
    write_global_fast_sensor, write_global_status, write_global_transition, GlobalCommandCell,
    GlobalStatusCell, GlobalTransitionCell, GLOBAL_AID_FRAME_LENGTH, GLOBAL_COMMAND_LENGTH,
    GLOBAL_FAST_SENSOR_LENGTH, GLOBAL_STATUS_LENGTH, GLOBAL_TRANSITION_LENGTH, KLR10_CONTRACT_ID,
};
use ksa64_interface::phase6::{
    parse_capabilities, parse_link_frame, write_capabilities, write_link_frame, EndpointRole,
    LinkCapabilities, LinkFrame, LinkHeader, LinkMode, LinkRecordType, CAPABILITY_PAYLOAD_LENGTH,
    CAP_EXACT_PACED, CAP_MISSION_CONTROL, CAP_TRANSCRIPT, KLF6_MAX_DECODED, KLF6_MAX_ENCODED,
    KLF6_NONE, PHASE6_LINK_CONTRACT_ID,
};
use ksa64_sim::phase10::{GlobalWorldError, GlobalWorldMachine};
use ksa64_sim::phase10_avionics::{
    reference_global_flight_config, GlobalAvionicsMission, GlobalReleaseBundle, GlobalSensorFaults,
};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

const LINK_SESSION: u32 = 0x4b4c_523a;
const NOMINAL_SESSION: u16 = 0x10a0;
const NOMINAL_SEED: u32 = 0x4b53_41a0;
const FNV_OFFSET: u32 = 0x811c_9dc5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase10Placement {
    HostHost,
    HostExternalFlight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase10SplitEvidence {
    pub placement: Phase10Placement,
    pub releases: u32,
    pub transition_mask: u8,
    pub sensor_checksum: u32,
    pub command_checksum: u32,
    pub status_checksum: u32,
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
}

#[derive(Debug)]
pub enum Phase10LinkError {
    Io(io::Error),
    Codec,
    Protocol,
    World(GlobalWorldError),
    CommandMismatch(Box<GlobalCommandCell>, Box<GlobalCommandCell>),
    StatusMismatch(Box<GlobalStatusCell>, Box<GlobalStatusCell>),
}

impl From<io::Error> for Phase10LinkError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<GlobalWorldError> for Phase10LinkError {
    fn from(value: GlobalWorldError) -> Self {
        Self::World(value)
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

fn capabilities() -> LinkCapabilities {
    LinkCapabilities {
        role: EndpointRole::Flight,
        mode: LinkMode::ExactPaced,
        flags: CAP_EXACT_PACED | CAP_MISSION_CONTROL | CAP_TRANSCRIPT,
        link_contract_id: PHASE6_LINK_CONTRACT_ID,
        vehicle_contract_id: 0x4756_1001,
        avionics_contract_id: KLR10_CONTRACT_ID,
        max_payload: 512,
        fast_hz: 32,
        navigation_hz: 8,
        guidance_hz: 1,
    }
}

fn send_frame<S: Write>(
    stream: &mut S,
    record: LinkRecordType,
    sequence: u32,
    measurement: u32,
    production: u32,
    effective: u32,
    payload: &[u8],
) -> Result<(), Phase10LinkError> {
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
        .map_err(|_| Phase10LinkError::Codec)?;
    stream.write_all(&encoded[..length])?;
    stream.flush()?;
    Ok(())
}

fn receive_frame<S: Read>(stream: &mut S) -> Result<OwnedFrame, Phase10LinkError> {
    let mut encoded = [0; KLF6_MAX_ENCODED];
    let mut length = 0usize;
    loop {
        if length == encoded.len() {
            return Err(Phase10LinkError::Codec);
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
        parse_link_frame(&encoded[..length], &mut decoded).map_err(|_| Phase10LinkError::Codec)?;
    if frame.header.session_id != LINK_SESSION {
        return Err(Phase10LinkError::Protocol);
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

fn nominal_runner(
    fixtures: &GlobalFixtureSet,
) -> Result<GlobalAvionicsMission<'_>, Phase10LinkError> {
    let world = GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )?;
    let flight =
        reference_global_flight_config(NOMINAL_SESSION, world.active_state()?, fixtures.mission)?;
    Ok(GlobalAvionicsMission::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
        flight,
        GlobalSensorFaults::NONE,
        NOMINAL_SEED,
    )?)
}

fn send_release<S: Read + Write>(
    stream: &mut S,
    index: u32,
    bundle: GlobalReleaseBundle,
    sensor_checksum: &mut u32,
    command_checksum: &mut u32,
    status_checksum: &mut u32,
) -> Result<(), Phase10LinkError> {
    let epoch = bundle.evidence.command.source_epoch;
    let base = index.saturating_mul(5);
    if let Some(transition) = bundle.transition {
        let mut bytes = [0; GLOBAL_TRANSITION_LENGTH];
        write_global_transition(&transition, &mut bytes).map_err(|_| Phase10LinkError::Codec)?;
        *sensor_checksum = hash_bytes(*sensor_checksum, &bytes);
        send_frame(
            stream,
            LinkRecordType::CanonicalSensor,
            base,
            u32::from(epoch),
            u32::from(epoch),
            u32::from(epoch),
            &bytes,
        )?;
    }
    if let Some(aid) = bundle.aid {
        let mut bytes = [0; GLOBAL_AID_FRAME_LENGTH];
        write_global_aid_frame(&aid, &mut bytes).map_err(|_| Phase10LinkError::Codec)?;
        *sensor_checksum = hash_bytes(*sensor_checksum, &bytes);
        send_frame(
            stream,
            LinkRecordType::CanonicalSensor,
            base + 1,
            u32::from(epoch),
            u32::from(epoch),
            KLF6_NONE,
            &bytes,
        )?;
    }
    if let Some(fast) = bundle.fast {
        let mut bytes = [0; GLOBAL_FAST_SENSOR_LENGTH];
        write_global_fast_sensor(&fast, &mut bytes).map_err(|_| Phase10LinkError::Codec)?;
        *sensor_checksum = hash_bytes(*sensor_checksum, &bytes);
        send_frame(
            stream,
            LinkRecordType::CanonicalSensor,
            base + 2,
            u32::from(epoch),
            u32::from(epoch),
            KLF6_NONE,
            &bytes,
        )?;
    }

    let command_frame = receive_frame(stream)?;
    if command_frame.record != LinkRecordType::CanonicalCommand
        || command_frame.sequence != base + 3
        || command_frame.measurement != u32::from(epoch)
        || command_frame.production != u32::from(epoch)
        || command_frame.effective != u32::from(epoch.wrapping_add(1))
    {
        return Err(Phase10LinkError::Protocol);
    }
    let command =
        parse_global_command(&command_frame.payload).map_err(|_| Phase10LinkError::Codec)?;
    if command != bundle.evidence.command {
        return Err(Phase10LinkError::CommandMismatch(
            Box::new(command),
            Box::new(bundle.evidence.command),
        ));
    }
    *command_checksum = hash_bytes(*command_checksum, &command_frame.payload);

    if let Some(expected_status) = bundle.evidence.status {
        let status_frame = receive_frame(stream)?;
        if status_frame.record != LinkRecordType::CanonicalTelemetry
            || status_frame.sequence != base + 4
            || status_frame.measurement != u32::from(epoch)
            || status_frame.production != u32::from(epoch)
            || status_frame.effective != KLF6_NONE
        {
            return Err(Phase10LinkError::Protocol);
        }
        let status =
            parse_global_status(&status_frame.payload).map_err(|_| Phase10LinkError::Codec)?;
        if status != expected_status {
            return Err(Phase10LinkError::StatusMismatch(
                Box::new(status),
                Box::new(expected_status),
            ));
        }
        *status_checksum = hash_bytes(*status_checksum, &status_frame.payload);
    }
    Ok(())
}

fn receive_capabilities<S: Read + Write>(stream: &mut S) -> Result<(), Phase10LinkError> {
    let frame = receive_frame(stream)?;
    if frame.record != LinkRecordType::Capabilities || frame.sequence != 0 {
        return Err(Phase10LinkError::Protocol);
    }
    let received = parse_capabilities(&frame.payload).map_err(|_| Phase10LinkError::Codec)?;
    if received != capabilities() {
        return Err(Phase10LinkError::Protocol);
    }
    send_frame(stream, LinkRecordType::Start, 0, 0, 0, 0, &[])
}

fn finish<S: Write>(
    stream: &mut S,
    evidence: Phase10SplitEvidence,
) -> Result<Phase10SplitEvidence, Phase10LinkError> {
    send_frame(
        stream,
        LinkRecordType::Stop,
        u32::MAX - 1,
        KLF6_NONE,
        KLF6_NONE,
        KLF6_NONE,
        &[],
    )?;
    Ok(evidence)
}

fn run_nominal_split<S: Read + Write>(
    stream: &mut S,
    max_releases: u32,
    placement: Phase10Placement,
) -> Result<Phase10SplitEvidence, Phase10LinkError> {
    receive_capabilities(stream)?;
    let fixtures = GlobalFixtureSet::embedded();
    let mut runner = nominal_runner(&fixtures)?;
    let mut sensor_checksum = FNV_OFFSET;
    let mut command_checksum = FNV_OFFSET;
    let mut status_checksum = FNV_OFFSET;
    let mut transition_mask = 0u8;
    let mut releases = 0u32;
    let mut last = None;
    while releases < max_releases && !runner.world().is_complete() {
        let bundle = runner.release_bundle()?;
        if let Some(transition) = bundle.transition {
            transition_mask |= 1 << transition_index(transition);
        }
        send_release(
            stream,
            releases,
            bundle,
            &mut sensor_checksum,
            &mut command_checksum,
            &mut status_checksum,
        )?;
        last = Some(bundle.evidence);
        releases = releases.saturating_add(1);
        if !runner.world().is_complete() {
            runner.advance_to_next_release()?;
        }
    }
    let last = last.ok_or(Phase10LinkError::Protocol)?;
    finish(
        stream,
        Phase10SplitEvidence {
            placement,
            releases,
            transition_mask,
            sensor_checksum,
            command_checksum,
            status_checksum,
            navigation_checksum: last.navigation.checksum,
            flight_checksum: last.flight_checksum,
        },
    )
}

fn transition_index(cell: GlobalTransitionCell) -> u8 {
    use ksa64_interface::phase10::GlobalFrameId::{
        EarthFixedEcefV1 as Ecef, EarthInertialEciV1 as Eci, LocalEnuV1 as Enu,
    };
    match (cell.from, cell.to) {
        (Enu, Ecef) => 0,
        (Ecef, Eci) => 1,
        (Eci, Ecef) => 2,
        (Ecef, Enu) => 3,
        _ => 7,
    }
}

fn rebased_transition_probe() -> Result<Vec<GlobalReleaseBundle>, Phase10LinkError> {
    let fixtures = GlobalFixtureSet::embedded();
    let mut runner = nominal_runner(&fixtures)?;
    let first = runner.release_bundle()?;
    let mut transitions = Vec::new();
    while transitions.len() < 4 {
        if !runner.world().is_complete() {
            runner.advance_to_next_release()?;
        }
        let bundle = runner.release_bundle()?;
        if bundle.transition.is_some() {
            transitions.push(bundle);
        }
        if runner.world().is_complete() {
            break;
        }
    }
    if transitions.len() != 4 {
        return Err(Phase10LinkError::Protocol);
    }
    let world = GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )?;
    let config =
        reference_global_flight_config(NOMINAL_SESSION, world.active_state()?, fixtures.mission)?;
    let mut flight = GlobalFlightComputer::new(config).ok_or(Phase10LinkError::Protocol)?;
    let mut sources = Vec::with_capacity(5);
    sources.push(first);
    sources.extend(transitions);
    let mut result = Vec::with_capacity(5);
    for (epoch, source) in sources.into_iter().enumerate() {
        let epoch = epoch as u16;
        let fast = source.fast.map(|mut cell| {
            cell.measurement_epoch = epoch;
            cell.production_epoch = epoch;
            cell
        });
        let aid = source.aid.map(|mut cell| {
            cell.measurement_epoch = epoch;
            cell.production_epoch = epoch;
            cell
        });
        let transition = source.transition.map(|mut cell| {
            cell.source_epoch = epoch;
            cell.effective_epoch = epoch;
            cell
        });
        let evidence = flight.tick(fast, aid, transition);
        result.push(GlobalReleaseBundle {
            fast,
            aid,
            transition,
            evidence,
        });
    }
    Ok(result)
}

fn run_transition_split<S: Read + Write>(
    stream: &mut S,
    placement: Phase10Placement,
) -> Result<Phase10SplitEvidence, Phase10LinkError> {
    receive_capabilities(stream)?;
    let bundles = rebased_transition_probe()?;
    let mut sensor_checksum = FNV_OFFSET;
    let mut command_checksum = FNV_OFFSET;
    let mut status_checksum = FNV_OFFSET;
    let mut transition_mask = 0u8;
    let mut last: Option<GlobalFlightEvidence> = None;
    for (index, bundle) in bundles.iter().copied().enumerate() {
        if let Some(transition) = bundle.transition {
            transition_mask |= 1 << transition_index(transition);
        }
        send_release(
            stream,
            index as u32,
            bundle,
            &mut sensor_checksum,
            &mut command_checksum,
            &mut status_checksum,
        )?;
        last = Some(bundle.evidence);
    }
    let last = last.ok_or(Phase10LinkError::Protocol)?;
    finish(
        stream,
        Phase10SplitEvidence {
            placement,
            releases: bundles.len() as u32,
            transition_mask,
            sensor_checksum,
            command_checksum,
            status_checksum,
            navigation_checksum: last.navigation.checksum,
            flight_checksum: last.flight_checksum,
        },
    )
}

pub fn run_host_external_with_limit<S: Read + Write>(
    stream: &mut S,
    max_releases: u32,
) -> Result<Phase10SplitEvidence, Phase10LinkError> {
    run_nominal_split(stream, max_releases, Phase10Placement::HostExternalFlight)
}

pub fn run_host_external_transition_probe<S: Read + Write>(
    stream: &mut S,
) -> Result<Phase10SplitEvidence, Phase10LinkError> {
    run_transition_split(stream, Phase10Placement::HostExternalFlight)
}

pub fn run_native_flight_endpoint<S: Read + Write>(stream: &mut S) -> Result<(), Phase10LinkError> {
    let cap = capabilities();
    let mut payload = [0; CAPABILITY_PAYLOAD_LENGTH];
    write_capabilities(&cap, &mut payload).map_err(|_| Phase10LinkError::Codec)?;
    send_frame(stream, LinkRecordType::Capabilities, 0, 0, 0, 0, &payload)?;
    let start = receive_frame(stream)?;
    if start.record != LinkRecordType::Start || !start.payload.is_empty() {
        return Err(Phase10LinkError::Protocol);
    }
    let mut flight = GlobalFlightComputer::new(ksa_g10r_reference_flight_config())
        .ok_or(Phase10LinkError::Protocol)?;
    let mut aid = None;
    let mut transition = None;
    loop {
        let frame = receive_frame(stream)?;
        if frame.record == LinkRecordType::Stop {
            return Ok(());
        }
        if frame.record != LinkRecordType::CanonicalSensor {
            return Err(Phase10LinkError::Protocol);
        }
        match frame.payload.get(3).copied() {
            Some(1) => {
                let fast = ksa64_interface::phase10::parse_global_fast_sensor(&frame.payload)
                    .map_err(|_| Phase10LinkError::Codec)?;
                let evidence = flight.tick(Some(fast), aid.take(), transition.take());
                let base = frame.sequence.saturating_sub(2);
                let mut command = [0; GLOBAL_COMMAND_LENGTH];
                write_global_command(&evidence.command, &mut command)
                    .map_err(|_| Phase10LinkError::Codec)?;
                send_frame(
                    stream,
                    LinkRecordType::CanonicalCommand,
                    base + 3,
                    u32::from(fast.measurement_epoch),
                    u32::from(fast.measurement_epoch),
                    u32::from(fast.measurement_epoch.wrapping_add(1)),
                    &command,
                )?;
                if let Some(status) = evidence.status {
                    let mut bytes = [0; GLOBAL_STATUS_LENGTH];
                    write_global_status(&status, &mut bytes)
                        .map_err(|_| Phase10LinkError::Codec)?;
                    send_frame(
                        stream,
                        LinkRecordType::CanonicalTelemetry,
                        base + 4,
                        u32::from(fast.measurement_epoch),
                        u32::from(fast.measurement_epoch),
                        KLF6_NONE,
                        &bytes,
                    )?;
                }
            }
            Some(2) => {
                aid = Some(
                    ksa64_interface::phase10::parse_global_aid_frame(&frame.payload)
                        .map_err(|_| Phase10LinkError::Codec)?,
                )
            }
            Some(3) => {
                transition = Some(
                    ksa64_interface::phase10::parse_global_transition(&frame.payload)
                        .map_err(|_| Phase10LinkError::Codec)?,
                )
            }
            _ => return Err(Phase10LinkError::Protocol),
        }
    }
}

pub fn run_host_native_with_limit(
    max_releases: u32,
) -> Result<Phase10SplitEvidence, Phase10LinkError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let endpoint = thread::spawn(move || -> Result<(), Phase10LinkError> {
        let (mut stream, _) = listener.accept()?;
        run_native_flight_endpoint(&mut stream)
    });
    let mut stream = TcpStream::connect(address)?;
    stream.set_nodelay(true)?;
    let result = run_nominal_split(&mut stream, max_releases, Phase10Placement::HostHost);
    endpoint.join().map_err(|_| Phase10LinkError::Protocol)??;
    result
}

pub fn run_host_native_transition_probe() -> Result<Phase10SplitEvidence, Phase10LinkError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let endpoint = thread::spawn(move || -> Result<(), Phase10LinkError> {
        let (mut stream, _) = listener.accept()?;
        run_native_flight_endpoint(&mut stream)
    });
    let mut stream = TcpStream::connect(address)?;
    stream.set_nodelay(true)?;
    let result = run_transition_split(&mut stream, Phase10Placement::HostHost);
    endpoint.join().map_err(|_| Phase10LinkError::Protocol)??;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_split_is_exact_for_all_release_classes() {
        let evidence = run_host_native_with_limit(33).unwrap();
        assert_eq!(evidence.releases, 33);
        assert_ne!(evidence.sensor_checksum, FNV_OFFSET);
        assert_ne!(evidence.command_checksum, FNV_OFFSET);
        assert_ne!(evidence.status_checksum, FNV_OFFSET);
    }

    #[test]
    fn rebased_probe_covers_every_frame_transition() {
        let evidence = run_host_native_transition_probe().unwrap();
        assert_eq!(evidence.releases, 5);
        assert_eq!(evidence.transition_mask, 0x0f);
    }
}
