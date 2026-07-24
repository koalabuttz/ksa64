//! User-facing Phase 6 host composition and passive Mission Control.
use ksa64_flight::phase6_realtime::{reference_realtime_guidance_slice, RealtimeFlightComputer};
use ksa64_interface::phase6::{
    parse_realtime_aid, parse_realtime_command, parse_realtime_inertial, parse_realtime_status,
    write_realtime_aid, write_realtime_command, write_realtime_inertial, write_realtime_status,
    RealtimeAidCell, RealtimeCommandCell, RealtimeInertialCell, RealtimeStatusCell, KLR6_READY,
    REALTIME_AID_LENGTH, REALTIME_COMMAND_LENGTH, REALTIME_INERTIAL_LENGTH, REALTIME_STATUS_LENGTH,
};
use ksa64_sim::phase6_mission_control::{
    compare_estimates, GroundComparison, GroundEstimator, GroundTrackingNetwork, TrackingConfig,
};
use ksa64_sim::phase6_realtime::{RealtimeRunError, RealtimeWorldEndpoint};
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

const MC_HASH_INITIAL: u32 = 2_166_136_261;
const MC_ALARM_CODEC: u16 = 1;
const MC_ALARM_GROUND: u16 = 2;

#[derive(Debug)]
pub enum RunnerError {
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
impl From<io::Error> for RunnerError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<RealtimeRunError> for RunnerError {
    fn from(value: RealtimeRunError) -> Self {
        Self::World(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerPace {
    Fast,
    Realtime,
    Step,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunnerOptions {
    pub mission_control: bool,
    pub pace: RunnerPace,
}
impl Default for RunnerOptions {
    fn default() -> Self {
        Self {
            mission_control: true,
            pace: RunnerPace::Fast,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionControlEvidence {
    pub world_cells: u32,
    pub flight_cells: u32,
    pub ground_fixes: u32,
    pub transcript_checksum: u32,
    pub ground_checksum: u32,
    pub alarms: u16,
    pub comparison: Option<GroundComparison>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunnerEvidence {
    pub complete: bool,
    pub fast_epochs: u32,
    pub mission_steps: u32,
    pub terminal_position_q12: [i32; 3],
    pub terminal_velocity_q24: [i32; 3],
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub status_flight_checksum: u32,
    pub final_flight_checksum: u32,
    pub navigation_checksum: u32,
    pub deadline_misses: u16,
    pub alarms: u16,
    pub mission_control: Option<MissionControlEvidence>,
}

struct HostMissionControl {
    world_cells: u32,
    flight_cells: u32,
    transcript_checksum: u32,
    alarms: u16,
    tracking: GroundTrackingNetwork,
    estimator: GroundEstimator,
    comparison: Option<GroundComparison>,
}
impl HostMissionControl {
    fn new() -> Self {
        Self {
            world_cells: 0,
            flight_cells: 0,
            transcript_checksum: MC_HASH_INITIAL,
            alarms: 0,
            tracking: GroundTrackingNetwork::new(0x4752_4e44, TrackingConfig::default()),
            estimator: GroundEstimator::new(),
            comparison: None,
        }
    }
    fn observe_world(
        &mut self,
        epoch: u32,
        inertial: RealtimeInertialCell,
        aid: Option<RealtimeAidCell>,
        position_q12: [i32; 3],
        velocity_q24: [i32; 3],
    ) {
        if let Some(aid) = aid {
            let mut bytes = [0u8; REALTIME_AID_LENGTH];
            if write_realtime_aid(&aid, &mut bytes).is_err() {
                self.alarms |= MC_ALARM_CODEC;
            } else {
                self.hash(&bytes);
                self.world_cells += 1;
            }
        }
        let mut bytes = [0u8; REALTIME_INERTIAL_LENGTH];
        if write_realtime_inertial(&inertial, &mut bytes).is_err() {
            self.alarms |= MC_ALARM_CODEC;
        } else {
            self.hash(&bytes);
            self.world_cells += 1;
        }
        self.tracking.observe(epoch, position_q12, velocity_q24);
        if let Some(fix) = self.tracking.poll(epoch) {
            if self.estimator.accept(epoch, fix).is_err() {
                self.alarms |= MC_ALARM_GROUND;
            }
        }
    }
    fn observe_flight(&mut self, command: RealtimeCommandCell, status: Option<RealtimeStatusCell>) {
        let mut command_bytes = [0u8; REALTIME_COMMAND_LENGTH];
        if write_realtime_command(&command, &mut command_bytes).is_err() {
            self.alarms |= MC_ALARM_CODEC;
        } else {
            self.hash(&command_bytes);
            self.flight_cells += 1;
        }
        if let Some(status) = status {
            let mut status_bytes = [0u8; REALTIME_STATUS_LENGTH];
            if write_realtime_status(&status, &mut status_bytes).is_err() {
                self.alarms |= MC_ALARM_CODEC;
            } else {
                self.hash(&status_bytes);
                self.flight_cells += 1;
            }
            if let Some(ground) = self.estimator.estimate() {
                self.comparison = Some(compare_estimates(
                    status.navigation_position_q12,
                    status.navigation_velocity_q24,
                    ground,
                ));
            }
        }
    }
    fn hash(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.transcript_checksum ^= *byte as u32;
            self.transcript_checksum = self.transcript_checksum.wrapping_mul(16_777_619);
        }
    }
    fn evidence(&self) -> MissionControlEvidence {
        let estimate = self.estimator.estimate();
        MissionControlEvidence {
            world_cells: self.world_cells,
            flight_cells: self.flight_cells,
            ground_fixes: estimate.map(|value| value.fixes).unwrap_or(0),
            transcript_checksum: self.transcript_checksum,
            ground_checksum: estimate.map(|value| value.checksum).unwrap_or(0),
            alarms: self.alarms,
            comparison: self.comparison,
        }
    }
}

struct Pacer {
    mode: RunnerPace,
    started: Instant,
}
impl Pacer {
    fn new(mode: RunnerPace) -> Self {
        Self {
            mode,
            started: Instant::now(),
        }
    }
    fn after_epoch(&self, epoch: u32) -> io::Result<()> {
        match self.mode {
            RunnerPace::Fast => Ok(()),
            RunnerPace::Realtime => {
                let target = self.started + Duration::from_micros(31_250 * (epoch as u64 + 1));
                if let Some(remaining) = target.checked_duration_since(Instant::now()) {
                    thread::sleep(remaining);
                }
                Ok(())
            }
            RunnerPace::Step => {
                eprint!("epoch {epoch} complete; press Enter to release the next epoch...");
                io::stderr().flush()?;
                let mut line = String::new();
                io::stdin().lock().read_line(&mut line)?;
                Ok(())
            }
        }
    }
}

pub fn run_world_with_flight<S: Read + Write>(
    stream: &mut S,
    max_epochs: u32,
    options: RunnerOptions,
) -> Result<RunnerEvidence, RunnerError> {
    let mut ready = [0u8; 4];
    stream.read_exact(&mut ready)?;
    if ready != KLR6_READY {
        return Err(RunnerError::Codec);
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
    let guidance = reference_realtime_guidance_slice(0);
    shadow.set_guidance_segment(guidance.start, guidance.end, guidance.rate);
    let mut last_status: Option<RealtimeStatusCell> = None;
    let mut mission_control = options.mission_control.then(HostMissionControl::new);
    let pacer = Pacer::new(options.pace);
    while !world.is_complete() && world.epoch() < max_epochs {
        let epoch = world.epoch();
        let release = world.release()?;
        if let Some(observer) = mission_control.as_mut() {
            observer.observe_world(
                epoch,
                release.inertial,
                release.aid,
                release.truth_position_q12,
                release.truth_velocity_q24,
            );
        }
        if epoch & 31 == 2 {
            let guidance = reference_realtime_guidance_slice((epoch >> 5) as u16);
            shadow.set_guidance_segment(guidance.start, guidance.end, guidance.rate);
        }
        let expected = shadow.tick(Some(release.inertial), release.aid);
        if let Some(aid) = release.aid {
            let mut bytes = [0u8; REALTIME_AID_LENGTH];
            write_realtime_aid(&aid, &mut bytes).map_err(|_| RunnerError::Codec)?;
            stream.write_all(&bytes)?;
        }
        let mut inertial = [0u8; REALTIME_INERTIAL_LENGTH];
        write_realtime_inertial(&release.inertial, &mut inertial)
            .map_err(|_| RunnerError::Codec)?;
        stream.write_all(&inertial)?;
        stream.flush()?;
        let mut command_bytes = [0u8; REALTIME_COMMAND_LENGTH];
        stream.read_exact(&mut command_bytes)?;
        let command = parse_realtime_command(&command_bytes).map_err(|_| RunnerError::Codec)?;
        if command != expected.command {
            return Err(RunnerError::CommandMismatch {
                epoch,
                expected: expected.command,
                actual: command,
            });
        }
        if command.source_epoch != epoch as u16
            || command.effective_epoch != epoch.wrapping_add(1) as u16
        {
            return Err(RunnerError::Epoch);
        }
        let status = if epoch & 3 == 0 {
            let mut bytes = [0u8; REALTIME_STATUS_LENGTH];
            stream.read_exact(&mut bytes)?;
            let parsed = parse_realtime_status(&bytes).map_err(|_| RunnerError::Codec)?;
            let expected_status = expected.status.ok_or(RunnerError::Epoch)?;
            if parsed != expected_status {
                return Err(RunnerError::StatusMismatch {
                    epoch,
                    expected: expected_status,
                    actual: parsed,
                });
            }
            if parsed.source_epoch != epoch as u16 {
                return Err(RunnerError::Epoch);
            }
            last_status = Some(parsed);
            Some(parsed)
        } else {
            None
        };
        if let Some(observer) = mission_control.as_mut() {
            observer.observe_flight(command, status);
        }
        world.accept_command(command)?;
        pacer.after_epoch(epoch)?;
    }
    let complete = world.is_complete();
    let snapshot = world.snapshot();
    let position = snapshot.truth.spatial().position();
    let velocity = snapshot.truth.spatial().velocity();
    let status = last_status.ok_or(RunnerError::Epoch)?;
    let navigation = shadow.navigation();
    Ok(RunnerEvidence {
        complete,
        fast_epochs: world.epoch(),
        mission_steps: snapshot.truth.step(),
        terminal_position_q12: [position.x(), position.y(), position.z()],
        terminal_velocity_q24: [velocity.x(), velocity.y(), velocity.z()],
        navigation_position_q12: status.navigation_position_q12,
        navigation_velocity_q24: status.navigation_velocity_q24,
        status_flight_checksum: status.flight_checksum,
        final_flight_checksum: shadow.flight_checksum(),
        navigation_checksum: navigation.checksum,
        deadline_misses: status.deadline_misses,
        alarms: status.alarms,
        mission_control: mission_control.map(|value| value.evidence()),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeFlightEvidence {
    pub flight_checksum: u32,
    pub navigation_checksum: u32,
}

pub fn run_native_flight_peer<S: Read + Write>(
    stream: &mut S,
) -> Result<NativeFlightEvidence, RunnerError> {
    stream.write_all(&KLR6_READY)?;
    stream.flush()?;
    let mut flight =
        RealtimeFlightComputer::new(0x6a52, [22_958_965, 0, 12_465_701], [0, 6_857_499, 0]);
    let guidance = reference_realtime_guidance_slice(0);
    flight.set_guidance_segment(guidance.start, guidance.end, guidance.rate);
    for epoch in 0u16.. {
        let aid = if epoch & 3 == 0 {
            let mut bytes = [0u8; REALTIME_AID_LENGTH];
            stream.read_exact(&mut bytes)?;
            Some(parse_realtime_aid(&bytes).map_err(|_| RunnerError::Codec)?)
        } else {
            None
        };
        let mut bytes = [0u8; REALTIME_INERTIAL_LENGTH];
        stream.read_exact(&mut bytes)?;
        let inertial = parse_realtime_inertial(&bytes).map_err(|_| RunnerError::Codec)?;
        if epoch & 31 == 2 {
            let guidance = reference_realtime_guidance_slice(epoch >> 5);
            flight.set_guidance_segment(guidance.start, guidance.end, guidance.rate);
        }
        let output = flight.tick(Some(inertial), aid);
        let mut command = [0u8; REALTIME_COMMAND_LENGTH];
        write_realtime_command(&output.command, &mut command).map_err(|_| RunnerError::Codec)?;
        stream.write_all(&command)?;
        if let Some(status) = output.status {
            let mut bytes = [0u8; REALTIME_STATUS_LENGTH];
            write_realtime_status(&status, &mut bytes).map_err(|_| RunnerError::Codec)?;
            stream.write_all(&bytes)?;
        }
        stream.flush()?;
        if inertial.flags & 1 != 0 {
            return Ok(NativeFlightEvidence {
                flight_checksum: flight.flight_checksum(),
                navigation_checksum: flight.navigation().checksum,
            });
        }
    }
    Err(RunnerError::Epoch)
}

pub fn run_native_host_mission(options: RunnerOptions) -> Result<RunnerEvidence, RunnerError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let flight = thread::spawn(move || -> Result<NativeFlightEvidence, RunnerError> {
        let mut stream = TcpStream::connect(address)?;
        super::phase6::configure_socket(&stream)?;
        run_native_flight_peer(&mut stream)
    });
    let (mut stream, _) = listener.accept()?;
    super::phase6::configure_socket(&stream)?;
    let result = run_world_with_flight(&mut stream, u32::MAX, options);
    drop(stream);
    let peer = flight.join().map_err(|_| RunnerError::Epoch)??;
    let evidence = result?;
    if peer.flight_checksum != evidence.final_flight_checksum
        || peer.navigation_checksum != evidence.navigation_checksum
    {
        return Err(RunnerError::Epoch);
    }
    Ok(evidence)
}
