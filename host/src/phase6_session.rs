//! Host-only KMR6 live Mission Control recording, recovery, replay, and export.
use crate::phase6_runner::{
    MissionControlSink, MissionControlUpdate, PaceRate, PaceSnapshot, RunnerEvidence,
};
use crc32fast::Hasher;
use ksa64_flight::phase6_realtime::RealtimeGuidanceSlice;
use ksa64_interface::phase6::{
    parse_realtime_aid, parse_realtime_command, parse_realtime_inertial, parse_realtime_status,
    write_realtime_aid, write_realtime_command, write_realtime_inertial, write_realtime_status,
    GroundTrackingFix, REALTIME_AID_LENGTH, REALTIME_COMMAND_LENGTH, REALTIME_INERTIAL_LENGTH,
    REALTIME_STATUS_LENGTH,
};
use ksa64_sim::phase6_mission_control::{GroundComparison, GroundEstimate};
use ksa64_sim::phase6_realtime::RealtimeDirectorSample;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const KMR6_HEADER_LENGTH: usize = 32;
const KMR6_VERSION: u16 = 1;
const RECORD_UPDATE: u8 = 1;
const RECORD_FINISH: u8 = 2;
const MAX_RECORD: usize = 1024 * 1024;

#[derive(Debug)]
pub enum SessionError {
    Io(io::Error),
    Header,
    Codec,
    Json(serde_json::Error),
}
impl From<io::Error> for SessionError {
    fn from(v: io::Error) -> Self {
        Self::Io(v)
    }
}
impl From<serde_json::Error> for SessionError {
    fn from(v: serde_json::Error) -> Self {
        Self::Json(v)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedUpdate {
    epoch: u32,
    wall_micros: u64,
    inertial: Vec<u8>,
    aid: Option<Vec<u8>>,
    command: Vec<u8>,
    status: Option<Vec<u8>>,
    ground_fix: Option<RecordedFix>,
    ground_estimate: Option<RecordedEstimate>,
    comparison: Option<RecordedComparison>,
    director: RecordedDirector,
    guidance: RecordedGuidance,
    world_cells: u32,
    flight_cells: u32,
    transcript_checksum: u32,
    mission_control_alarms: u16,
    pace_rate: u8,
    paused: bool,
    cancelled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedFix {
    fix_id: u32,
    measurement_epoch: u32,
    production_epoch: u32,
    position: [i32; 3],
    velocity: [i32; 3],
    network_id: u16,
    validity: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedEstimate {
    epoch: u32,
    position: [i32; 3],
    velocity: [i32; 3],
    fixes: u32,
    checksum: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedComparison {
    position: [i32; 3],
    velocity: [i32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedGuidance {
    start: [i16; 3],
    end: [i16; 3],
    rate: [i16; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordedDirector {
    position: [i32; 3],
    velocity: [i32; 3],
    acceleration: [i32; 3],
    attitude: [i32; 4],
    angular_rate: [i32; 3],
    flexible: [i32; 8],
    total_mass: i32,
    active_propellant: i32,
    rcs_propellant: i32,
    time: i32,
    step: u32,
    active_stage: u8,
    phase: u8,
    substep: u8,
    mach: i32,
    dynamic_pressure: i32,
    angle_of_attack_sine: i32,
    gimbal_requested: [i32; 2],
    gimbal_lagged: [i32; 2],
    gimbal_applied: [i32; 2],
    events: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionEvidence {
    pub complete: bool,
    pub operator_stopped: bool,
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
}
impl From<&RunnerEvidence> for SessionEvidence {
    fn from(v: &RunnerEvidence) -> Self {
        Self {
            complete: v.complete,
            operator_stopped: v.operator_stopped,
            fast_epochs: v.fast_epochs,
            mission_steps: v.mission_steps,
            terminal_position_q12: v.terminal_position_q12,
            terminal_velocity_q24: v.terminal_velocity_q24,
            navigation_position_q12: v.navigation_position_q12,
            navigation_velocity_q24: v.navigation_velocity_q24,
            status_flight_checksum: v.status_flight_checksum,
            final_flight_checksum: v.final_flight_checksum,
            navigation_checksum: v.navigation_checksum,
            deadline_misses: v.deadline_misses,
            alarms: v.alarms,
        }
    }
}
fn rate_to_u8(v: PaceRate) -> u8 {
    match v {
        PaceRate::Quarter => 0,
        PaceRate::Half => 1,
        PaceRate::Realtime => 2,
        PaceRate::Double => 3,
        PaceRate::Max => 4,
    }
}
fn rate_from_u8(v: u8) -> PaceRate {
    match v {
        0 => PaceRate::Quarter,
        1 => PaceRate::Half,
        2 => PaceRate::Realtime,
        3 => PaceRate::Double,
        _ => PaceRate::Max,
    }
}
impl RecordedUpdate {
    fn from_live(v: MissionControlUpdate) -> Result<Self, SessionError> {
        let mut inertial = vec![0; REALTIME_INERTIAL_LENGTH];
        write_realtime_inertial(&v.inertial, &mut inertial).map_err(|_| SessionError::Codec)?;
        let aid = if let Some(x) = v.aid {
            let mut b = vec![0; REALTIME_AID_LENGTH];
            write_realtime_aid(&x, &mut b).map_err(|_| SessionError::Codec)?;
            Some(b)
        } else {
            None
        };
        let mut command = vec![0; REALTIME_COMMAND_LENGTH];
        write_realtime_command(&v.command, &mut command).map_err(|_| SessionError::Codec)?;
        let status = if let Some(x) = v.status {
            let mut b = vec![0; REALTIME_STATUS_LENGTH];
            write_realtime_status(&x, &mut b).map_err(|_| SessionError::Codec)?;
            Some(b)
        } else {
            None
        };
        Ok(Self {
            epoch: v.epoch,
            wall_micros: v.wall_micros,
            inertial,
            aid,
            command,
            status,
            ground_fix: v.ground_fix.map(|x| RecordedFix {
                fix_id: x.fix_id,
                measurement_epoch: x.measurement_epoch,
                production_epoch: x.production_epoch,
                position: x.position_ecef_q12,
                velocity: x.velocity_ecef_q24,
                network_id: x.network_id,
                validity: x.validity,
            }),
            ground_estimate: v.ground_estimate.map(|x| RecordedEstimate {
                epoch: x.epoch,
                position: x.position_q12,
                velocity: x.velocity_q24,
                fixes: x.fixes,
                checksum: x.checksum,
            }),
            comparison: v.comparison.map(|x| RecordedComparison {
                position: x.position_delta_q12,
                velocity: x.velocity_delta_q24,
            }),
            director: RecordedDirector::from(v.director),
            guidance: RecordedGuidance {
                start: v.guidance.start,
                end: v.guidance.end,
                rate: v.guidance.rate,
            },
            world_cells: v.world_cells,
            flight_cells: v.flight_cells,
            transcript_checksum: v.transcript_checksum,
            mission_control_alarms: v.mission_control_alarms,
            pace_rate: rate_to_u8(v.pace.rate),
            paused: v.pace.paused,
            cancelled: v.pace.cancelled,
        })
    }
    fn into_live(self) -> Result<MissionControlUpdate, SessionError> {
        Ok(MissionControlUpdate {
            epoch: self.epoch,
            wall_micros: self.wall_micros,
            inertial: parse_realtime_inertial(&self.inertial).map_err(|_| SessionError::Codec)?,
            aid: self
                .aid
                .as_deref()
                .map(parse_realtime_aid)
                .transpose()
                .map_err(|_| SessionError::Codec)?,
            command: parse_realtime_command(&self.command).map_err(|_| SessionError::Codec)?,
            status: self
                .status
                .as_deref()
                .map(parse_realtime_status)
                .transpose()
                .map_err(|_| SessionError::Codec)?,
            ground_fix: self.ground_fix.map(|x| GroundTrackingFix {
                fix_id: x.fix_id,
                measurement_epoch: x.measurement_epoch,
                production_epoch: x.production_epoch,
                position_ecef_q12: x.position,
                velocity_ecef_q24: x.velocity,
                network_id: x.network_id,
                validity: x.validity,
            }),
            ground_estimate: self.ground_estimate.map(|x| GroundEstimate {
                epoch: x.epoch,
                position_q12: x.position,
                velocity_q24: x.velocity,
                fixes: x.fixes,
                checksum: x.checksum,
            }),
            comparison: self.comparison.map(|x| GroundComparison {
                position_delta_q12: x.position,
                velocity_delta_q24: x.velocity,
            }),
            director: self.director.into(),
            guidance: RealtimeGuidanceSlice {
                start: self.guidance.start,
                end: self.guidance.end,
                rate: self.guidance.rate,
            },
            world_cells: self.world_cells,
            flight_cells: self.flight_cells,
            transcript_checksum: self.transcript_checksum,
            mission_control_alarms: self.mission_control_alarms,
            pace: PaceSnapshot {
                rate: rate_from_u8(self.pace_rate),
                paused: self.paused,
                cancelled: self.cancelled,
            },
        })
    }
}
impl From<RealtimeDirectorSample> for RecordedDirector {
    fn from(v: RealtimeDirectorSample) -> Self {
        Self {
            position: v.position_q12,
            velocity: v.velocity_q24,
            acceleration: v.acceleration_q28,
            attitude: v.attitude_q30,
            angular_rate: v.angular_rate_q24,
            flexible: v.flexible_q24,
            total_mass: v.total_mass_q12,
            active_propellant: v.active_propellant_q12,
            rcs_propellant: v.rcs_propellant_q12,
            time: v.time_q16,
            step: v.step,
            active_stage: v.active_stage,
            phase: v.phase,
            substep: v.substep,
            mach: v.mach_q16,
            dynamic_pressure: v.dynamic_pressure_q16,
            angle_of_attack_sine: v.angle_of_attack_sine_q16,
            gimbal_requested: v.gimbal_requested_q16,
            gimbal_lagged: v.gimbal_lagged_q16,
            gimbal_applied: v.gimbal_applied_q16,
            events: v.events,
        }
    }
}
impl From<RecordedDirector> for RealtimeDirectorSample {
    fn from(v: RecordedDirector) -> Self {
        Self {
            position_q12: v.position,
            velocity_q24: v.velocity,
            acceleration_q28: v.acceleration,
            attitude_q30: v.attitude,
            angular_rate_q24: v.angular_rate,
            flexible_q24: v.flexible,
            total_mass_q12: v.total_mass,
            active_propellant_q12: v.active_propellant,
            rcs_propellant_q12: v.rcs_propellant,
            time_q16: v.time,
            step: v.step,
            active_stage: v.active_stage,
            phase: v.phase,
            substep: v.substep,
            mach_q16: v.mach,
            dynamic_pressure_q16: v.dynamic_pressure,
            angle_of_attack_sine_q16: v.angle_of_attack_sine,
            gimbal_requested_q16: v.gimbal_requested,
            gimbal_lagged_q16: v.gimbal_lagged,
            gimbal_applied_q16: v.gimbal_applied,
            events: v.events,
        }
    }
}

fn put_u8(o: &mut Vec<u8>, v: u8) {
    o.push(v)
}
fn put_bool(o: &mut Vec<u8>, v: bool) {
    o.push(v as u8)
}
fn put_u16(o: &mut Vec<u8>, v: u16) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn put_i16(o: &mut Vec<u8>, v: i16) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn put_u32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn put_i32(o: &mut Vec<u8>, v: i32) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn put_u64(o: &mut Vec<u8>, v: u64) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn put_i16s<const N: usize>(o: &mut Vec<u8>, v: [i16; N]) {
    for x in v {
        put_i16(o, x)
    }
}
fn put_i32s<const N: usize>(o: &mut Vec<u8>, v: [i32; N]) {
    for x in v {
        put_i32(o, x)
    }
}
struct Decoder<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> Decoder<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], SessionError> {
        if self.b.len().saturating_sub(self.p) < n {
            return Err(SessionError::Codec);
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, SessionError> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, SessionError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SessionError::Codec),
        }
    }
    fn u16(&mut self) -> Result<u16, SessionError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, SessionError> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, SessionError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, SessionError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, SessionError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i16s<const N: usize>(&mut self) -> Result<[i16; N], SessionError> {
        let mut v = [0; N];
        for x in &mut v {
            *x = self.i16()?
        }
        Ok(v)
    }
    fn i32s<const N: usize>(&mut self) -> Result<[i32; N], SessionError> {
        let mut v = [0; N];
        for x in &mut v {
            *x = self.i32()?
        }
        Ok(v)
    }
}
impl RecordedUpdate {
    fn encode_binary(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(512);
        put_u32(&mut o, self.epoch);
        put_u64(&mut o, self.wall_micros);
        o.extend_from_slice(&self.inertial);
        put_bool(&mut o, self.aid.is_some());
        if let Some(x) = &self.aid {
            o.extend_from_slice(x)
        }
        o.extend_from_slice(&self.command);
        put_bool(&mut o, self.status.is_some());
        if let Some(x) = &self.status {
            o.extend_from_slice(x)
        }
        put_bool(&mut o, self.ground_fix.is_some());
        if let Some(x) = &self.ground_fix {
            put_u32(&mut o, x.fix_id);
            put_u32(&mut o, x.measurement_epoch);
            put_u32(&mut o, x.production_epoch);
            put_i32s(&mut o, x.position);
            put_i32s(&mut o, x.velocity);
            put_u16(&mut o, x.network_id);
            put_u16(&mut o, x.validity)
        }
        put_bool(&mut o, self.ground_estimate.is_some());
        if let Some(x) = &self.ground_estimate {
            put_u32(&mut o, x.epoch);
            put_i32s(&mut o, x.position);
            put_i32s(&mut o, x.velocity);
            put_u32(&mut o, x.fixes);
            put_u32(&mut o, x.checksum)
        }
        put_bool(&mut o, self.comparison.is_some());
        if let Some(x) = &self.comparison {
            put_i32s(&mut o, x.position);
            put_i32s(&mut o, x.velocity)
        }
        let d = &self.director;
        put_i32s(&mut o, d.position);
        put_i32s(&mut o, d.velocity);
        put_i32s(&mut o, d.acceleration);
        put_i32s(&mut o, d.attitude);
        put_i32s(&mut o, d.angular_rate);
        put_i32s(&mut o, d.flexible);
        put_i32(&mut o, d.total_mass);
        put_i32(&mut o, d.active_propellant);
        put_i32(&mut o, d.rcs_propellant);
        put_i32(&mut o, d.time);
        put_u32(&mut o, d.step);
        put_u8(&mut o, d.active_stage);
        put_u8(&mut o, d.phase);
        put_u8(&mut o, d.substep);
        put_i32(&mut o, d.mach);
        put_i32(&mut o, d.dynamic_pressure);
        put_i32(&mut o, d.angle_of_attack_sine);
        put_i32s(&mut o, d.gimbal_requested);
        put_i32s(&mut o, d.gimbal_lagged);
        put_i32s(&mut o, d.gimbal_applied);
        put_u16(&mut o, d.events);
        put_i16s(&mut o, self.guidance.start);
        put_i16s(&mut o, self.guidance.end);
        put_i16s(&mut o, self.guidance.rate);
        put_u32(&mut o, self.world_cells);
        put_u32(&mut o, self.flight_cells);
        put_u32(&mut o, self.transcript_checksum);
        put_u16(&mut o, self.mission_control_alarms);
        put_u8(&mut o, self.pace_rate);
        put_bool(&mut o, self.paused);
        put_bool(&mut o, self.cancelled);
        o
    }
    fn decode_binary(b: &[u8]) -> Result<Self, SessionError> {
        let mut d = Decoder::new(b);
        let epoch = d.u32()?;
        let wall_micros = d.u64()?;
        let inertial = d.take(REALTIME_INERTIAL_LENGTH)?.to_vec();
        let aid = if d.bool()? {
            Some(d.take(REALTIME_AID_LENGTH)?.to_vec())
        } else {
            None
        };
        let command = d.take(REALTIME_COMMAND_LENGTH)?.to_vec();
        let status = if d.bool()? {
            Some(d.take(REALTIME_STATUS_LENGTH)?.to_vec())
        } else {
            None
        };
        let ground_fix = if d.bool()? {
            Some(RecordedFix {
                fix_id: d.u32()?,
                measurement_epoch: d.u32()?,
                production_epoch: d.u32()?,
                position: d.i32s()?,
                velocity: d.i32s()?,
                network_id: d.u16()?,
                validity: d.u16()?,
            })
        } else {
            None
        };
        let ground_estimate = if d.bool()? {
            Some(RecordedEstimate {
                epoch: d.u32()?,
                position: d.i32s()?,
                velocity: d.i32s()?,
                fixes: d.u32()?,
                checksum: d.u32()?,
            })
        } else {
            None
        };
        let comparison = if d.bool()? {
            Some(RecordedComparison {
                position: d.i32s()?,
                velocity: d.i32s()?,
            })
        } else {
            None
        };
        let director = RecordedDirector {
            position: d.i32s()?,
            velocity: d.i32s()?,
            acceleration: d.i32s()?,
            attitude: d.i32s()?,
            angular_rate: d.i32s()?,
            flexible: d.i32s()?,
            total_mass: d.i32()?,
            active_propellant: d.i32()?,
            rcs_propellant: d.i32()?,
            time: d.i32()?,
            step: d.u32()?,
            active_stage: d.u8()?,
            phase: d.u8()?,
            substep: d.u8()?,
            mach: d.i32()?,
            dynamic_pressure: d.i32()?,
            angle_of_attack_sine: d.i32()?,
            gimbal_requested: d.i32s()?,
            gimbal_lagged: d.i32s()?,
            gimbal_applied: d.i32s()?,
            events: d.u16()?,
        };
        let guidance = RecordedGuidance {
            start: d.i16s()?,
            end: d.i16s()?,
            rate: d.i16s()?,
        };
        let result = Self {
            epoch,
            wall_micros,
            inertial,
            aid,
            command,
            status,
            ground_fix,
            ground_estimate,
            comparison,
            director,
            guidance,
            world_cells: d.u32()?,
            flight_cells: d.u32()?,
            transcript_checksum: d.u32()?,
            mission_control_alarms: d.u16()?,
            pace_rate: {
                let value = d.u8()?;
                if value > 4 {
                    return Err(SessionError::Codec);
                }
                value
            },
            paused: d.bool()?,
            cancelled: d.bool()?,
        };
        if d.p != b.len() {
            return Err(SessionError::Codec);
        }
        Ok(result)
    }
}

pub struct SessionRecorder {
    path: PathBuf,
    writer: BufWriter<File>,
    records: u32,
}
impl SessionRecorder {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?
        }
        let mut writer = BufWriter::new(File::create(&path)?);
        let mut h = [0u8; KMR6_HEADER_LENGTH];
        h[..4].copy_from_slice(b"KMR6");
        h[4..6].copy_from_slice(&KMR6_VERSION.to_le_bytes());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        h[8..16].copy_from_slice(&now.to_le_bytes());
        let crc = crc32(&h[..28]);
        h[28..].copy_from_slice(&crc.to_le_bytes());
        writer.write_all(&h)?;
        Ok(Self {
            path,
            writer,
            records: 0,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn record_update(&mut self, v: MissionControlUpdate) -> Result<(), SessionError> {
        let payload = RecordedUpdate::from_live(v)?.encode_binary();
        self.write_record(RECORD_UPDATE, &payload)
    }
    pub fn finish(&mut self, v: &RunnerEvidence) -> Result<(), SessionError> {
        let payload = serde_json::to_vec(&SessionEvidence::from(v))?;
        self.write_record(RECORD_FINISH, &payload)?;
        self.writer.flush()?;
        Ok(())
    }
    fn write_record(&mut self, kind: u8, payload: &[u8]) -> Result<(), SessionError> {
        let mut prefix = [0u8; 6];
        prefix[0] = kind;
        prefix[2..].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        let mut h = Hasher::new();
        h.update(&prefix);
        h.update(payload);
        let crc = h.finalize();
        self.writer.write_all(&prefix)?;
        self.writer.write_all(payload)?;
        self.writer.write_all(&crc.to_le_bytes())?;
        self.records += 1;
        Ok(())
    }
}
pub struct RecordingSink {
    recorder: SessionRecorder,
    error: Option<SessionError>,
}
impl RecordingSink {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        Ok(Self {
            recorder: SessionRecorder::create(path)?,
            error: None,
        })
    }
    pub fn check(self) -> Result<PathBuf, SessionError> {
        match self.error {
            Some(e) => Err(e),
            None => Ok(self.recorder.path),
        }
    }
}
impl MissionControlSink for RecordingSink {
    fn publish(&mut self, v: MissionControlUpdate) {
        if self.error.is_none() {
            if let Err(e) = self.recorder.record_update(v) {
                self.error = Some(e)
            }
        }
    }
    fn finish(&mut self, v: &RunnerEvidence) {
        if self.error.is_none() {
            if let Err(e) = self.recorder.finish(v) {
                self.error = Some(e)
            }
        }
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(bytes);
    h.finalize()
}
pub fn default_session_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    PathBuf::from(format!("target/phase6/sessions/ksa6r-{stamp}-{pid}.kmr6"))
}

#[derive(Debug)]
pub struct Session {
    pub updates: Vec<MissionControlUpdate>,
    pub evidence: Option<SessionEvidence>,
    pub complete: bool,
    pub recovered: bool,
}
impl Session {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        let mut data = Vec::new();
        File::open(path)?.read_to_end(&mut data)?;
        if data.len() < KMR6_HEADER_LENGTH
            || &data[..4] != b"KMR6"
            || u16::from_le_bytes([data[4], data[5]]) != KMR6_VERSION
            || data[6..8] != [0, 0]
            || data[16..28] != [0; 12]
            || crc32(&data[..28]) != u32::from_le_bytes(data[28..32].try_into().unwrap())
        {
            return Err(SessionError::Header);
        }
        let mut at = KMR6_HEADER_LENGTH;
        let mut updates = Vec::new();
        let mut evidence: Option<SessionEvidence> = None;
        let mut recovered = false;
        while at < data.len() {
            if data.len() - at < 10 {
                recovered = true;
                break;
            }
            let prefix = &data[at..at + 6];
            let kind = prefix[0];
            if prefix[1] != 0 {
                recovered = true;
                break;
            }
            let len = u32::from_le_bytes(prefix[2..6].try_into().unwrap()) as usize;
            if len > MAX_RECORD || data.len() - at < 6 + len + 4 {
                recovered = true;
                break;
            }
            let payload = &data[at + 6..at + 6 + len];
            let expected =
                u32::from_le_bytes(data[at + 6 + len..at + 10 + len].try_into().unwrap());
            let mut h = Hasher::new();
            h.update(prefix);
            h.update(payload);
            if h.finalize() != expected {
                recovered = true;
                break;
            }
            match kind {
                RECORD_UPDATE => {
                    let r = RecordedUpdate::decode_binary(payload)?;
                    updates.push(r.into_live()?)
                }
                RECORD_FINISH => evidence = Some(serde_json::from_slice(payload)?),
                _ => {
                    recovered = true;
                    break;
                }
            }
            at += 10 + len
        }
        let complete = evidence.as_ref().map(|v| v.complete).unwrap_or(false) && !recovered;
        Ok(Self {
            updates,
            evidence,
            complete,
            recovered,
        })
    }
    pub fn export_json(&self, path: impl AsRef<Path>) -> Result<(), SessionError> {
        #[derive(Serialize)]
        struct Export<'a> {
            complete: bool,
            recovered: bool,
            evidence: &'a Option<SessionEvidence>,
            samples: Vec<ExportRow>,
        }
        let rows = self.updates.iter().map(ExportRow::from).collect();
        fs::write(
            path,
            serde_json::to_vec_pretty(&Export {
                complete: self.complete,
                recovered: self.recovered,
                evidence: &self.evidence,
                samples: rows,
            })?,
        )?;
        Ok(())
    }
    pub fn export_csv(&self, path: impl AsRef<Path>) -> Result<(), SessionError> {
        let mut w = BufWriter::new(File::create(path)?);
        writeln!(w,"epoch,time_s,altitude_km,speed_km_s,mach,dynamic_pressure_kpa,stage,phase,mass_t,propellant_t,flight_checksum")?;
        for u in &self.updates {
            let r = ExportRow::from(u);
            writeln!(
                w,
                "{},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{:.6},{:.6},{}",
                r.epoch,
                r.time_s,
                r.altitude_km,
                r.speed_km_s,
                r.mach,
                r.dynamic_pressure_kpa,
                r.stage,
                r.phase,
                r.mass_t,
                r.propellant_t,
                r.flight_checksum
            )?
        }
        w.flush()?;
        Ok(())
    }
}
#[derive(Serialize)]
struct ExportRow {
    epoch: u32,
    time_s: f64,
    altitude_km: f64,
    speed_km_s: f64,
    mach: f64,
    dynamic_pressure_kpa: f64,
    stage: u8,
    phase: u8,
    mass_t: f64,
    propellant_t: f64,
    flight_checksum: u32,
}
impl From<&MissionControlUpdate> for ExportRow {
    fn from(v: &MissionControlUpdate) -> Self {
        let p = v.director.position_q12.map(|x| x as f64 / 4096.0);
        let speed = (v
            .director
            .velocity_q24
            .iter()
            .map(|x| (*x as f64 / 16_777_216.0).powi(2))
            .sum::<f64>())
        .sqrt();
        let radius = (p.iter().map(|x| x * x).sum::<f64>()).sqrt();
        Self {
            epoch: v.epoch,
            time_s: v.director.time_q16 as f64 / 65536.0,
            altitude_km: radius - 6371.0,
            speed_km_s: speed,
            mach: v.director.mach_q16 as f64 / 65536.0,
            dynamic_pressure_kpa: v.director.dynamic_pressure_q16 as f64 / 65536.0,
            stage: v.director.active_stage,
            phase: v.director.phase,
            mass_t: v.director.total_mass_q12 as f64 / 4096.0,
            propellant_t: v.director.active_propellant_q12 as f64 / 4096.0,
            flight_checksum: v.status.map(|s| s.flight_checksum).unwrap_or(0),
        }
    }
}
