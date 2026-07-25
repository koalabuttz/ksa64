//! KLF6-framed Phase 8.5 host/external-flight placement.
use crate::phase8_5::{
    checked_in_reference, LocalPlacement, Phase85HostError, Phase85RunEvidence, Phase85Sink,
    Phase85Update,
};
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_flight::phase8_5::LocalFlightComputer;
use ksa64_interface::phase6::{
    parse_capabilities, parse_link_frame, write_capabilities, write_link_frame, EndpointRole,
    LinkCapabilities, LinkFrame, LinkHeader, LinkMode, LinkRecordType, CAPABILITY_PAYLOAD_LENGTH,
    CAP_EXACT_PACED, CAP_MISSION_CONTROL, CAP_TRANSCRIPT, KLF6_MAX_DECODED, KLF6_MAX_ENCODED,
    KLF6_NONE, PHASE6_LINK_CONTRACT_ID,
};
use ksa64_interface::phase8_5::{
    parse_local_aid, parse_local_command, parse_local_inertial, parse_local_status,
    write_local_aid, write_local_command, write_local_inertial, write_local_status, LocalAidCell,
    LocalStatusCell, KLR8_CONTRACT_ID, LOCAL_AID_LENGTH, LOCAL_COMMAND_LENGTH,
    LOCAL_INERTIAL_LENGTH, LOCAL_STATUS_LENGTH,
};
use ksa64_sim::phase8_5::{
    evaluate_with_avionics, local_flight_config, AvionicsEvaluationRequest, LocalAvionicsVariation,
    LocalWorldEndpoint,
};
use std::io::{self, Read, Write};

const LINK_SESSION: u32 = 0x4b4c_5238;
#[derive(Debug)]
pub enum Phase85LinkError {
    Io(io::Error),
    Codec,
    Protocol,
    Host(Phase85HostError),
}
impl From<io::Error> for Phase85LinkError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<Phase85HostError> for Phase85LinkError {
    fn from(value: Phase85HostError) -> Self {
        Self::Host(value)
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
fn send_frame<S: Write>(
    stream: &mut S,
    record: LinkRecordType,
    sequence: u32,
    measurement: u32,
    production: u32,
    effective: u32,
    payload: &[u8],
) -> Result<(), Phase85LinkError> {
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
        .map_err(|_| Phase85LinkError::Codec)?;
    stream.write_all(&encoded[..length])?;
    stream.flush()?;
    Ok(())
}
fn receive_frame<S: Read>(stream: &mut S) -> Result<OwnedFrame, Phase85LinkError> {
    let mut encoded = [0; KLF6_MAX_ENCODED];
    let mut length = 0usize;
    loop {
        if length == encoded.len() {
            return Err(Phase85LinkError::Codec);
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
        parse_link_frame(&encoded[..length], &mut decoded).map_err(|_| Phase85LinkError::Codec)?;
    if frame.header.session_id != LINK_SESSION {
        return Err(Phase85LinkError::Protocol);
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
fn capabilities(vehicle_identity: u32) -> LinkCapabilities {
    LinkCapabilities {
        role: EndpointRole::Flight,
        mode: LinkMode::ExactPaced,
        flags: CAP_EXACT_PACED | CAP_MISSION_CONTROL | CAP_TRANSCRIPT,
        link_contract_id: PHASE6_LINK_CONTRACT_ID,
        vehicle_contract_id: vehicle_identity,
        avionics_contract_id: KLR8_CONTRACT_ID,
        max_payload: 512,
        fast_hz: 32,
        navigation_hz: 8,
        guidance_hz: 1,
    }
}

/// Native endpoint used as the exact protocol oracle for VICE/C64 endpoints.
pub fn run_native_flight_endpoint<S: Read + Write>(
    stream: &mut S,
    gimbal: bool,
) -> Result<(), Phase85LinkError> {
    let reference = checked_in_reference(gimbal)?;
    let config = local_flight_config(reference.avionics, reference.capability, &reference.motor)
        .map_err(Phase85HostError::World)?;
    let mut numeric = ksa64_core::numeric::NumericStatus::CLEAR;
    let axis = ksa64_core::phase8_world::rail_axis_from_mission(reference.mission, &mut numeric)
        .map_err(|_| Phase85LinkError::Protocol)?;
    let q = ksa64_core::phase8_world::attitude_from_rail_axis(axis, &mut numeric)
        .map_err(|_| Phase85LinkError::Protocol)?;
    let mut flight = LocalFlightComputer::new(
        config,
        [0, 0, reference.mission.launch_altitude.raw()],
        [
            (q.x() >> 15) as i16,
            (q.y() >> 15) as i16,
            (q.z() >> 15) as i16,
        ],
    )
    .ok_or(Phase85LinkError::Protocol)?;
    let mut cap_bytes = [0; CAPABILITY_PAYLOAD_LENGTH];
    write_capabilities(&capabilities(reference.vehicle.identity), &mut cap_bytes)
        .map_err(|_| Phase85LinkError::Codec)?;
    send_frame(stream, LinkRecordType::Capabilities, 0, 0, 0, 0, &cap_bytes)?;
    let start = receive_frame(stream)?;
    if start.record != LinkRecordType::Start {
        return Err(Phase85LinkError::Protocol);
    }
    let mut aid: Option<LocalAidCell> = None;
    loop {
        let frame = receive_frame(stream)?;
        match frame.record {
            LinkRecordType::CanonicalSensor if frame.payload.len() == LOCAL_AID_LENGTH => {
                aid = Some(parse_local_aid(&frame.payload).map_err(|_| Phase85LinkError::Codec)?);
            }
            LinkRecordType::CanonicalSensor if frame.payload.len() == LOCAL_INERTIAL_LENGTH => {
                let inertial =
                    parse_local_inertial(&frame.payload).map_err(|_| Phase85LinkError::Codec)?;
                let out = flight.tick(Some(inertial), aid.take());
                let mut command = [0; LOCAL_COMMAND_LENGTH];
                write_local_command(&out.command, &mut command)
                    .map_err(|_| Phase85LinkError::Codec)?;
                send_frame(
                    stream,
                    LinkRecordType::CanonicalCommand,
                    u32::from(inertial.measurement_epoch) * 2 + 1,
                    u32::from(out.command.source_epoch),
                    u32::from(out.command.source_epoch),
                    u32::from(out.command.effective_epoch),
                    &command,
                )?;
                if let Some(status) = out.status {
                    let mut bytes = [0; LOCAL_STATUS_LENGTH];
                    write_local_status(&status, &mut bytes).map_err(|_| Phase85LinkError::Codec)?;
                    send_frame(
                        stream,
                        LinkRecordType::CanonicalTelemetry,
                        u32::from(inertial.measurement_epoch) * 2 + 2,
                        u32::from(status.source_epoch),
                        u32::from(status.production_epoch),
                        KLF6_NONE,
                        &bytes,
                    )?;
                }
            }
            LinkRecordType::Stop => return Ok(()),
            _ => return Err(Phase85LinkError::Protocol),
        }
    }
}

pub fn run_host_external_with_limit<S: Read + Write>(
    stream: &mut S,
    gimbal: bool,
    mut sink: Option<&mut dyn Phase85Sink>,
    max_releases: u32,
) -> Result<Option<Phase85RunEvidence>, Phase85LinkError> {
    let reference = checked_in_reference(gimbal)?;
    let cap_frame = receive_frame(stream)?;
    if cap_frame.record != LinkRecordType::Capabilities || cap_frame.sequence != 0 {
        return Err(Phase85LinkError::Protocol);
    }
    let cap = parse_capabilities(&cap_frame.payload).map_err(|_| Phase85LinkError::Codec)?;
    if cap.role != EndpointRole::Flight
        || cap.mode != LinkMode::ExactPaced
        || cap.link_contract_id != PHASE6_LINK_CONTRACT_ID
        || cap.avionics_contract_id != KLR8_CONTRACT_ID
        || cap.vehicle_contract_id != reference.capability.vehicle_identity
    {
        return Err(Phase85LinkError::Protocol);
    }
    send_frame(stream, LinkRecordType::Start, 0, 0, 0, 0, &[])?;
    let config = local_flight_config(reference.avionics, reference.capability, &reference.motor)
        .map_err(Phase85HostError::World)?;
    let mut world = LocalWorldEndpoint::new(
        &reference.vehicle,
        &reference.motor,
        reference.mission,
        &reference.wind,
        SpatialMissionVariation::NOMINAL,
        reference.capability,
    )
    .map_err(Phase85HostError::World)?;
    let initial = world.snapshot().state;
    let q = initial.attitude;
    let mut shadow = LocalFlightComputer::new(
        config,
        [
            initial.position.x(),
            initial.position.y(),
            initial.position.z(),
        ],
        [
            (q.x() >> 15) as i16,
            (q.y() >> 15) as i16,
            (q.z() >> 15) as i16,
        ],
    )
    .ok_or(Phase85LinkError::Protocol)?;
    let mut releases = 0u32;
    while !world.is_complete() && releases < max_releases {
        let Some(release) = world.release().map_err(Phase85HostError::World)? else {
            break;
        };
        let epoch = u32::from(release.inertial.measurement_epoch);
        if let Some(aid) = release.aid {
            let mut bytes = [0; LOCAL_AID_LENGTH];
            write_local_aid(&aid, &mut bytes).map_err(|_| Phase85LinkError::Codec)?;
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
        let mut bytes = [0; LOCAL_INERTIAL_LENGTH];
        write_local_inertial(&release.inertial, &mut bytes).map_err(|_| Phase85LinkError::Codec)?;
        send_frame(
            stream,
            LinkRecordType::CanonicalSensor,
            epoch * 2 + 1,
            epoch,
            epoch,
            KLF6_NONE,
            &bytes,
        )?;
        let expected = shadow.tick(Some(release.inertial), release.aid);
        let command_frame = receive_frame(stream)?;
        if command_frame.record != LinkRecordType::CanonicalCommand
            || command_frame.sequence != epoch * 2 + 1
            || command_frame.measurement != epoch
            || command_frame.production != epoch
            || command_frame.effective != epoch + 1
        {
            return Err(Phase85LinkError::Protocol);
        }
        let command =
            parse_local_command(&command_frame.payload).map_err(|_| Phase85LinkError::Codec)?;
        if command != expected.command {
            return Err(Phase85LinkError::Protocol);
        }
        let status: Option<LocalStatusCell> = if epoch & 3 == 0 {
            let status_frame = receive_frame(stream)?;
            if status_frame.record != LinkRecordType::CanonicalTelemetry
                || status_frame.sequence != epoch * 2 + 2
                || status_frame.measurement != epoch
                || status_frame.production != epoch
                || status_frame.effective != KLF6_NONE
            {
                return Err(Phase85LinkError::Protocol);
            }
            let value =
                parse_local_status(&status_frame.payload).map_err(|_| Phase85LinkError::Codec)?;
            if Some(value) != expected.status {
                return Err(Phase85LinkError::Protocol);
            }
            Some(value)
        } else {
            None
        };
        world
            .accept_command(command)
            .map_err(Phase85HostError::World)?;
        if let Some(target) = sink.as_deref_mut() {
            let snapshot = release.director.snapshot;
            let update = Phase85Update {
                epoch: epoch as u16,
                time_s: f64::from(snapshot.state.time.raw()) / 262_144.0,
                phase: snapshot.phase as u8,
                events: snapshot.events,
                truth_position_m: [
                    f64::from(snapshot.state.position.x()) / 8192.0,
                    f64::from(snapshot.state.position.y()) / 8192.0,
                    f64::from(snapshot.state.position.z()) / 8192.0,
                ],
                truth_velocity_mps: [
                    f64::from(snapshot.state.velocity.x()) / 524_288.0,
                    f64::from(snapshot.state.velocity.y()) / 524_288.0,
                    f64::from(snapshot.state.velocity.z()) / 524_288.0,
                ],
                onboard_position_m: expected
                    .navigation
                    .position_q13
                    .map(|v| f64::from(v) / 8192.0),
                onboard_velocity_mps: expected
                    .navigation
                    .velocity_q19
                    .map(|v| f64::from(v) / 524_288.0),
                ground_position_m: [
                    f64::from(snapshot.state.position.x()) / 8192.0,
                    f64::from(snapshot.state.position.y()) / 8192.0,
                    f64::from(snapshot.state.position.z()) / 8192.0,
                ],
                ground_velocity_mps: [
                    f64::from(snapshot.state.velocity.x()) / 524_288.0,
                    f64::from(snapshot.state.velocity.y()) / 524_288.0,
                    f64::from(snapshot.state.velocity.z()) / 524_288.0,
                ],
                attitude_vector: release.inertial.platform_angle,
                angular_rate: release.inertial.angular_rate,
                control_demand: command.control_demand,
                commanded_gimbal: command.gimbal,
                applied_gimbal: release.director.applied_gimbal,
                inertial_validity: release.inertial.validity,
                aid_validity: release.aid.map(|v| v.validity).unwrap_or(0),
                alarms: expected.alarms,
                flight_mode: status.map(|v| v.mode).unwrap_or(0),
                armed: expected.armed,
                drogue_latched: expected.drogue_latched,
                main_latched: expected.main_latched,
                mass_kg: f64::from(snapshot.mass.mass.raw()) / 2_097_152.0,
                thrust_n: f64::from(snapshot.thrust_q13) / 8192.0,
                mach: f64::from(snapshot.aero.mach_q24) / 16_777_216.0,
                dynamic_pressure_pa: f64::from(snapshot.aero.dynamic_pressure_q13) / 8192.0,
                angle_of_attack_deg: f64::from(snapshot.aero.angle_of_attack_q28) / 268_435_456.0
                    * 180.0
                    / std::f64::consts::PI,
                static_margin: f64::from(snapshot.aero.static_margin_q24) / 16_777_216.0,
                wind_mps: snapshot.wind_q22.map(|v| f64::from(v) / 4_194_304.0),
                truth_checksum: release.director.truth_checksum,
                navigation_checksum: expected.navigation.checksum,
                flight_checksum: expected.flight_checksum,
            };
            let frame = ksa64_interface::phase8_5::Kat8Frame {
                epoch: epoch as u16,
                phase: update.phase,
                flags: u8::from(expected.safe),
                time_q18: snapshot.state.time.raw(),
                director_checksum: release.director.truth_checksum,
                inertial: release.inertial,
                command,
                status,
                aid_crc16: 0,
                aid_validity: update.aid_validity,
                truth_altitude_q13: snapshot.state.position.z(),
                truth_velocity_q19: [
                    snapshot.state.velocity.x(),
                    snapshot.state.velocity.y(),
                    snapshot.state.velocity.z(),
                ],
                applied_gimbal: release.director.applied_gimbal,
                events: snapshot.events,
                deployment_feedback: release.inertial.actuator_feedback,
            };
            target.publish(&update, &frame);
        }
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
    if !world.is_complete() {
        return Ok(None);
    }
    let summary = evaluate_with_avionics(AvionicsEvaluationRequest {
        vehicle: &reference.vehicle,
        motor: &reference.motor,
        mission: reference.mission,
        wind: &reference.wind,
        variation: SpatialMissionVariation::NOMINAL,
        variation_checksum: 0,
        avionics: reference.avionics,
        capability: reference.capability,
        uncertainty_case: LocalAvionicsVariation::NOMINAL,
    })
    .map_err(Phase85HostError::World)?;
    let evidence = Phase85RunEvidence {
        placement: LocalPlacement::HostExternalFlight,
        releases,
        summary,
    };
    if let Some(target) = sink {
        target.finish(&evidence);
    }
    Ok(Some(evidence))
}

pub fn run_host_external<S: Read + Write>(
    stream: &mut S,
    gimbal: bool,
    sink: Option<&mut dyn Phase85Sink>,
) -> Result<Phase85RunEvidence, Phase85LinkError> {
    run_host_external_with_limit(stream, gimbal, sink, u32::MAX)?.ok_or(Phase85LinkError::Protocol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    #[test]
    fn klf6_native_external_placement_is_exact() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            run_native_flight_endpoint(&mut stream, false).unwrap();
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let external = run_host_external(&mut stream, false, None).unwrap();
        endpoint.join().unwrap();
        let native = crate::phase8_5::run_host_host(false, None).unwrap();
        assert_eq!(external.summary, native.summary);
        assert_eq!(external.releases, native.releases);
    }
}
