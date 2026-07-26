//! KLF6-framed Phase 9.5 host-world / externally paced flight placement.
use crate::phase8_5::checked_in_reference;
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_core::phase8_pack::{parse_spatial_vehicle_pack, SpatialVehiclePack};
use ksa64_core::phase9_5_contract::{
    parse_allocator_pack, parse_effector_pack, AdvancedEffectorPack, PriorityResidualAllocatorPack,
};
use ksa64_flight::phase9_5::AdvancedFlightComputer;
use ksa64_flight::phase9_5_allocator::AllocatedAdvancedFlightComputer;
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
use ksa64_sim::phase9_5_mission::{AdvancedMissionFaults, AdvancedWorldEndpoint};
use std::io::{self, Read, Write};

const LINK_SESSION: u32 = 0x4b4c_5239;
const FNV_OFFSET: u32 = 0x811c_9dc5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase95SplitEvidence {
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

fn new_flight() -> Result<AllocatedAdvancedFlightComputer, Phase95LinkError> {
    let reference = checked_in_reference(false).map_err(|_| Phase95LinkError::Protocol)?;
    let (vehicle, effectors, allocator) = reference_packs()?;
    let flight_config = advanced_flight_config(
        vehicle.identity,
        &reference.motor,
        reference_avionics_profile(false),
        &effectors,
        &allocator,
    )
    .map_err(|_| Phase95LinkError::Protocol)?;
    let allocator_config = allocator_config(&allocator, &effectors, [910; 2])
        .map_err(|_| Phase95LinkError::Protocol)?;
    let mut numeric = ksa64_core::numeric::NumericStatus::CLEAR;
    let mission = reference.mission;
    let axis = ksa64_core::phase8_world::rail_axis_from_mission(mission, &mut numeric)
        .map_err(|_| Phase95LinkError::Protocol)?;
    let attitude = ksa64_core::phase8_world::attitude_from_rail_axis(axis, &mut numeric)
        .map_err(|_| Phase95LinkError::Protocol)?;
    let target = [
        (attitude.x() >> 15) as i16,
        (attitude.y() >> 15) as i16,
        (attitude.z() >> 15) as i16,
    ];
    let base = AdvancedFlightComputer::new(flight_config, [0; 3], target)
        .ok_or(Phase95LinkError::Protocol)?;
    AllocatedAdvancedFlightComputer::new(base, allocator_config).ok_or(Phase95LinkError::Protocol)
}

/// Native protocol oracle for the stock C64 flight endpoint.
pub fn run_native_flight_endpoint<S: Read + Write>(stream: &mut S) -> Result<(), Phase95LinkError> {
    let mut cap_bytes = [0; CAPABILITY_PAYLOAD_LENGTH];
    write_capabilities(&capabilities()?, &mut cap_bytes).map_err(|_| Phase95LinkError::Codec)?;
    send_frame(stream, LinkRecordType::Capabilities, 0, 0, 0, 0, &cap_bytes)?;
    if receive_frame(stream)?.record != LinkRecordType::Start {
        return Err(Phase95LinkError::Protocol);
    }
    let mut flight = new_flight()?;
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

/// Run a finite or complete host-world mission against an externally paced flight endpoint.
pub fn run_host_external_with_limit<S: Read + Write>(
    stream: &mut S,
    max_releases: u32,
) -> Result<Phase95SplitEvidence, Phase95LinkError> {
    let cap_frame = receive_frame(stream)?;
    if cap_frame.record != LinkRecordType::Capabilities || cap_frame.sequence != 0 {
        return Err(Phase95LinkError::Protocol);
    }
    let cap = parse_capabilities(&cap_frame.payload).map_err(|_| Phase95LinkError::Codec)?;
    let expected_cap = capabilities()?;
    if cap != expected_cap {
        return Err(Phase95LinkError::Protocol);
    }
    send_frame(stream, LinkRecordType::Start, 0, 0, 0, 0, &[])?;

    let reference = checked_in_reference(false).map_err(|_| Phase95LinkError::Protocol)?;
    let (vehicle, effectors, _) = reference_packs()?;
    let mut mission = reference.mission;
    mission.vehicle_identity = vehicle.identity;
    let motor = &reference.motor;
    let wind = &reference.wind;
    let capability = reference_gimbal_capability(vehicle.identity);
    let mut world = AdvancedWorldEndpoint::new(
        &vehicle,
        motor,
        mission,
        wind,
        SpatialMissionVariation::NOMINAL,
        capability,
        &effectors,
        AdvancedMissionFaults::NOMINAL,
    )
    .map_err(|_| Phase95LinkError::World)?;
    let mut shadow = new_flight()?;
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
    Ok(Phase95SplitEvidence {
        releases,
        sensor_checksum,
        command_checksum,
        status_checksum,
        truth_checksum,
        navigation_checksum,
        flight_checksum,
        allocator_checksum,
    })
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
}
