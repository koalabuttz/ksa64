//! Phase 6 host socket broker and deterministic loopback acceptance.
use ksa64_flight::phase6_realtime::{reference_realtime_guidance_slice, RealtimeFlightComputer};
use ksa64_interface::phase6::{
    parse_realtime_command, parse_realtime_status, write_realtime_aid, write_realtime_inertial,
    RealtimeCommandCell, RealtimeStatusCell, KLR6_READY, REALTIME_AID_LENGTH,
    REALTIME_COMMAND_LENGTH, REALTIME_INERTIAL_LENGTH, REALTIME_STATUS_LENGTH,
};
use ksa64_sim::phase6_realtime::{RealtimeRunError, RealtimeWorldEndpoint};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug)]
pub enum BridgeError {
    Io(io::Error),
    Codec,
    World(RealtimeRunError),
    Epoch,
    CommandMismatch {
        epoch: u32,
        expected: RealtimeCommandCell,
        actual: RealtimeCommandCell,
    },
    StatusMismatch {
        epoch: u32,
        expected: RealtimeStatusCell,
        actual: RealtimeStatusCell,
    },
}
impl From<io::Error> for BridgeError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<RealtimeRunError> for BridgeError {
    fn from(value: RealtimeRunError) -> Self {
        Self::World(value)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BridgeEvidence {
    pub fast_epochs: u32,
    pub mission_steps: u32,
    pub terminal_position_q12: [i32; 3],
    pub terminal_velocity_q24: [i32; 3],
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub flight_checksum: u32,
    pub final_flight_checksum: u32,
    pub navigation_checksum: u32,
    pub deadline_misses: u16,
    pub alarms: u16,
}
/// Blocking step-and-ack broker. Transport timing is external; simulation ordering is fixed.
pub fn run_realtime_world_bridge<S: Read + Write>(
    stream: &mut S,
    max_epochs: u32,
) -> Result<BridgeEvidence, BridgeError> {
    let mut ready = [0u8; 4];
    stream.read_exact(&mut ready)?;
    if ready != KLR6_READY {
        return Err(BridgeError::Codec);
    }
    let mut world = RealtimeWorldEndpoint::new_nominal()?;
    let initial = world.snapshot().truth;
    let initial_position = initial.spatial().position();
    let initial_velocity = initial.spatial().velocity();
    let mut shadow = RealtimeFlightComputer::new(
        0x6a52,
        [
            initial_position.x(),
            initial_position.y(),
            initial_position.z(),
        ],
        [
            initial_velocity.x(),
            initial_velocity.y(),
            initial_velocity.z(),
        ],
    );
    let initial_guidance = reference_realtime_guidance_slice(0);
    shadow.set_guidance_segment(
        initial_guidance.start,
        initial_guidance.end,
        initial_guidance.rate,
    );
    let mut last_status: Option<RealtimeStatusCell> = None;
    while !world.is_complete() && world.epoch() < max_epochs {
        let epoch = world.epoch();
        let release = world.release()?;
        if epoch & 31 == 2 {
            let guidance = reference_realtime_guidance_slice((epoch >> 5) as u16);
            shadow.set_guidance_segment(guidance.start, guidance.end, guidance.rate);
        }
        let expected = shadow.tick(Some(release.inertial), release.aid);
        if let Some(aid) = release.aid {
            let mut b = [0u8; REALTIME_AID_LENGTH];
            write_realtime_aid(&aid, &mut b).map_err(|_| BridgeError::Codec)?;
            stream.write_all(&b)?;
        }
        let mut inertial = [0u8; REALTIME_INERTIAL_LENGTH];
        write_realtime_inertial(&release.inertial, &mut inertial)
            .map_err(|_| BridgeError::Codec)?;
        stream.write_all(&inertial)?;
        stream.flush()?;
        let mut command = [0u8; REALTIME_COMMAND_LENGTH];
        stream.read_exact(&mut command)?;
        let command = parse_realtime_command(&command).map_err(|_| BridgeError::Codec)?;
        if command != expected.command {
            return Err(BridgeError::CommandMismatch {
                epoch,
                expected: expected.command,
                actual: command,
            });
        }
        if command.source_epoch != epoch as u16
            || command.effective_epoch != epoch.wrapping_add(1) as u16
        {
            return Err(BridgeError::Epoch);
        }
        if epoch & 3 == 0 {
            let mut status = [0u8; REALTIME_STATUS_LENGTH];
            stream.read_exact(&mut status)?;
            let parsed = parse_realtime_status(&status).map_err(|_| BridgeError::Codec)?;
            let expected_status = expected.status.ok_or(BridgeError::Epoch)?;
            if parsed != expected_status {
                return Err(BridgeError::StatusMismatch {
                    epoch,
                    expected: expected_status,
                    actual: parsed,
                });
            }
            if parsed.source_epoch != epoch as u16 {
                return Err(BridgeError::Epoch);
            }
            last_status = Some(parsed);
        }
        world.accept_command(command)?;
    }
    let snapshot = world.snapshot();
    let p = snapshot.truth.spatial().position();
    let v = snapshot.truth.spatial().velocity();
    let status = last_status.ok_or(BridgeError::Epoch)?;
    let shadow_navigation = shadow.navigation();
    Ok(BridgeEvidence {
        fast_epochs: world.epoch(),
        mission_steps: snapshot.truth.step(),
        terminal_position_q12: [p.x(), p.y(), p.z()],
        terminal_velocity_q24: [v.x(), v.y(), v.z()],
        navigation_position_q12: status.navigation_position_q12,
        navigation_velocity_q24: status.navigation_velocity_q24,
        flight_checksum: status.flight_checksum,
        final_flight_checksum: shadow.flight_checksum(),
        navigation_checksum: shadow_navigation.checksum,
        deadline_misses: status.deadline_misses,
        alarms: status.alarms,
    })
}
/// Native TCP adapter used by VICE and Ultimate bridges.
pub fn configure_socket(stream: &TcpStream) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(300)))?;
    stream.set_write_timeout(Some(Duration::from_secs(300)))
}
