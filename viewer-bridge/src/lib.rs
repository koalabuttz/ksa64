//! Phase 12A versioned C ABI for truth-blind KSA64 presentation clients.
//! A dedicated worker owns the accepted `LiveMissionSession`; public calls
//! validate/copy input and use bounded queues or immutable polling state.
//!
//! # Safety
//!
//! The checked-in C header is the caller contract for every exported unsafe
//! function: pointers must remain valid for the duration of a call, handles
//! must originate from `ksa64_viewer_start`, and owned buffers must be returned
//! exactly once without modifying their pointer or length.
#![allow(clippy::missing_safety_doc)]

mod presentation;
pub use presentation::*;

use ksa64_host::application::{Ksa64Application, MissionDisplay, MissionPace, MissionRequest};
use ksa64_host::phase11_live::{
    LiveMissionSession, MissionActionReceipt, MissionOperatorAction, MissionSessionEvent,
    MissionSessionEventKind, MissionSessionLifecycle, MissionSessionPace, MissionSessionSnapshot,
};
use ksa64_host::phase11_prediction::HostPrediction;
use ksa64_host::phase11_session::verify_complete_session;
use ksa64_host::phase12b_live::{FullMissionSession, FullMissionSnapshot};
use ksa64_interface::phase11::{
    parse_kua11, parse_kul11, write_kua11, write_kul11, OperationalRole, UplinkCommandLoad,
    UplinkControlKind, UplinkControlRecord, KUA11_LENGTH, KUL11_LENGTH,
};
use std::collections::{hash_map::RandomState, HashMap, VecDeque};
use std::hash::{BuildHasher, Hash, Hasher};
use std::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::slice;
use std::str;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};

pub const KSA64_VIEWER_ABI_VERSION: u32 = 1;
pub const KSA64_VIEWER_BUILD_IDENTITY: u32 = 0x120b_0001;
pub const KSA64_VIEWER_COMMAND_CAPACITY: usize = 32;
pub const KSA64_VIEWER_EVENT_CAPACITY: usize = 256;
pub const KSA64_VIEWER_MAX_ADVANCE_RELEASES: u32 = 64;
pub const KSA64_VIEWER_MAX_CALLER_SPAN: usize = 16 * 1024 * 1024;
pub const KSA64_VIEWER_TIMELINE_CAPACITY: usize = 256;
pub const KSA64_VIEWER_SAMPLE_CAPACITY: usize = 256;
pub const KSA64_VIEWER_FEATURE_PANIC_PROBE: u32 = 1 << 0;
pub const KSA64_VIEWER_FEATURE_OPERATIONS_V1: u32 = 1 << 1;
pub const KSA64_VIEWER_FEATURE_TYPED_ACTIONS_V1: u32 = 1 << 2;
pub const KSA64_VIEWER_FEATURE_ASYNC_STATUS_V1: u32 = 1 << 3;
pub const KSA64_VIEWER_PRESENTATION_ADAPTER_LEGACY: u32 = 0x120b_1001;
pub const VALID_FRAME: u64 = 1 << 0;
pub const VALID_MISSION_TIME: u64 = 1 << 1;
pub const VALID_POSITION: u64 = 1 << 2;
pub const VALID_VELOCITY: u64 = 1 << 3;
pub const VALID_FLIGHT_CHECKSUM: u64 = 1 << 4;
pub const VALID_NAVIGATION_CHECKSUM: u64 = 1 << 5;
pub const VALID_COMMAND_CHECKSUM: u64 = 1 << 6;
pub const VALID_PREDICTION: u64 = 1 << 7;
pub const VALID_EVIDENCE: u64 = 1 << 8;
pub const VALID_STAGED_LOAD: u64 = 1 << 9;
pub const VALID_SAFE: u64 = 1 << 10;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultCode {
    Ok = 0,
    Queued = 1,
    NoData = 2,
    Unchanged = 3,
    InvalidArgument = -1,
    AbiMismatch = -2,
    StructSize = -3,
    InvalidUtf8 = -4,
    Unsupported = -5,
    Lifecycle = -6,
    ActionUnavailable = -7,
    ActionRejected = -8,
    QueueFull = -9,
    Closed = -10,
    Internal = -11,
    Panic = -12,
    EventOverflow = -13,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AbiInfo {
    pub abi_version: u32,
    pub struct_size: u32,
    pub build_identity: u32,
    pub release_hz: u32,
    pub command_capacity: u32,
    pub event_capacity: u32,
    pub maximum_advance_releases: u32,
    pub feature_flags: u32,
    pub catalog_count: u32,
    pub snapshot_size: u32,
    pub event_size: u32,
    pub span_size: u32,
    pub owned_buffer_size: u32,
    pub source_commit: [u8; 16],
    pub target_triple: [u8; 32],
    pub catalog_sha256: [u8; 32],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Span {
    pub abi_version: u32,
    pub struct_size: u32,
    pub data: *const u8,
    pub length: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OwnedBuffer {
    pub abi_version: u32,
    pub struct_size: u32,
    pub data: *mut u8,
    pub length: u64,
    pub allocation_id: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Event {
    pub abi_version: u32,
    pub struct_size: u32,
    pub sequence: u32,
    pub release_epoch: u32,
    pub kind: u32,
    pub detail_identity: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub command_sequence: u64,
    pub command_result: i32,
    pub role: u32,
    pub definition_identity: u32,
    pub lifecycle: u32,
    pub pace: u32,
    pub release_epoch: u32,
    pub release_period_micros: u32,
    pub frame: u32,
    pub mission_time_q16: u32,
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub flight_checksum: u32,
    pub navigation_checksum: u32,
    pub command_checksum: u32,
    pub evidence_identity: u32,
    pub procedure_chain: u32,
    pub journal_chain: u32,
    pub action_chain: u32,
    pub procedure_state: u32,
    pub procedure_step: u32,
    pub staged_load_identity: u32,
    pub action_count: u32,
    pub event_count: u32,
    pub rejected_loads: u32,
    pub safe: u32,
    pub prediction_identity: u32,
    pub prediction_checksum: u32,
    pub prediction_frame: u32,
    pub prediction_terminal_reason: u32,
    pub prediction_apogee_q12_km: i32,
    pub prediction_perigee_q12_km: i32,
    pub prediction_time_to_apogee_q16: u32,
    pub prediction_time_to_impact_q16: u32,
    pub prediction_impact_position_q12_km: [i32; 3],
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            abi_version: KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            command_sequence: 0,
            command_result: 0,
            role: 0,
            definition_identity: 0,
            lifecycle: 0,
            pace: 0,
            release_epoch: 0,
            release_period_micros: 0,
            frame: 0,
            mission_time_q16: 0,
            navigation_position_q12: [0; 3],
            navigation_velocity_q24: [0; 3],
            flight_checksum: 0,
            navigation_checksum: 0,
            command_checksum: 0,
            evidence_identity: 0,
            procedure_chain: 0,
            journal_chain: 0,
            action_chain: 0,
            procedure_state: 0,
            procedure_step: 0,
            staged_load_identity: 0,
            action_count: 0,
            event_count: 0,
            rejected_loads: 0,
            safe: 0,
            prediction_identity: 0,
            prediction_checksum: 0,
            prediction_frame: 0,
            prediction_terminal_reason: 0,
            prediction_apogee_q12_km: 0,
            prediction_perigee_q12_km: 0,
            prediction_time_to_apogee_q16: 0,
            prediction_time_to_impact_q16: 0,
            prediction_impact_position_q12_km: [0; 3],
        }
    }
}

#[derive(Default)]
struct Shared {
    snapshot: Option<Snapshot>,
    operational: Option<OperationalViewV1>,
    procedure: Option<ProcedureViewV1>,
    disposition: Option<DispositionV1>,
    events: VecDeque<Event>,
    timeline: VecDeque<TimelineEventV1>,
    samples: VecDeque<ReleaseSampleV1>,
    prediction_header: Option<PredictionPathHeaderV1>,
    prediction_points: Vec<PredictionPathPointV1>,
    action_proposal: Option<ActionProposalV1>,
    action_receipt: Option<ActionReceiptV1>,
    recommended: Option<Vec<u8>>,
    commit: Option<Vec<u8>>,
    bundle: Option<Vec<u8>>,
    diagnostic: String,
    snapshot_publication: u64,
    action_receipt_publication: u64,
    event_overflow: bool,
    timeline_overflow: bool,
    sample_overflow: bool,
    worker_failed: bool,
    worker_done: bool,
    shutdown_requested: bool,
    last_command_result: i32,
}
enum Command {
    Pause,
    Resume,
    Pace(MissionSessionPace),
    Step,
    Advance(u32),
    Stage(UplinkCommandLoad, u32),
    Commit(UplinkControlRecord),
    Cancel(UplinkControlRecord),
    Abort(u32),
    #[cfg(test)]
    TestBarrier(SyncSender<()>, std::sync::mpsc::Receiver<()>),
    #[cfg(any(test, feature = "panic-probe"))]
    PanicProbe,
    Shutdown,
}
#[repr(C)]
pub struct Handle {
    _private: [u8; 0],
}
struct HandleState {
    commands: SyncSender<Command>,
    shared: Arc<Mutex<Shared>>,
    closed: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
    last_snapshot_publication: AtomicU64,
    last_operational_publication: AtomicU64,
    last_action_receipt_publication: AtomicU64,
    pending_commands: Arc<AtomicUsize>,
}
static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(1);
static HANDLE_HASHER: OnceLock<RandomState> = OnceLock::new();
static HANDLES: OnceLock<Mutex<HashMap<usize, Arc<HandleState>>>> = OnceLock::new();
static NEXT_BUFFER: AtomicUsize = AtomicUsize::new(1);
static BUFFERS: OnceLock<Mutex<HashMap<u64, (usize, usize)>>> = OnceLock::new();
static LIBRARY_DIAGNOSTIC: OnceLock<Mutex<String>> = OnceLock::new();
fn next_handle_token() -> usize {
    loop {
        let sequence = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        let mut hasher = HANDLE_HASHER.get_or_init(RandomState::new).build_hasher();
        sequence.hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        let token = hasher.finish() as usize;
        if token != 0 {
            return token;
        }
    }
}
fn handles() -> &'static Mutex<HashMap<usize, Arc<HandleState>>> {
    HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn buffers() -> &'static Mutex<HashMap<u64, (usize, usize)>> {
    BUFFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn fixed_bytes<const N: usize>(text: &str) -> [u8; N] {
    let mut output = [0_u8; N];
    let bytes = text.as_bytes();
    let count = bytes.len().min(N);
    output[..count].copy_from_slice(&bytes[..count]);
    output
}
fn library_diagnostic() -> &'static Mutex<String> {
    LIBRARY_DIAGNOSTIC.get_or_init(|| Mutex::new(String::new()))
}
fn set_library_diagnostic(message: impl Into<String>) {
    if let Ok(mut diagnostic) = library_diagnostic().lock() {
        *diagnostic = message.into();
    }
}
fn validate(abi: u32, size: u32, expected: usize) -> Result<(), ResultCode> {
    if abi != KSA64_VIEWER_ABI_VERSION {
        return Err(ResultCode::AbiMismatch);
    }
    if size as usize != expected {
        return Err(ResultCode::StructSize);
    }
    Ok(())
}
unsafe fn copy_span(span: *const Span) -> Result<Vec<u8>, ResultCode> {
    if span.is_null() {
        return Err(ResultCode::InvalidArgument);
    }
    let span = unsafe { &*span };
    validate(span.abi_version, span.struct_size, size_of::<Span>())?;
    let length = usize::try_from(span.length).map_err(|_| ResultCode::InvalidArgument)?;
    if length > isize::MAX as usize || length > KSA64_VIEWER_MAX_CALLER_SPAN {
        return Err(ResultCode::InvalidArgument);
    }
    if length == 0 {
        return Ok(Vec::new());
    }
    if span.data.is_null() {
        return Err(ResultCode::InvalidArgument);
    }
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(length)
        .map_err(|_| ResultCode::Internal)?;
    owned.extend_from_slice(unsafe { slice::from_raw_parts(span.data, length) });
    Ok(owned)
}
fn owned(bytes: Vec<u8>) -> Result<OwnedBuffer, ResultCode> {
    if bytes.is_empty() {
        return Err(ResultCode::InvalidArgument);
    }
    let b = bytes.into_boxed_slice();
    let n = b.len();
    let data = Box::into_raw(b) as *mut u8;
    let allocation_id = NEXT_BUFFER.fetch_add(1, Ordering::Relaxed) as u64;
    if allocation_id == 0 {
        unsafe { drop(Box::from_raw(ptr::slice_from_raw_parts_mut(data, n))) }
        return Err(ResultCode::Internal);
    }
    let mut registry = buffers().lock().map_err(|_| ResultCode::Internal)?;
    registry.insert(allocation_id, (data as usize, n));
    Ok(OwnedBuffer {
        abi_version: KSA64_VIEWER_ABI_VERSION,
        struct_size: size_of::<OwnedBuffer>() as u32,
        data,
        length: n as u64,
        allocation_id,
    })
}
fn role_id(s: &str) -> Option<u32> {
    Some(match s {
        "observer" => 1,
        "guided-operator" => 2,
        "flight-controller" => 3,
        "flight-software-engineer" => 4,
        "sim-director" => 5,
        "scripted-operator" => 6,
        _ => return None,
    })
}
fn role_name(id: u32) -> Option<&'static str> {
    Some(match id {
        1 => "observer",
        2 => "guided-operator",
        3 => "flight-controller",
        4 => "flight-software-engineer",
        5 => "sim-director",
        6 => "scripted-operator",
        _ => return None,
    })
}
fn lifecycle(v: MissionSessionLifecycle) -> u32 {
    match v {
        MissionSessionLifecycle::Compiled => 1,
        MissionSessionLifecycle::Ready => 2,
        MissionSessionLifecycle::Running => 3,
        MissionSessionLifecycle::Paused => 4,
        MissionSessionLifecycle::Completed => 5,
        MissionSessionLifecycle::Aborted => 6,
    }
}
fn pace(v: MissionSessionPace) -> u32 {
    match v {
        MissionSessionPace::Fast => 1,
        MissionSessionPace::Realtime => 2,
        MissionSessionPace::Paused => 3,
        MissionSessionPace::SingleStep => 4,
    }
}
fn event_kind(v: MissionSessionEventKind) -> u32 {
    match v {
        MissionSessionEventKind::Compiled => 1,
        MissionSessionEventKind::Prepared => 2,
        MissionSessionEventKind::Release => 3,
        MissionSessionEventKind::Paused => 4,
        MissionSessionEventKind::Resumed => 5,
        MissionSessionEventKind::PaceChanged => 6,
        MissionSessionEventKind::ActionStaged => 7,
        MissionSessionEventKind::ActionCommitted => 8,
        MissionSessionEventKind::ActionCancelled => 9,
        MissionSessionEventKind::ActionRejected => 10,
        MissionSessionEventKind::Completed => 11,
        MissionSessionEventKind::Aborted => 12,
    }
}
fn bridge_event(v: &MissionSessionEvent) -> Event {
    Event {
        abi_version: KSA64_VIEWER_ABI_VERSION,
        struct_size: size_of::<Event>() as u32,
        sequence: v.sequence,
        release_epoch: v.release_epoch,
        kind: event_kind(v.kind),
        detail_identity: v.detail_identity,
    }
}
fn bridge_snapshot(
    v: &MissionSessionSnapshot,
    role: u32,
    seq: u64,
    result: ResultCode,
) -> Snapshot {
    let mut o = Snapshot {
        command_sequence: seq,
        command_result: result as i32,
        role,
        definition_identity: v.definition_identity,
        lifecycle: lifecycle(v.lifecycle),
        pace: pace(v.pace),
        release_epoch: v.release_epoch,
        release_period_micros: v.release_period_micros,
        procedure_chain: v.procedure_chain,
        journal_chain: v.journal_chain,
        action_chain: v.action_chain,
        procedure_state: v.procedure_state as u32 + 1,
        procedure_step: u32::from(v.procedure_step),
        action_count: u32::try_from(v.action_count).unwrap_or(u32::MAX),
        event_count: u32::try_from(v.event_count).unwrap_or(u32::MAX),
        rejected_loads: u32::from(v.rejected_loads),
        ..Snapshot::default()
    };
    if let Some(x) = v.frame {
        o.validity_mask |= VALID_FRAME;
        o.frame = x as u32
    }
    if let Some(x) = v.mission_time_q16 {
        o.validity_mask |= VALID_MISSION_TIME;
        o.mission_time_q16 = x
    }
    if let Some(x) = v.navigation_position_q12 {
        o.validity_mask |= VALID_POSITION;
        o.navigation_position_q12 = x
    }
    if let Some(x) = v.navigation_velocity_q24 {
        o.validity_mask |= VALID_VELOCITY;
        o.navigation_velocity_q24 = x
    }
    if let Some(x) = v.flight_checksum {
        o.validity_mask |= VALID_FLIGHT_CHECKSUM;
        o.flight_checksum = x
    }
    if let Some(x) = v.navigation_checksum {
        o.validity_mask |= VALID_NAVIGATION_CHECKSUM;
        o.navigation_checksum = x
    }
    if let Some(x) = v.command_checksum {
        o.validity_mask |= VALID_COMMAND_CHECKSUM;
        o.command_checksum = x
    }
    if let Some(x) = v.evidence_identity {
        o.validity_mask |= VALID_EVIDENCE;
        o.evidence_identity = x
    }
    if let Some(x) = v.staged_load_identity {
        o.validity_mask |= VALID_STAGED_LOAD;
        o.staged_load_identity = x
    }
    if let Some(x) = v.safe {
        o.validity_mask |= VALID_SAFE;
        o.safe = u32::from(x)
    }
    if let Some(x) = v.prediction {
        o.validity_mask |= VALID_PREDICTION;
        o.prediction_identity = x.prediction_identity;
        o.prediction_checksum = x.prediction_checksum;
        o.prediction_frame = x.frame as u32;
        o.prediction_terminal_reason = x.terminal_reason as u32;
        o.prediction_apogee_q12_km = x.apogee_q12_km;
        o.prediction_perigee_q12_km = x.perigee_q12_km;
        o.prediction_time_to_apogee_q16 = x.time_to_apogee_q16;
        o.prediction_time_to_impact_q16 = x.time_to_impact_q16;
        o.prediction_impact_position_q12_km = x.impact_position_q12_km
    }
    o
}

fn copy_fixed_text<const N: usize>(text: &str) -> ([u8; N], u32) {
    let mut output = [0_u8; N];
    let count = text.len().min(N);
    output[..count].copy_from_slice(&text.as_bytes()[..count]);
    (output, count as u32)
}

fn procedure_text(step: u32) -> (&'static str, &'static str) {
    match step {
        0 => (
            "GNSS LOSS / DETECT",
            "Monitor GNSS validity and confirm inertial propagation remains healthy.",
        ),
        1 => (
            "GNSS LOSS / VERIFY",
            "Compare onboard navigation with the independent ground estimate.",
        ),
        2 => (
            "GROUND UPDATE / REVIEW",
            "Review the proposed ground navigation update and its execution window.",
        ),
        3 => (
            "GROUND UPDATE / STAGE",
            "Stage the complete identity-bound navigation update.",
        ),
        4 => (
            "GROUND UPDATE / COMMIT",
            "Commit the staged update for its declared 32 Hz release.",
        ),
        5 => (
            "CONTINGENCY / REVIEW",
            "Review the continuation branch and its mission consequences.",
        ),
        6 => (
            "CONTINGENCY / STAGE",
            "Stage the bounded contingency-branch selection.",
        ),
        7 => (
            "CONTINGENCY / COMMIT",
            "Commit the selected branch for its declared release.",
        ),
        _ => (
            "PROCEDURE / COMPLETE",
            "Continue monitoring vehicle, avionics, and evidence health.",
        ),
    }
}

fn bridge_procedure(snapshot: &Snapshot) -> ProcedureViewV1 {
    let mut output = ProcedureViewV1 {
        procedure_identity: 0x11c0_1001,
        state: snapshot.procedure_state,
        active_step: snapshot.procedure_step,
        step_count: 9,
        entered_epoch: snapshot.release_epoch,
        deadline_epoch: if snapshot.release_epoch
            < ksa64_host::phase12b::DECISION_WINDOW_CLOSE_RELEASE
        {
            ksa64_host::phase12b::DECISION_WINDOW_CLOSE_RELEASE
        } else {
            snapshot.release_epoch
        },
        ..ProcedureViewV1::default()
    };
    output.validity_mask = VIEW_VALID_PROCEDURE;
    let (title, instruction) = procedure_text(snapshot.procedure_step);
    (output.title, output.title_length) = copy_fixed_text(title);
    (output.instruction, output.instruction_length) = copy_fixed_text(instruction);
    output
}

fn bridge_disposition(snapshot: &Snapshot) -> DispositionV1 {
    use ksa64_host::phase12b::{
        classify_disposition, AvionicsDisposition, EvidenceDisposition,
        MissionObjectiveDisposition, OperationalDispositionEvidence, OperatorDisposition,
        ProcedureDisposition, VehicleDisposition,
    };
    if snapshot.lifecycle != 5 && snapshot.lifecycle != 6 {
        return DispositionV1::default();
    }
    let axes = if snapshot.lifecycle == 6 {
        OperationalDispositionEvidence {
            objective: MissionObjectiveDisposition::NotAchieved,
            vehicle: VehicleDisposition::Unknown,
            procedure: ProcedureDisposition::Failed,
            operator: OperatorDisposition::RejectedAction,
            avionics: AvionicsDisposition::Failed,
            evidence: EvidenceDisposition::ObservationIncomplete,
        }
    } else {
        OperationalDispositionEvidence {
            objective: MissionObjectiveDisposition::PrimaryAchieved,
            vehicle: if snapshot.safe == 1 {
                VehicleDisposition::SafeState
            } else {
                VehicleDisposition::Recovered
            },
            procedure: if snapshot.rejected_loads == 0 {
                ProcedureDisposition::Completed
            } else {
                ProcedureDisposition::Mistimed
            },
            operator: if snapshot.action_count == 0 {
                OperatorDisposition::NoAction
            } else {
                OperatorDisposition::TimelyReference
            },
            avionics: if snapshot.safe == 1 {
                AvionicsDisposition::SafeRecovery
            } else {
                AvionicsDisposition::DegradedOperational
            },
            evidence: if snapshot.validity_mask & VALID_EVIDENCE != 0 {
                EvidenceDisposition::Complete
            } else {
                EvidenceDisposition::ObservationIncomplete
            },
        }
    };
    let classified = classify_disposition(axes);
    DispositionV1 {
        validity_mask: VIEW_VALID_DISPOSITION
            | if axes.evidence == EvidenceDisposition::Complete {
                VIEW_VALID_EVIDENCE
            } else {
                0
            },
        overall: classified.overall as u32,
        objective: axes.objective as u32,
        vehicle: axes.vehicle as u32,
        procedure: axes.procedure as u32,
        operator: axes.operator as u32,
        avionics: axes.avionics as u32,
        evidence: axes.evidence as u32,
        reason_identity: 0,
        ..DispositionV1::default()
    }
}

fn bridge_operational(
    snapshot: &Snapshot,
    scenario_identity: u32,
    publication: u64,
) -> OperationalViewV1 {
    let mut output = OperationalViewV1 {
        publication_sequence: publication,
        scenario_identity,
        execution_adapter_identity: KSA64_VIEWER_PRESENTATION_ADAPTER_LEGACY,
        role: snapshot.role,
        lifecycle: snapshot.lifecycle,
        pace: snapshot.pace,
        release_epoch: snapshot.release_epoch,
        release_period_micros: snapshot.release_period_micros,
        frame: snapshot.frame,
        mission_time_q16: snapshot.mission_time_q16,
        navigation_position_q12: snapshot.navigation_position_q12,
        navigation_velocity_q24: snapshot.navigation_velocity_q24,
        flight_checksum: snapshot.flight_checksum,
        navigation_checksum: snapshot.navigation_checksum,
        command_checksum: snapshot.command_checksum,
        procedure_state: snapshot.procedure_state,
        procedure_step: snapshot.procedure_step,
        staged_load_identity: snapshot.staged_load_identity,
        action_count: snapshot.action_count,
        rejected_loads: snapshot.rejected_loads,
        safe: snapshot.safe,
        gnss_state: if snapshot.release_epoch == 0 { 1 } else { 2 },
        prediction_identity: snapshot.prediction_identity,
        prediction_checksum: snapshot.prediction_checksum,
        prediction_apogee_q12_km: snapshot.prediction_apogee_q12_km,
        prediction_time_to_apogee_q16: snapshot.prediction_time_to_apogee_q16,
        prediction_time_to_impact_q16: snapshot.prediction_time_to_impact_q16,
        ..OperationalViewV1::default()
    };
    if snapshot.validity_mask & VALID_MISSION_TIME != 0 {
        output.validity_mask |= VIEW_VALID_MISSION_TIME;
    }
    if snapshot.validity_mask & (VALID_POSITION | VALID_VELOCITY) != 0 {
        output.validity_mask |= VIEW_VALID_NAVIGATION;
    }
    if snapshot.validity_mask & VALID_PREDICTION != 0 {
        output.validity_mask |= VIEW_VALID_PREDICTION;
    }
    output.validity_mask |= VIEW_VALID_PROCEDURE | VIEW_VALID_GNSS;
    if snapshot.staged_load_identity != 0 {
        output.validity_mask |= VIEW_VALID_ACTION;
    }
    if snapshot.validity_mask & VALID_EVIDENCE != 0 {
        output.validity_mask |= VIEW_VALID_EVIDENCE;
    }
    if snapshot.lifecycle == 5 || snapshot.lifecycle == 6 {
        output.validity_mask |= VIEW_VALID_DISPOSITION;
    }
    output
}

fn bridge_timeline(event: &MissionSessionEvent) -> TimelineEventV1 {
    let (source, severity, label) = match event.kind {
        MissionSessionEventKind::Compiled => (6, 1, "Session definition compiled"),
        MissionSessionEventKind::Prepared => (6, 1, "Session ready"),
        MissionSessionEventKind::Release => (2, 1, "Avionics release"),
        MissionSessionEventKind::Paused => (5, 1, "Operations paused"),
        MissionSessionEventKind::Resumed => (5, 1, "Operations resumed"),
        MissionSessionEventKind::PaceChanged => (5, 1, "Presentation pace changed"),
        MissionSessionEventKind::ActionStaged => (5, 1, "Uplink load staged"),
        MissionSessionEventKind::ActionCommitted => (5, 1, "Uplink load committed"),
        MissionSessionEventKind::ActionCancelled => (5, 2, "Uplink load cancelled"),
        MissionSessionEventKind::ActionRejected => (5, 3, "Uplink action rejected"),
        MissionSessionEventKind::Completed => (6, 1, "Mission evidence completed"),
        MissionSessionEventKind::Aborted => (6, 3, "Session aborted"),
    };
    let (label, label_length) = copy_fixed_text(label);
    TimelineEventV1 {
        sequence: event.sequence,
        release_epoch: event.release_epoch,
        source,
        severity,
        event_identity: event_kind(event.kind),
        detail_identity: event.detail_identity,
        label_length,
        label,
        ..TimelineEventV1::default()
    }
}

fn bridge_sample(snapshot: &Snapshot) -> ReleaseSampleV1 {
    let mut output = ReleaseSampleV1 {
        release_epoch: snapshot.release_epoch,
        mission_time_q16: snapshot.mission_time_q16,
        frame: snapshot.frame,
        onboard_position_q12: snapshot.navigation_position_q12,
        onboard_velocity_q24: snapshot.navigation_velocity_q24,
        predicted_impact_q12: snapshot.prediction_impact_position_q12_km,
        predicted_apogee_q12_km: snapshot.prediction_apogee_q12_km,
        altitude_q12_km: snapshot.navigation_position_q12[2],
        downrange_q12_km: snapshot.navigation_position_q12[0],
        crossrange_q12_km: snapshot.navigation_position_q12[1],
        ..ReleaseSampleV1::default()
    };
    if snapshot.validity_mask & VALID_MISSION_TIME != 0 {
        output.validity_mask |= VIEW_VALID_MISSION_TIME;
    }
    if snapshot.validity_mask & (VALID_POSITION | VALID_VELOCITY) != 0 {
        output.validity_mask |= VIEW_VALID_NAVIGATION;
    }
    if snapshot.validity_mask & VALID_PREDICTION != 0 {
        output.validity_mask |= VIEW_VALID_PREDICTION;
    }
    output
}

fn bridge_prediction(
    snapshot: &Snapshot,
) -> (Option<PredictionPathHeaderV1>, Vec<PredictionPathPointV1>) {
    if snapshot.validity_mask & (VALID_PREDICTION | VALID_POSITION)
        != (VALID_PREDICTION | VALID_POSITION)
    {
        return (None, Vec::new());
    }
    let path_identity = snapshot.prediction_identity ^ 0x120b_5001;
    let points = vec![
        PredictionPathPointV1 {
            path_identity,
            point_index: 0,
            release_epoch: snapshot.release_epoch,
            frame: snapshot.frame,
            position_q12_km: snapshot.navigation_position_q12,
            altitude_q12_km: snapshot.navigation_position_q12[2],
            ..PredictionPathPointV1::default()
        },
        PredictionPathPointV1 {
            path_identity,
            point_index: 1,
            release_epoch: snapshot
                .release_epoch
                .saturating_add(snapshot.prediction_time_to_impact_q16 / 2_048),
            frame: snapshot.prediction_frame,
            flags: 1,
            position_q12_km: snapshot.prediction_impact_position_q12_km,
            altitude_q12_km: snapshot.prediction_impact_position_q12_km[2],
            ..PredictionPathPointV1::default()
        },
    ];
    let header = PredictionPathHeaderV1 {
        validity_mask: VIEW_VALID_PREDICTION,
        path_identity,
        product: 1,
        model_identity: snapshot.prediction_identity,
        source_estimate_identity: snapshot.navigation_checksum,
        source_estimate_checksum: snapshot.navigation_checksum,
        source_epoch: snapshot.release_epoch,
        generation_epoch: snapshot.release_epoch,
        frame: snapshot.prediction_frame,
        terminal_reason: snapshot.prediction_terminal_reason,
        point_count: points.len() as u32,
        cadence_releases: 32,
        path_checksum: snapshot.prediction_checksum,
        ..PredictionPathHeaderV1::default()
    };
    (Some(header), points)
}

fn proposal_from_bytes(bytes: &[u8]) -> Option<ActionProposalV1> {
    let load = parse_kul11(bytes).ok()?;
    let payload_checksum = u32::from_le_bytes(bytes.get(508..512)?.try_into().ok()?);
    let label = match load.load_type {
        ksa64_interface::phase11::UplinkLoadType::GroundNavigationUpdate => {
            "Review ground navigation update"
        }
        ksa64_interface::phase11::UplinkLoadType::MissionEventTarget => {
            "Review mission-event target"
        }
        ksa64_interface::phase11::UplinkLoadType::ContingencyBranch => "Review contingency branch",
        ksa64_interface::phase11::UplinkLoadType::NavigationMode => {
            "Review navigation-mode request"
        }
        ksa64_interface::phase11::UplinkLoadType::HighLevelMode => "Review high-level mode request",
    };
    let (label, label_length) = copy_fixed_text(label);
    Some(ActionProposalV1 {
        validity_mask: VIEW_VALID_ACTION,
        proposal_identity: load.load_identity,
        load_identity: load.load_identity,
        load_type: load.load_type as u32,
        stage_epoch: load.stage_epoch,
        earliest_commit_epoch: load.not_before_epoch.saturating_sub(2),
        activation_epoch: load.requested_effective_epoch,
        expires_epoch: load.expires_epoch,
        payload_checksum,
        completed_event_mask: load.prerequisite_event_mask,
        permitted_operations: 1,
        label_length,
        label,
        ..ActionProposalV1::default()
    })
}

fn bridge_receipt(
    receipt: MissionActionReceipt,
    operation: u32,
    publication: u64,
) -> ActionReceiptV1 {
    ActionReceiptV1 {
        validity_mask: VIEW_VALID_ACTION,
        publication_sequence: publication,
        proposal_identity: receipt.record.load_identity,
        load_identity: receipt.record.load_identity,
        control_identity: receipt.record.control_identity,
        receipt_epoch: receipt.record.request_epoch,
        effective_epoch: receipt.record.effective_epoch,
        state: receipt.record.state as u32,
        reason: receipt.record.reason as u32,
        accepted: u32::from(receipt.accepted),
        operation,
        receipt_checksum: receipt.record.receipt_checksum,
        ..ActionReceiptV1::default()
    }
}

fn operational_role(identity: u32) -> Option<OperationalRole> {
    Some(match identity {
        1 => OperationalRole::Observer,
        2 => OperationalRole::GuidedOperator,
        3 => OperationalRole::FlightController,
        4 => OperationalRole::FlightSoftwareEngineer,
        5 => OperationalRole::SimDirector,
        6 => OperationalRole::ScriptedOperator,
        _ => return None,
    })
}

fn full_snapshot_legacy(
    snapshot: &FullMissionSnapshot,
    role: u32,
    sequence: u64,
    result: ResultCode,
) -> Snapshot {
    let mut output = Snapshot {
        command_sequence: sequence,
        command_result: result as i32,
        role,
        definition_identity: ksa64_host::phase12b::FULL_GNSS_LOSS_DEFINITION_ID,
        lifecycle: lifecycle(snapshot.lifecycle),
        pace: pace(snapshot.pace),
        release_epoch: snapshot.release_epoch,
        release_period_micros: 31_250,
        mission_time_q16: snapshot.mission_time_q16,
        procedure_state: u32::from(snapshot.procedure.is_some()),
        procedure_step: snapshot
            .procedure
            .as_ref()
            .map_or(0, |value| u32::from(value.active_step)),
        action_count: snapshot.action_count,
        event_count: snapshot.event_count,
        rejected_loads: u32::from(snapshot.rejected_loads),
        ..Snapshot::default()
    };
    output.validity_mask |= VALID_MISSION_TIME;
    if let Some(flight) = snapshot.flight {
        output.validity_mask |= VALID_FRAME
            | VALID_POSITION
            | VALID_VELOCITY
            | VALID_FLIGHT_CHECKSUM
            | VALID_NAVIGATION_CHECKSUM
            | VALID_COMMAND_CHECKSUM
            | VALID_SAFE;
        output.frame = flight.navigation.frame as u32;
        output.navigation_position_q12 = flight.navigation.position_q12;
        output.navigation_velocity_q24 = flight.navigation.velocity_q24;
        output.flight_checksum = flight.flight_checksum;
        output.navigation_checksum = flight.navigation.checksum;
        output.command_checksum = flight.command.command_checksum;
        output.safe = u32::from(flight.safe);
    }
    if let Some(action) = snapshot.recommended_action {
        output.validity_mask |= VALID_STAGED_LOAD;
        output.staged_load_identity = action.proposal_identity;
    }
    if let Some(prediction) = snapshot.latest_onboard_prediction.as_ref() {
        let summary = prediction.summary;
        output.validity_mask |= VALID_PREDICTION;
        output.prediction_identity = summary.prediction_identity;
        output.prediction_checksum = summary.prediction_checksum;
        output.prediction_frame = summary.frame as u32;
        output.prediction_terminal_reason = summary.terminal_reason as u32;
        output.prediction_apogee_q12_km = summary.apogee_q12_km;
        output.prediction_perigee_q12_km = summary.perigee_q12_km;
        output.prediction_time_to_apogee_q16 = summary.time_to_apogee_q16;
        output.prediction_time_to_impact_q16 = summary.time_to_impact_q16;
        output.prediction_impact_position_q12_km = summary.impact_position_q12_km;
    }
    output
}

fn full_procedure(snapshot: &FullMissionSnapshot) -> ProcedureViewV1 {
    let Some(value) = snapshot.procedure.as_ref() else {
        return ProcedureViewV1::default();
    };
    let mut output = ProcedureViewV1 {
        validity_mask: VIEW_VALID_PROCEDURE,
        procedure_identity: value.procedure_identity,
        state: 1,
        active_step: u32::from(value.active_step),
        step_count: u32::from(value.step_count),
        entered_epoch: value.entered_epoch,
        deadline_epoch: value.deadline_epoch,
        predicate_count: value.predicates.len().min(PROCEDURE_PREDICATE_CAPACITY) as u32,
        ..ProcedureViewV1::default()
    };
    for (index, predicate) in value
        .predicates
        .iter()
        .take(PROCEDURE_PREDICATE_CAPACITY)
        .enumerate()
    {
        output.predicate_identities[index] = u32::from(predicate.predicate_id);
        output.predicate_states[index] = if !predicate.valid {
            0
        } else if predicate.satisfied {
            2
        } else {
            1
        };
    }
    (output.title, output.title_length) = copy_fixed_text(&value.title);
    (output.instruction, output.instruction_length) = copy_fixed_text(&value.instruction);
    output
}

fn full_disposition(snapshot: &FullMissionSnapshot) -> DispositionV1 {
    let Some(value) = snapshot.disposition else {
        return DispositionV1::default();
    };
    DispositionV1 {
        validity_mask: VIEW_VALID_DISPOSITION | VIEW_VALID_EVIDENCE,
        overall: value.overall as u32,
        objective: value.axes.objective as u32,
        vehicle: value.axes.vehicle as u32,
        procedure: value.axes.procedure as u32,
        operator: value.axes.operator as u32,
        avionics: value.axes.avionics as u32,
        evidence: value.axes.evidence as u32,
        ..DispositionV1::default()
    }
}

fn full_operational(
    snapshot: &FullMissionSnapshot,
    role: u32,
    publication: u64,
) -> OperationalViewV1 {
    let legacy = full_snapshot_legacy(snapshot, role, publication, ResultCode::Ok);
    let mut output = bridge_operational(&legacy, SCENARIO_FULL_GNSS_LOSS, publication);
    output.execution_adapter_identity = 0x120b_1002;
    output.gnss_state = if snapshot.release_epoch < ksa64_host::phase12b::GNSS_LOSS_RELEASE {
        1
    } else if snapshot.release_epoch < ksa64_host::phase12b::GNSS_QUALIFIED_RELEASE {
        2
    } else {
        3
    };
    if let Some(ground) = snapshot.ground {
        output.validity_mask |= VIEW_VALID_GROUND_ESTIMATE;
        output.ground_position_q12 = ground.position_q12_km;
        output.ground_velocity_q24 = ground.velocity_q24_km_s;
    }
    if snapshot.procedure.is_none() {
        output.validity_mask &= !VIEW_VALID_PROCEDURE;
    }
    // Completion is not evidence until the sealed KSB11 has passed Rust-side
    // verification. The worker adds these bits atomically with the verified bundle.
    output.validity_mask &= !(VIEW_VALID_DISPOSITION | VIEW_VALID_EVIDENCE);
    output
}

fn full_action_proposal(snapshot: &FullMissionSnapshot) -> Option<ActionProposalV1> {
    let value = snapshot.recommended_action?;
    let label = match value.load_type {
        ksa64_interface::phase11::UplinkLoadType::GroundNavigationUpdate => {
            "Review independent ground navigation update"
        }
        ksa64_interface::phase11::UplinkLoadType::ContingencyBranch => {
            "Review mission continuation branch"
        }
        _ => "Review bounded mission command",
    };
    let (label, label_length) = copy_fixed_text(label);
    Some(ActionProposalV1 {
        validity_mask: VIEW_VALID_ACTION,
        proposal_identity: value.proposal_identity,
        load_identity: value.proposal_identity,
        load_type: value.load_type as u32,
        earliest_commit_epoch: value.earliest_commit_epoch,
        activation_epoch: value.activation_epoch,
        expires_epoch: value.expires_epoch,
        payload_checksum: value.payload_checksum,
        permitted_operations: 1,
        label_length,
        label,
        ..ActionProposalV1::default()
    })
}

fn full_timeline(
    value: &ksa64_host::phase12b::TimelineEventView,
    sequence: u32,
) -> TimelineEventV1 {
    let (label, label_length) = copy_fixed_text(&value.label);
    TimelineEventV1 {
        sequence,
        release_epoch: value.epoch,
        source: value.source as u32,
        severity: u32::from(value.severity),
        event_identity: value.event_identity,
        detail_identity: value.event_identity,
        label_length,
        label,
        ..TimelineEventV1::default()
    }
}

fn full_release_sample(value: ksa64_host::phase12b::ReleaseSampleView) -> ReleaseSampleV1 {
    ReleaseSampleV1 {
        validity_mask: VIEW_VALID_MISSION_TIME | VIEW_VALID_NAVIGATION | VIEW_VALID_GROUND_ESTIMATE,
        release_epoch: value.epoch,
        mission_time_q16: value.mission_time_q16,
        flags: value.flags,
        onboard_position_q12: [
            value.downrange_q12_km,
            value.crossrange_q12_km,
            value.onboard_altitude_q12_km,
        ],
        ground_position_q12: [
            value.downrange_q12_km,
            value.crossrange_q12_km,
            value.ground_altitude_q12_km,
        ],
        predicted_apogee_q12_km: value.onboard_altitude_q12_km,
        altitude_q12_km: value.altitude_q12_km,
        speed_q24_km_s: value.speed_q24_km_s,
        downrange_q12_km: value.downrange_q12_km,
        crossrange_q12_km: value.crossrange_q12_km,
        ..ReleaseSampleV1::default()
    }
}

fn full_prediction(
    prediction: Option<&HostPrediction>,
) -> (Option<PredictionPathHeaderV1>, Vec<PredictionPathPointV1>) {
    let Some(prediction) = prediction else {
        return (None, Vec::new());
    };
    let header = PredictionPathHeaderV1 {
        validity_mask: VIEW_VALID_PREDICTION,
        path_identity: prediction.header.path_identity,
        product: prediction.header.product as u32,
        model_identity: prediction.header.model_identity,
        source_estimate_identity: prediction.header.source_estimate_identity,
        source_estimate_checksum: prediction.header.source_estimate_checksum,
        source_epoch: prediction.header.source_epoch,
        generation_epoch: prediction.header.generation_epoch,
        frame: prediction
            .points
            .first()
            .map_or(0, |point| point.frame as u32),
        terminal_reason: prediction.header.terminal_reason as u32,
        point_count: prediction.points.len() as u32,
        cadence_releases: u32::from(prediction.header.cadence_releases),
        path_checksum: prediction.header.path_checksum,
        ..PredictionPathHeaderV1::default()
    };
    let points = prediction
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| PredictionPathPointV1 {
            path_identity: prediction.header.path_identity,
            point_index: index as u32,
            release_epoch: point.epoch,
            frame: point.frame as u32,
            flags: u32::from(point.flags),
            position_q12_km: point.position_q12_km,
            altitude_q12_km: point.altitude_q12_km,
            downrange_q12_km: point.downrange_q12_km,
            crossrange_q12_km: point.crossrange_q12_km,
            ..PredictionPathPointV1::default()
        })
        .collect();
    (Some(header), points)
}

fn map_error(e: ksa64_host::phase11_live::MissionSessionError) -> ResultCode {
    use ksa64_host::phase11_live::MissionSessionError::*;
    match e {
        Unsupported | Authoring => ResultCode::Unsupported,
        Lifecycle | NotCompleted => ResultCode::Lifecycle,
        ActionUnavailable => ResultCode::ActionUnavailable,
        ActionRejected => ResultCode::ActionRejected,
        Procedure => ResultCode::Internal,
    }
}
fn diag(shared: &Arc<Mutex<Shared>>, s: impl Into<String>) {
    if let Ok(mut v) = shared.lock() {
        v.diagnostic = s.into()
    }
}
fn publish(
    shared: &Arc<Mutex<Shared>>,
    session: &LiveMissionSession,
    role: u32,
    scenario_identity: u32,
    seq: u64,
    result: ResultCode,
    cursor: &mut u32,
) -> Result<(), ResultCode> {
    let snap = session.snapshot();
    let events = session.events_after(*cursor);
    let recommended = if let Some(load) = session.recommended_load() {
        let mut bytes = vec![0; KUL11_LENGTH];
        write_kul11(&load, &mut bytes).map_err(|_| ResultCode::Internal)?;
        Some(bytes)
    } else {
        None
    };
    let commit = if let Some(request) = session.commit_request_for_staged() {
        let mut bytes = vec![0; KUA11_LENGTH];
        write_kua11(&request, &mut bytes).map_err(|_| ResultCode::Internal)?;
        Some(bytes)
    } else {
        None
    };
    let action_proposal = recommended.as_deref().and_then(proposal_from_bytes);
    let bridge = bridge_snapshot(&snap, role, seq, result);
    let procedure = bridge_procedure(&bridge);
    let disposition = bridge_disposition(&bridge);
    let (prediction_header, prediction_points) = bridge_prediction(&bridge);
    let sample = bridge_sample(&bridge);
    let mut state = shared.lock().map_err(|_| ResultCode::Internal)?;
    state.snapshot_publication = state.snapshot_publication.saturating_add(1);
    state.snapshot = Some(bridge);
    state.operational = Some(bridge_operational(
        &bridge,
        scenario_identity,
        state.snapshot_publication,
    ));
    state.procedure = Some(procedure);
    state.disposition = Some(disposition);
    state.action_proposal = action_proposal;
    state.prediction_header = prediction_header;
    state.prediction_points = prediction_points;
    state.last_command_result = result as i32;
    for event in events {
        if state.events.len() == KSA64_VIEWER_EVENT_CAPACITY {
            state.event_overflow = true;
            state.diagnostic = format!(
                "event queue overflow at session event {}; stream is incomplete",
                event.sequence
            );
        } else if !state.event_overflow {
            state.events.push_back(bridge_event(event));
        }
        if state.timeline.len() == KSA64_VIEWER_TIMELINE_CAPACITY {
            state.timeline_overflow = true;
        } else if !state.timeline_overflow {
            state.timeline.push_back(bridge_timeline(event));
        }
        *cursor = event.sequence;
    }
    let new_sample = state
        .samples
        .back()
        .is_none_or(|previous| previous.release_epoch != sample.release_epoch)
        && (sample.release_epoch.is_multiple_of(32)
            || bridge.lifecycle == 5
            || bridge.lifecycle == 6);
    if new_sample {
        if state.samples.len() == KSA64_VIEWER_SAMPLE_CAPACITY {
            state.samples.pop_front();
            state.sample_overflow = true;
        }
        state.samples.push_back(sample);
    }
    state.recommended = recommended;
    state.commit = commit;
    Ok(())
}
fn apply(
    command: Command,
    live: &mut LiveMissionSession,
) -> Result<(bool, Option<(MissionActionReceipt, u32)>), ResultCode> {
    let mut receipt = None;
    match command {
        Command::Pause => live.pause().map_err(map_error)?,
        Command::Resume => live.resume().map_err(map_error)?,
        Command::Pace(x) => live.set_pace(x).map_err(map_error)?,
        Command::Step => {
            live.step_one_release().map_err(map_error)?;
        }
        Command::Advance(x) => {
            for _ in 0..x {
                let before = live.snapshot().release_epoch;
                live.advance_bounded(1).map_err(map_error)?;
                if live.snapshot().release_epoch == before
                    || live.recommended_load().is_some()
                    || live.lifecycle() == MissionSessionLifecycle::Completed
                {
                    break;
                }
            }
        }
        Command::Stage(x, m) => {
            let result = live
                .submit_operator_action(MissionOperatorAction::Stage {
                    load: x,
                    completed_event_mask: m,
                })
                .map_err(map_error)?;
            receipt = Some((result, 1));
        }
        Command::Commit(x) => {
            let result = live
                .submit_operator_action(MissionOperatorAction::Commit(x))
                .map_err(map_error)?;
            receipt = Some((result, 2));
        }
        Command::Cancel(x) => {
            let result = live
                .submit_operator_action(MissionOperatorAction::Cancel(x))
                .map_err(map_error)?;
            receipt = Some((result, 3));
        }
        Command::Abort(x) => live.abort(x).map_err(map_error)?,
        #[cfg(test)]
        Command::TestBarrier(reached, release) => {
            reached.send(()).map_err(|_| ResultCode::Internal)?;
            release.recv().map_err(|_| ResultCode::Internal)?;
        }
        #[cfg(any(test, feature = "panic-probe"))]
        Command::PanicProbe => panic!("contained viewer bridge panic probe"),
        Command::Shutdown => return Ok((false, None)),
    }
    Ok((true, receipt))
}
#[allow(clippy::too_many_arguments)]
fn publish_full(
    shared: &Arc<Mutex<Shared>>,
    session: &FullMissionSession,
    role: u32,
    sequence: u64,
    result: ResultCode,
    event_cursor: &mut u32,
    timeline_cursor: &mut u32,
    sample_cursor: &mut u32,
) -> Result<(), ResultCode> {
    let snapshot = session.snapshot();
    let events = session.events_after(*event_cursor);
    let timeline = session.timeline_after(*timeline_cursor);
    let samples = session.release_samples_after(*sample_cursor);
    let recommended = if let Some(load) = session.recommended_load() {
        let mut bytes = vec![0; KUL11_LENGTH];
        write_kul11(&load, &mut bytes).map_err(|_| ResultCode::Internal)?;
        Some(bytes)
    } else {
        None
    };
    let commit = if let Some(request) = session.commit_request_for_staged() {
        let mut bytes = vec![0; KUA11_LENGTH];
        write_kua11(&request, &mut bytes).map_err(|_| ResultCode::Internal)?;
        Some(bytes)
    } else {
        None
    };
    let action_proposal = full_action_proposal(&snapshot);
    let legacy = full_snapshot_legacy(&snapshot, role, sequence, result);
    let operational = full_operational(&snapshot, role, sequence.saturating_add(1));
    let procedure = full_procedure(&snapshot);
    let (prediction_header, prediction_points) = full_prediction(
        snapshot
            .latest_ground_prediction
            .as_ref()
            .or(snapshot.latest_onboard_prediction.as_ref()),
    );
    let mut state = shared.lock().map_err(|_| ResultCode::Internal)?;
    state.snapshot_publication = state.snapshot_publication.saturating_add(1);
    state.snapshot = Some(legacy);
    state.operational = Some(OperationalViewV1 {
        publication_sequence: state.snapshot_publication,
        ..operational
    });
    state.procedure = (procedure.validity_mask != 0).then_some(procedure);
    state.disposition = None;
    state.action_proposal = action_proposal;
    state.prediction_header = prediction_header;
    state.prediction_points = prediction_points;
    state.last_command_result = result as i32;
    for event in events {
        if event.kind != MissionSessionEventKind::Release {
            if state.events.len() == KSA64_VIEWER_EVENT_CAPACITY {
                state.event_overflow = true;
                state.diagnostic = format!(
                    "event queue overflow at session event {}; stream is incomplete",
                    event.sequence
                );
            } else if !state.event_overflow {
                state.events.push_back(bridge_event(event));
            }
        }
        *event_cursor = event.sequence;
    }
    for value in timeline {
        if state.timeline.len() == KSA64_VIEWER_TIMELINE_CAPACITY {
            state.timeline_overflow = true;
        } else if !state.timeline_overflow {
            state
                .timeline
                .push_back(full_timeline(value, timeline_cursor.saturating_add(1)));
        }
        *timeline_cursor = timeline_cursor.saturating_add(1);
    }
    for value in samples {
        if state.samples.len() == KSA64_VIEWER_SAMPLE_CAPACITY {
            state.samples.pop_front();
            state.sample_overflow = true;
        }
        state.samples.push_back(full_release_sample(*value));
        *sample_cursor = sample_cursor.saturating_add(1);
    }
    state.recommended = recommended;
    state.commit = commit;
    Ok(())
}

fn apply_full(
    command: Command,
    live: &mut FullMissionSession,
) -> Result<(bool, Option<(MissionActionReceipt, u32)>), ResultCode> {
    let mut receipt = None;
    match command {
        Command::Pause => live.pause().map_err(map_error)?,
        Command::Resume => live.resume().map_err(map_error)?,
        Command::Pace(value) => live.set_pace(value).map_err(map_error)?,
        Command::Step => {
            live.step_one_release().map_err(map_error)?;
        }
        Command::Advance(count) => {
            live.advance_bounded(count).map_err(map_error)?;
        }
        Command::Stage(load, mask) => {
            let result = live
                .submit_operator_action(MissionOperatorAction::Stage {
                    load,
                    completed_event_mask: mask,
                })
                .map_err(map_error)?;
            receipt = Some((result, 1));
        }
        Command::Commit(request) => {
            let result = live
                .submit_operator_action(MissionOperatorAction::Commit(request))
                .map_err(map_error)?;
            receipt = Some((result, 2));
        }
        Command::Cancel(request) => {
            let result = live
                .submit_operator_action(MissionOperatorAction::Cancel(request))
                .map_err(map_error)?;
            receipt = Some((result, 3));
        }
        Command::Abort(identity) => live.abort(identity).map_err(map_error)?,
        #[cfg(test)]
        Command::TestBarrier(reached, release) => {
            reached.send(()).map_err(|_| ResultCode::Internal)?;
            release.recv().map_err(|_| ResultCode::Internal)?;
        }
        #[cfg(any(test, feature = "panic-probe"))]
        Command::PanicProbe => panic!("contained viewer bridge panic probe"),
        Command::Shutdown => return Ok((false, None)),
    }
    Ok((true, receipt))
}

fn full_worker_main(
    role_identity: u32,
    initial_pace: MissionSessionPace,
    rx: std::sync::mpsc::Receiver<Command>,
    shared: Arc<Mutex<Shared>>,
    pending_commands: Arc<AtomicUsize>,
    initialized: SyncSender<Result<(), ResultCode>>,
) {
    if operational_role(role_identity).is_none() {
        let _ = initialized.send(Err(ResultCode::Unsupported));
        return;
    }
    let request = MissionRequest {
        id: "ksa-g10r.operations".into(),
        scenario: Some("gnss-loss-full".into()),
        role: Some(role_name(role_identity).expect("validated role").into()),
        display: MissionDisplay::None,
        pace: MissionPace::Realtime,
        scripted: false,
        output: None,
    };
    let mut live = match Ksa64Application::default().start_full_operations_mission(&request) {
        Ok(value) => value,
        Err(error) => {
            diag(
                &shared,
                format!("start full mission failed: {:?}", error.diagnostic),
            );
            let _ = initialized.send(Err(ResultCode::Unsupported));
            if let Ok(mut state) = shared.lock() {
                state.worker_done = true;
            }
            return;
        }
    };
    if initial_pace != MissionSessionPace::Realtime {
        if let Err(error) = live.set_pace(initial_pace) {
            let _ = initialized.send(Err(map_error(error)));
            if let Ok(mut state) = shared.lock() {
                state.worker_done = true;
            }
            return;
        }
    }
    let mut sequence = 0_u64;
    let mut event_cursor = 0_u32;
    let mut timeline_cursor = 0_u32;
    let mut sample_cursor = 0_u32;
    if let Err(error) = publish_full(
        &shared,
        &live,
        role_identity,
        sequence,
        ResultCode::Ok,
        &mut event_cursor,
        &mut timeline_cursor,
        &mut sample_cursor,
    ) {
        let _ = initialized.send(Err(error));
        if let Ok(mut state) = shared.lock() {
            state.worker_done = true;
        }
        return;
    }
    if initialized.send(Ok(())).is_err() {
        if let Ok(mut state) = shared.lock() {
            state.worker_done = true;
        }
        return;
    }
    while let Ok(command) = rx.recv() {
        pending_commands.fetch_sub(1, Ordering::AcqRel);
        sequence = sequence.saturating_add(1);
        let (keep_running, receipt, result) = match apply_full(command, &mut live) {
            Ok((keep_running, receipt)) => (keep_running, receipt, ResultCode::Ok),
            Err(error) => {
                diag(&shared, format!("command {sequence} failed: {error:?}"));
                (true, None, error)
            }
        };
        if let Some((receipt, operation)) = receipt {
            if let Ok(mut state) = shared.lock() {
                state.action_receipt_publication =
                    state.action_receipt_publication.saturating_add(1);
                let publication = state.action_receipt_publication;
                state.action_receipt = Some(bridge_receipt(receipt, operation, publication));
            }
        }
        if !keep_running {
            break;
        }
        let completed_disposition = (live.lifecycle() == MissionSessionLifecycle::Completed)
            .then(|| full_disposition(&live.snapshot()));
        if let Err(error) = publish_full(
            &shared,
            &live,
            role_identity,
            sequence,
            result,
            &mut event_cursor,
            &mut timeline_cursor,
            &mut sample_cursor,
        ) {
            if let Ok(mut state) = shared.lock() {
                state.worker_failed = true;
                state.worker_done = true;
                state.diagnostic = format!("full snapshot publication failed: {error:?}");
            }
            return;
        }
        if live.lifecycle() == MissionSessionLifecycle::Completed {
            match live.finish() {
                Ok(done) => match verify_complete_session(&done.session.bundle) {
                    Ok(scan) if scan.sealed && scan.completed => {
                        if let Ok(mut state) = shared.lock() {
                            if let Some(snapshot) = state.snapshot.as_mut() {
                                snapshot.evidence_identity =
                                    done.session.evidence.evidence_identity;
                                snapshot.validity_mask |= VALID_EVIDENCE;
                            }
                            if let Some(operational) = state.operational.as_mut() {
                                operational.validity_mask |=
                                    VIEW_VALID_EVIDENCE | VIEW_VALID_DISPOSITION;
                            }
                            state.disposition = completed_disposition;
                            state.bundle = Some(done.session.bundle);
                        }
                    }
                    Ok(_) => {
                        if let Ok(mut state) = shared.lock() {
                            state.worker_failed = true;
                            state.diagnostic =
                                "full evidence did not contain a sealed completed KSB11".into();
                        }
                    }
                    Err(error) => {
                        if let Ok(mut state) = shared.lock() {
                            state.worker_failed = true;
                            state.diagnostic =
                                format!("full evidence verification failed: {error:?}");
                        }
                    }
                },
                Err(error) => {
                    if let Ok(mut state) = shared.lock() {
                        state.worker_failed = true;
                        state.diagnostic = format!("full finalization failed: {error:?}");
                    }
                }
            }
            break;
        }
    }
    if let Ok(mut state) = shared.lock() {
        state.worker_done = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn worker_main(
    role: String,
    role_identity: u32,
    scenario_identity: u32,
    initial_pace: MissionSessionPace,
    rx: std::sync::mpsc::Receiver<Command>,
    shared: Arc<Mutex<Shared>>,
    pending_commands: Arc<AtomicUsize>,
    initialized: SyncSender<Result<(), ResultCode>>,
) {
    let req = MissionRequest {
        id: "ksa-g10r.operations".into(),
        // Phase 12B's accepted full-world runner is plugged into this adapter by
        // the host application. Until then, the compact Phase 11 fixture keeps
        // ABI development independently testable without changing its evidence.
        scenario: Some("gnss-loss".into()),
        role: Some(role),
        display: MissionDisplay::None,
        pace: MissionPace::Fast,
        scripted: false,
        output: None,
    };
    let mut live = match Ksa64Application::default().start_mission(&req) {
        Ok(x) => x,
        Err(e) => {
            diag(&shared, format!("start mission failed: {:?}", e.diagnostic));
            let _ = initialized.send(Err(ResultCode::Unsupported));
            if let Ok(mut state) = shared.lock() {
                state.worker_done = true;
            }
            return;
        }
    };
    if initial_pace != MissionSessionPace::Fast {
        if let Err(error) = live.set_pace(initial_pace) {
            diag(&shared, format!("initial pace failed: {error:?}"));
            let _ = initialized.send(Err(map_error(error)));
            if let Ok(mut state) = shared.lock() {
                state.worker_done = true;
            }
            return;
        }
    }
    let mut seq = 0u64;
    let mut cursor = 0u32;
    if let Err(error) = publish(
        &shared,
        &live,
        role_identity,
        scenario_identity,
        seq,
        ResultCode::Ok,
        &mut cursor,
    ) {
        diag(&shared, format!("initial publication failed: {error:?}"));
        let _ = initialized.send(Err(error));
        if let Ok(mut state) = shared.lock() {
            state.worker_done = true;
        }
        return;
    }
    if initialized.send(Ok(())).is_err() {
        if let Ok(mut state) = shared.lock() {
            state.worker_done = true;
        }
        return;
    }
    while let Ok(cmd) = rx.recv() {
        pending_commands.fetch_sub(1, Ordering::AcqRel);
        seq = seq.saturating_add(1);
        let (keep_running, receipt, result) = match apply(cmd, &mut live) {
            Ok((keep_running, receipt)) => (keep_running, receipt, ResultCode::Ok),
            Err(error) => {
                diag(&shared, format!("command {seq} failed: {error:?}"));
                (true, None, error)
            }
        };
        if let Some((receipt, operation)) = receipt {
            if let Ok(mut state) = shared.lock() {
                state.action_receipt_publication =
                    state.action_receipt_publication.saturating_add(1);
                let publication = state.action_receipt_publication;
                state.action_receipt = Some(bridge_receipt(receipt, operation, publication));
            }
        }
        if !keep_running {
            break;
        }
        if let Err(error) = publish(
            &shared,
            &live,
            role_identity,
            scenario_identity,
            seq,
            result,
            &mut cursor,
        ) {
            if let Ok(mut state) = shared.lock() {
                state.worker_failed = true;
                state.diagnostic = format!("snapshot publication failed: {error:?}");
                state.worker_done = true;
            }
            return;
        }
        if live.lifecycle() == MissionSessionLifecycle::Completed {
            match live.finish() {
                Ok(done) => {
                    if let Ok(mut state) = shared.lock() {
                        state.bundle = Some(done.bundle);
                    }
                }
                Err(e) => {
                    if let Ok(mut state) = shared.lock() {
                        state.worker_failed = true;
                        state.diagnostic = format!("finalization failed: {e:?}");
                    }
                }
            }
            break;
        }
    }
    if let Ok(mut state) = shared.lock() {
        state.worker_done = true;
    }
}
fn boundary(f: impl FnOnce() -> ResultCode) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => {
            if (result as i32) < 0 {
                set_library_diagnostic(format!("bridge call failed with result {}", result as i32));
            }
            result as i32
        }
        Err(_) => {
            set_library_diagnostic("bridge export panicked and was contained");
            ResultCode::Panic as i32
        }
    }
}
fn href(h: *const Handle) -> Result<Arc<HandleState>, ResultCode> {
    if h.is_null() {
        return Err(ResultCode::InvalidArgument);
    }
    handles()
        .lock()
        .map_err(|_| ResultCode::Internal)?
        .get(&(h as usize))
        .cloned()
        .ok_or(ResultCode::InvalidArgument)
}
fn enqueue(h: Arc<HandleState>, c: Command) -> ResultCode {
    if h.closed.load(Ordering::Acquire) {
        return ResultCode::Closed;
    }
    if h.shared.lock().map_or(true, |state| state.worker_failed) {
        return ResultCode::Panic;
    }
    h.pending_commands.fetch_add(1, Ordering::AcqRel);
    match h.commands.try_send(c) {
        Ok(()) => ResultCode::Queued,
        Err(TrySendError::Full(_)) => {
            h.pending_commands.fetch_sub(1, Ordering::AcqRel);
            ResultCode::QueueFull
        }
        Err(TrySendError::Disconnected(_)) => {
            h.pending_commands.fetch_sub(1, Ordering::AcqRel);
            ResultCode::Closed
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_get_abi_info(out: *mut AbiInfo) -> i32 {
    boundary(|| {
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let h = unsafe { &*out };
        if let Err(e) = validate(h.abi_version, h.struct_size, size_of::<AbiInfo>()) {
            return e;
        }
        unsafe {
            let catalog =
                serde_json::to_vec_pretty(&Ksa64Application::default().catalog().json(true))
                    .expect("accepted catalog JSON");
            *out = AbiInfo {
                abi_version: 1,
                struct_size: size_of::<AbiInfo>() as u32,
                build_identity: KSA64_VIEWER_BUILD_IDENTITY,
                release_hz: 32,
                command_capacity: KSA64_VIEWER_COMMAND_CAPACITY as u32,
                event_capacity: KSA64_VIEWER_EVENT_CAPACITY as u32,
                maximum_advance_releases: KSA64_VIEWER_MAX_ADVANCE_RELEASES,
                feature_flags: (if cfg!(feature = "panic-probe") {
                    KSA64_VIEWER_FEATURE_PANIC_PROBE
                } else {
                    0
                }) | KSA64_VIEWER_FEATURE_OPERATIONS_V1
                    | KSA64_VIEWER_FEATURE_TYPED_ACTIONS_V1
                    | KSA64_VIEWER_FEATURE_ASYNC_STATUS_V1,
                catalog_count: 13,
                snapshot_size: size_of::<Snapshot>() as u32,
                event_size: size_of::<Event>() as u32,
                span_size: size_of::<Span>() as u32,
                owned_buffer_size: size_of::<OwnedBuffer>() as u32,
                source_commit: fixed_bytes(env!("KSA64_SOURCE_COMMIT")),
                target_triple: fixed_bytes(env!("KSA64_TARGET_TRIPLE")),
                catalog_sha256: ksa64_host::phase11_session::sha256(&catalog),
            }
        }
        ResultCode::Ok
    })
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_catalog(out: *mut OwnedBuffer) -> i32 {
    boundary(|| {
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let h = unsafe { &*out };
        if let Err(e) = validate(h.abi_version, h.struct_size, size_of::<OwnedBuffer>()) {
            return e;
        }
        if !h.data.is_null() || h.length != 0 || h.allocation_id != 0 {
            return ResultCode::InvalidArgument;
        }
        let b = match serde_json::to_vec_pretty(&Ksa64Application::default().catalog().json(true)) {
            Ok(x) => x,
            Err(_) => return ResultCode::Internal,
        };
        match owned(b) {
            Ok(value) => unsafe { *out = value },
            Err(error) => return error,
        }
        ResultCode::Ok
    })
}
fn start_internal(
    role: &str,
    role_identity: u32,
    scenario_identity: u32,
    initial_pace: MissionSessionPace,
    out: *mut *mut Handle,
) -> ResultCode {
    if out.is_null() {
        return ResultCode::InvalidArgument;
    }
    unsafe { *out = ptr::null_mut() }
    if !matches!(
        scenario_identity,
        SCENARIO_LEGACY_GNSS_FIXTURE | SCENARIO_FULL_GNSS_LOSS
    ) {
        return ResultCode::Unsupported;
    }
    let (tx, rx) = sync_channel(KSA64_VIEWER_COMMAND_CAPACITY);
    let (initialized_tx, initialized_rx) = sync_channel(0);
    let shared = Arc::new(Mutex::new(Shared::default()));
    let worker_shared = Arc::clone(&shared);
    let failure_shared = Arc::clone(&shared);
    let pending_commands = Arc::new(AtomicUsize::new(0));
    let worker_pending = Arc::clone(&pending_commands);
    let role = role.to_owned();
    let worker = match thread::Builder::new()
        .name("ksa64-viewer-session".into())
        .spawn(move || {
            if catch_unwind(AssertUnwindSafe(|| {
                if scenario_identity == SCENARIO_FULL_GNSS_LOSS {
                    full_worker_main(
                        role_identity,
                        initial_pace,
                        rx,
                        worker_shared,
                        worker_pending,
                        initialized_tx,
                    )
                } else {
                    worker_main(
                        role,
                        role_identity,
                        scenario_identity,
                        initial_pace,
                        rx,
                        worker_shared,
                        worker_pending,
                        initialized_tx,
                    )
                }
            }))
            .is_err()
            {
                if let Ok(mut state) = failure_shared.lock() {
                    state.worker_failed = true;
                    state.worker_done = true;
                    state.diagnostic = "viewer worker panicked and was contained".to_owned();
                }
            }
        }) {
        Ok(x) => x,
        Err(_) => return ResultCode::Internal,
    };
    match initialized_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = worker.join();
            return error;
        }
        Err(_) => {
            let _ = worker.join();
            return ResultCode::Panic;
        }
    }
    let state = Arc::new(HandleState {
        commands: tx,
        shared,
        closed: AtomicBool::new(false),
        worker: Mutex::new(Some(worker)),
        last_snapshot_publication: AtomicU64::new(0),
        last_operational_publication: AtomicU64::new(0),
        last_action_receipt_publication: AtomicU64::new(0),
        pending_commands,
    });
    let mut registry = match handles().lock() {
        Ok(value) => value,
        Err(_) => return ResultCode::Internal,
    };
    let token = loop {
        let candidate = next_handle_token();
        if !registry.contains_key(&candidate) {
            break candidate;
        }
    };
    registry.insert(token, state);
    unsafe { *out = token as *mut Handle }
    ResultCode::Ok
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_start(role: *const Span, out: *mut *mut Handle) -> i32 {
    boundary(|| {
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        unsafe { *out = ptr::null_mut() }
        let bytes = match unsafe { copy_span(role) } {
            Ok(value) => value,
            Err(error) => return error,
        };
        let role = match str::from_utf8(&bytes) {
            Ok(value) => value,
            Err(_) => return ResultCode::InvalidUtf8,
        };
        let identity = match role_id(role) {
            Some(value) => value,
            None => return ResultCode::Unsupported,
        };
        start_internal(
            role,
            identity,
            SCENARIO_LEGACY_GNSS_FIXTURE,
            MissionSessionPace::Fast,
            out,
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_start_v1(
    request: *const StartRequestV1,
    out: *mut *mut Handle,
) -> i32 {
    boundary(|| {
        if request.is_null() || out.is_null() {
            return ResultCode::InvalidArgument;
        }
        unsafe { *out = ptr::null_mut() }
        let request = unsafe { &*request };
        if let Err(error) = validate(
            request.abi_version,
            request.struct_size,
            size_of::<StartRequestV1>(),
        ) {
            return error;
        }
        if request.flags & !START_FLAG_MASK != 0 || request.reserved.iter().any(|value| *value != 0)
        {
            return ResultCode::InvalidArgument;
        }
        let role = match role_name(request.role) {
            Some(value) => value,
            None => return ResultCode::Unsupported,
        };
        let pace = match request.initial_pace {
            1 => MissionSessionPace::Fast,
            2 => MissionSessionPace::Realtime,
            4 => MissionSessionPace::SingleStep,
            _ => return ResultCode::InvalidArgument,
        };
        start_internal(role, request.role, request.scenario_identity, pace, out)
    })
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_destroy(h: *mut Handle) -> i32 {
    boundary(|| {
        if h.is_null() {
            return ResultCode::InvalidArgument;
        }
        let h = match handles().lock() {
            Ok(mut registry) => match registry.remove(&(h as usize)) {
                Some(value) => value,
                None => return ResultCode::InvalidArgument,
            },
            Err(_) => return ResultCode::Internal,
        };
        h.closed.store(true, Ordering::Release);
        h.pending_commands.fetch_add(1, Ordering::AcqRel);
        if h.commands.send(Command::Shutdown).is_err() {
            h.pending_commands.fetch_sub(1, Ordering::AcqRel);
        }
        let worker = h.worker.lock().ok().and_then(|mut x| x.take());
        if let Some(x) = worker {
            if x.join().is_err() {
                return ResultCode::Panic;
            }
        }
        ResultCode::Ok
    })
}
macro_rules! simple_export {
    ($name:ident,$cmd:expr) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(h: *const Handle) -> i32 {
            boundary(|| match href(h) {
                Ok(h) => enqueue(h, $cmd),
                Err(e) => e,
            })
        }
    };
}
simple_export!(ksa64_viewer_pause, Command::Pause);
simple_export!(ksa64_viewer_resume, Command::Resume);
simple_export!(ksa64_viewer_step, Command::Step);
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_set_pace(h: *const Handle, p: u32) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(x) => x,
            Err(e) => return e,
        };
        let p = match p {
            1 => MissionSessionPace::Fast,
            2 => MissionSessionPace::Realtime,
            4 => MissionSessionPace::SingleStep,
            _ => return ResultCode::InvalidArgument,
        };
        enqueue(h, Command::Pace(p))
    })
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_advance(h: *const Handle, n: u32) -> i32 {
    boundary(|| {
        if n == 0 || n > KSA64_VIEWER_MAX_ADVANCE_RELEASES {
            return ResultCode::InvalidArgument;
        }
        match href(h) {
            Ok(h) => enqueue(h, Command::Advance(n)),
            Err(e) => e,
        }
    })
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_abort(h: *const Handle, id: u32) -> i32 {
    boundary(|| {
        if id == 0 {
            return ResultCode::InvalidArgument;
        }
        match href(h) {
            Ok(h) => enqueue(h, Command::Abort(id)),
            Err(e) => e,
        }
    })
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_poll_snapshot(h: *const Handle, out: *mut Snapshot) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(x) => x,
            Err(e) => return e,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(e) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<Snapshot>(),
        ) {
            return e;
        }
        let state = match h.shared.lock() {
            Ok(x) => x,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        match state.snapshot {
            Some(snapshot) => {
                let publication = state.snapshot_publication;
                if h.last_snapshot_publication.load(Ordering::Acquire) == publication {
                    return ResultCode::Unchanged;
                }
                unsafe { *out = snapshot }
                h.last_snapshot_publication
                    .store(publication, Ordering::Release);
                ResultCode::Ok
            }
            None => ResultCode::NoData,
        }
    })
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_poll_event(h: *const Handle, out: *mut Event) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(x) => x,
            Err(e) => return e,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(e) = validate(header.abi_version, header.struct_size, size_of::<Event>()) {
            return e;
        }
        let mut state = match h.shared.lock() {
            Ok(x) => x,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        if state.event_overflow {
            return ResultCode::EventOverflow;
        }
        match state.events.pop_front() {
            Some(x) => {
                unsafe { *out = x }
                ResultCode::Ok
            }
            None => ResultCode::NoData,
        }
    })
}

macro_rules! latest_view_export {
    ($name:ident, $ty:ty, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(h: *const Handle, out: *mut $ty) -> i32 {
            boundary(|| {
                let h = match href(h) {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                if out.is_null() {
                    return ResultCode::InvalidArgument;
                }
                let header = unsafe { &*out };
                if let Err(error) =
                    validate(header.abi_version, header.struct_size, size_of::<$ty>())
                {
                    return error;
                }
                let state = match h.shared.lock() {
                    Ok(value) => value,
                    Err(_) => return ResultCode::Internal,
                };
                if state.worker_failed {
                    return ResultCode::Panic;
                }
                match state.$field {
                    Some(value) => {
                        unsafe { *out = value }
                        ResultCode::Ok
                    }
                    None => ResultCode::NoData,
                }
            })
        }
    };
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_poll_operational_v1(
    h: *const Handle,
    out: *mut OperationalViewV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<OperationalViewV1>(),
        ) {
            return error;
        }
        let state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        let value = match state.operational {
            Some(value) => value,
            None => return ResultCode::NoData,
        };
        if h.last_operational_publication.load(Ordering::Acquire) == value.publication_sequence {
            return ResultCode::Unchanged;
        }
        unsafe { *out = value }
        h.last_operational_publication
            .store(value.publication_sequence, Ordering::Release);
        ResultCode::Ok
    })
}

latest_view_export!(ksa64_viewer_procedure_v1, ProcedureViewV1, procedure);
latest_view_export!(ksa64_viewer_disposition_v1, DispositionV1, disposition);

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_poll_timeline_v1(
    h: *const Handle,
    out: *mut TimelineEventV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<TimelineEventV1>(),
        ) {
            return error;
        }
        let mut state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        if state.timeline_overflow {
            return ResultCode::EventOverflow;
        }
        match state.timeline.pop_front() {
            Some(value) => {
                unsafe { *out = value }
                ResultCode::Ok
            }
            None => ResultCode::NoData,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_poll_release_sample_v1(
    h: *const Handle,
    out: *mut ReleaseSampleV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<ReleaseSampleV1>(),
        ) {
            return error;
        }
        let mut state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        match state.samples.pop_front() {
            Some(value) => {
                unsafe { *out = value }
                ResultCode::Ok
            }
            None => ResultCode::NoData,
        }
    })
}

latest_view_export!(
    ksa64_viewer_prediction_path_header_v1,
    PredictionPathHeaderV1,
    prediction_header
);

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_prediction_path_point_v1(
    h: *const Handle,
    index: u32,
    out: *mut PredictionPathPointV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<PredictionPathPointV1>(),
        ) {
            return error;
        }
        let state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        match state.prediction_points.get(index as usize).copied() {
            Some(value) => {
                unsafe { *out = value }
                ResultCode::Ok
            }
            None => ResultCode::NoData,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_action_proposal_v1(
    h: *const Handle,
    out: *mut ActionProposalV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<ActionProposalV1>(),
        ) {
            return error;
        }
        let state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        match state.action_proposal {
            Some(value) => {
                unsafe { *out = value }
                ResultCode::Ok
            }
            None => ResultCode::NoData,
        }
    })
}

fn typed_proposal_load(
    h: *const Handle,
    proposal_identity: u32,
) -> Result<(Arc<HandleState>, UplinkCommandLoad), ResultCode> {
    if proposal_identity == 0 {
        return Err(ResultCode::InvalidArgument);
    }
    let handle = href(h)?;
    let bytes = handle
        .shared
        .lock()
        .map_err(|_| ResultCode::Internal)?
        .recommended
        .clone()
        .ok_or(ResultCode::ActionUnavailable)?;
    let load = parse_kul11(&bytes).map_err(|_| ResultCode::Internal)?;
    if load.load_identity != proposal_identity {
        return Err(ResultCode::ActionRejected);
    }
    Ok((handle, load))
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_submit_action_proposal_v1(
    h: *const Handle,
    proposal_identity: u32,
    completed_event_mask: u32,
) -> i32 {
    boundary(|| match typed_proposal_load(h, proposal_identity) {
        Ok((handle, load)) => enqueue(handle, Command::Stage(load, completed_event_mask)),
        Err(error) => error,
    })
}

fn typed_control(h: *const Handle, proposal_identity: u32, cancel: bool) -> ResultCode {
    if proposal_identity == 0 {
        return ResultCode::InvalidArgument;
    }
    let handle = match href(h) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let bytes = match handle.shared.lock() {
        Ok(state) => match state.commit.clone() {
            Some(value) => value,
            None => return ResultCode::ActionUnavailable,
        },
        Err(_) => return ResultCode::Internal,
    };
    let mut record = match parse_kua11(&bytes) {
        Ok(value) => value,
        Err(_) => return ResultCode::Internal,
    };
    if record.load_identity != proposal_identity {
        return ResultCode::ActionRejected;
    }
    if cancel {
        record.kind = UplinkControlKind::Cancellation;
    }
    enqueue(
        handle,
        if cancel {
            Command::Cancel(record)
        } else {
            Command::Commit(record)
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_commit_action_v1(
    h: *const Handle,
    proposal_identity: u32,
) -> i32 {
    boundary(|| typed_control(h, proposal_identity, false))
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_cancel_action_v1(
    h: *const Handle,
    proposal_identity: u32,
) -> i32 {
    boundary(|| typed_control(h, proposal_identity, true))
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_poll_action_receipt_v1(
    h: *const Handle,
    out: *mut ActionReceiptV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<ActionReceiptV1>(),
        ) {
            return error;
        }
        let state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.worker_failed {
            return ResultCode::Panic;
        }
        let value = match state.action_receipt {
            Some(value) => value,
            None => return ResultCode::NoData,
        };
        if h.last_action_receipt_publication.load(Ordering::Acquire) == value.publication_sequence {
            return ResultCode::Unchanged;
        }
        unsafe { *out = value }
        h.last_action_receipt_publication
            .store(value.publication_sequence, Ordering::Release);
        ResultCode::Ok
    })
}

fn crc32_bytes(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_transport_status_v1(
    h: *const Handle,
    out: *mut TransportStatusV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<TransportStatusV1>(),
        ) {
            return error;
        }
        let state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        unsafe {
            *out = TransportStatusV1 {
                validity_mask: u64::MAX,
                commands_pending: h.pending_commands.load(Ordering::Acquire) as u32,
                events_pending: state.events.len() as u32,
                timeline_pending: state.timeline.len() as u32,
                samples_pending: state.samples.len() as u32,
                worker_state: if state.worker_failed {
                    3
                } else if state.worker_done {
                    2
                } else {
                    1
                },
                shutdown_requested: u32::from(state.shutdown_requested),
                finalization_state: if state.bundle.is_some() {
                    2
                } else if state.worker_done {
                    3
                } else {
                    1
                },
                event_overflow: u32::from(state.event_overflow),
                timeline_overflow: u32::from(state.timeline_overflow),
                sample_overflow: u32::from(state.sample_overflow),
                last_command_result: state.last_command_result,
                ..TransportStatusV1::default()
            }
        }
        ResultCode::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_finish_status_v1(
    h: *const Handle,
    out: *mut FinishStatusV1,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<FinishStatusV1>(),
        ) {
            return error;
        }
        let state = match h.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        let snapshot = state.snapshot.unwrap_or_default();
        let mut value = FinishStatusV1 {
            lifecycle: snapshot.lifecycle,
            finalization_state: if state.bundle.is_some() {
                2
            } else if state.worker_done {
                3
            } else {
                1
            },
            shutdown_state: if state.worker_done {
                2
            } else if state.shutdown_requested {
                1
            } else {
                0
            },
            evidence_identity: snapshot.evidence_identity,
            ..FinishStatusV1::default()
        };
        if let Some(bundle) = state.bundle.as_deref() {
            value.validity_mask = VIEW_VALID_EVIDENCE;
            value.evidence_length = bundle.len() as u64;
            value.evidence_crc32 = crc32_bytes(bundle);
        }
        unsafe { *out = value }
        ResultCode::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_request_shutdown_v1(h: *const Handle) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(value) => value,
            Err(error) => return error,
        };
        {
            let mut state = match h.shared.lock() {
                Ok(value) => value,
                Err(_) => return ResultCode::Internal,
            };
            if state.worker_done {
                state.shutdown_requested = true;
                return ResultCode::Ok;
            }
            state.shutdown_requested = true;
        }
        enqueue(h, Command::Shutdown)
    })
}

fn copy_buffer(
    h: *const Handle,
    out: *mut OwnedBuffer,
    f: impl FnOnce(&Shared) -> Option<Vec<u8>>,
    allow_failed: bool,
) -> ResultCode {
    let h = match href(h) {
        Ok(x) => x,
        Err(e) => return e,
    };
    if out.is_null() {
        return ResultCode::InvalidArgument;
    }
    let header = unsafe { &*out };
    if let Err(e) = validate(
        header.abi_version,
        header.struct_size,
        size_of::<OwnedBuffer>(),
    ) {
        return e;
    }
    if !header.data.is_null() || header.length != 0 || header.allocation_id != 0 {
        return ResultCode::InvalidArgument;
    }
    let state = match h.shared.lock() {
        Ok(x) => x,
        Err(_) => return ResultCode::Internal,
    };
    if state.worker_failed && !allow_failed {
        return ResultCode::Panic;
    }
    match f(&state) {
        Some(x) => {
            match owned(x) {
                Ok(value) => unsafe { *out = value },
                Err(error) => return error,
            }
            ResultCode::Ok
        }
        None => ResultCode::NoData,
    }
}
macro_rules! buffer_export {
    ($name:ident,$field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(h: *const Handle, out: *mut OwnedBuffer) -> i32 {
            boundary(|| copy_buffer(h, out, |s| s.$field.clone(), false))
        }
    };
}
buffer_export!(ksa64_viewer_recommended_load, recommended);
buffer_export!(ksa64_viewer_commit_request, commit);
buffer_export!(ksa64_viewer_completed_ksb11, bundle);
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_library_diagnostic(out: *mut OwnedBuffer) -> i32 {
    boundary(|| {
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*out };
        if let Err(error) = validate(
            header.abi_version,
            header.struct_size,
            size_of::<OwnedBuffer>(),
        ) {
            return error;
        }
        if !header.data.is_null() || header.length != 0 || header.allocation_id != 0 {
            return ResultCode::InvalidArgument;
        }
        let message = match library_diagnostic().lock() {
            Ok(diagnostic) if !diagnostic.is_empty() => diagnostic.as_bytes().to_vec(),
            Ok(_) => return ResultCode::NoData,
            Err(_) => return ResultCode::Internal,
        };
        match owned(message) {
            Ok(buffer) => unsafe { *out = buffer },
            Err(error) => return error,
        }
        ResultCode::Ok
    })
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_diagnostic(h: *const Handle, out: *mut OwnedBuffer) -> i32 {
    boundary(|| {
        copy_buffer(
            h,
            out,
            |s| (!s.diagnostic.is_empty()).then(|| s.diagnostic.as_bytes().to_vec()),
            true,
        )
    })
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_submit_stage(
    h: *const Handle,
    p: *const Span,
    mask: u32,
) -> i32 {
    boundary(|| {
        let h = match href(h) {
            Ok(x) => x,
            Err(e) => return e,
        };
        let b = match unsafe { copy_span(p) } {
            Ok(x) => x,
            Err(e) => return e,
        };
        match parse_kul11(&b) {
            Ok(x) => enqueue(h, Command::Stage(x, mask)),
            Err(_) => ResultCode::InvalidArgument,
        }
    })
}
fn submit_control(h: *const Handle, p: *const Span, commit: bool) -> ResultCode {
    let h = match href(h) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let b = match unsafe { copy_span(p) } {
        Ok(x) => x,
        Err(e) => return e,
    };
    match parse_kua11(&b) {
        Ok(x) => enqueue(
            h,
            if commit {
                Command::Commit(x)
            } else {
                Command::Cancel(x)
            },
        ),
        Err(_) => ResultCode::InvalidArgument,
    }
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_submit_commit(h: *const Handle, p: *const Span) -> i32 {
    boundary(|| submit_control(h, p, true))
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_submit_cancel(h: *const Handle, p: *const Span) -> i32 {
    boundary(|| submit_control(h, p, false))
}
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_free_buffer(b: *mut OwnedBuffer) -> i32 {
    boundary(|| {
        if b.is_null() {
            return ResultCode::InvalidArgument;
        }
        let b = unsafe { &mut *b };
        if let Err(e) = validate(b.abi_version, b.struct_size, size_of::<OwnedBuffer>()) {
            return e;
        }
        let n = match usize::try_from(b.length) {
            Ok(x) => x,
            Err(_) => return ResultCode::InvalidArgument,
        };
        if n == 0 || b.data.is_null() {
            return ResultCode::InvalidArgument;
        }
        let registered = match buffers().lock() {
            Ok(mut registry) => match registry.get(&b.allocation_id) {
                Some((pointer, length)) if *pointer == b.data as usize && *length == n => {
                    registry.remove(&b.allocation_id)
                }
                _ => None,
            },
            Err(_) => return ResultCode::Internal,
        };
        if registered.is_none() {
            return ResultCode::InvalidArgument;
        }
        unsafe { drop(Box::from_raw(ptr::slice_from_raw_parts_mut(b.data, n))) }
        b.data = ptr::null_mut();
        b.length = 0;
        b.allocation_id = 0;
        ResultCode::Ok
    })
}
#[cfg(any(test, feature = "panic-probe"))]
simple_export!(ksa64_viewer_test_panic_probe, Command::PanicProbe);

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // The full Phase 12B fixture uses the accepted process-global reference
    // pack cache. Serialize the two FFI tests that construct that fixture so
    // ordinary parallel cargo test execution cannot race initialization.
    static FULL_SESSION_TEST_LOCK: Mutex<()> = Mutex::new(());
    fn buf() -> OwnedBuffer {
        OwnedBuffer {
            abi_version: 1,
            struct_size: size_of::<OwnedBuffer>() as u32,
            data: ptr::null_mut(),
            length: 0,
            allocation_id: 0,
        }
    }
    unsafe fn take(mut b: OwnedBuffer) -> Vec<u8> {
        let v = if b.length == 0 {
            vec![]
        } else {
            unsafe { slice::from_raw_parts(b.data, b.length as usize) }.to_vec()
        };
        assert_eq!(unsafe { ksa64_viewer_free_buffer(&mut b) }, 0);
        v
    }
    unsafe fn start() -> *mut Handle {
        let s = "guided-operator";
        let span = Span {
            abi_version: 1,
            struct_size: size_of::<Span>() as u32,
            data: s.as_ptr(),
            length: s.len() as u64,
        };
        let mut h = ptr::null_mut();
        assert_eq!(unsafe { ksa64_viewer_start(&span, &mut h) }, 0);
        h
    }
    unsafe fn snap(h: *mut Handle) -> Snapshot {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let mut x = Snapshot::default();
            match unsafe { ksa64_viewer_poll_snapshot(h, &mut x) } {
                0 => return x,
                2 | 3 if Instant::now() < end => thread::yield_now(),
                n => panic!("poll {n}"),
            }
        }
    }
    unsafe fn wait(h: *mut Handle, seq: u64) -> Snapshot {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let x = unsafe { snap(h) };
            if x.command_sequence > seq {
                return x;
            }
            assert!(Instant::now() < end);
            thread::yield_now()
        }
    }
    unsafe fn complete_with_recommended_actions(h: *mut Handle, mut x: Snapshot) -> Vec<u8> {
        while x.lifecycle != 5 {
            if let Some(load) = unsafe { optional(ksa64_viewer_recommended_load, h) } {
                let s = Span {
                    abi_version: 1,
                    struct_size: size_of::<Span>() as u32,
                    data: load.as_ptr(),
                    length: load.len() as u64,
                };
                assert_eq!(unsafe { ksa64_viewer_submit_stage(h, &s, 0) }, 1);
                x = unsafe { wait(h, x.command_sequence) };
                let commit =
                    unsafe { optional(ksa64_viewer_commit_request, h) }.expect("commit request");
                let s = Span {
                    abi_version: 1,
                    struct_size: size_of::<Span>() as u32,
                    data: commit.as_ptr(),
                    length: commit.len() as u64,
                };
                assert_eq!(unsafe { ksa64_viewer_submit_commit(h, &s) }, 1);
                x = unsafe { wait(h, x.command_sequence) };
            }
            assert_eq!(unsafe { ksa64_viewer_advance(h, 32) }, 1);
            x = unsafe { wait(h, x.command_sequence) };
            assert_eq!(x.command_result, 0);
        }
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(bundle) = unsafe { optional(ksa64_viewer_completed_ksb11, h) } {
                break bundle;
            }
            assert!(Instant::now() < end);
            thread::yield_now();
        }
    }
    unsafe fn optional(
        f: unsafe extern "C" fn(*const Handle, *mut OwnedBuffer) -> i32,
        h: *mut Handle,
    ) -> Option<Vec<u8>> {
        let mut b = buf();
        match unsafe { f(h, &mut b) } {
            0 => Some(unsafe { take(b) }),
            2 => None,
            n => panic!("buffer {n}"),
        }
    }
    #[test]
    fn abi_and_catalog() {
        unsafe {
            assert_eq!(
                ksa64_viewer_get_abi_info(ptr::null_mut()),
                ResultCode::InvalidArgument as i32
            );
            let mut library = buf();
            assert_eq!(
                ksa64_viewer_library_diagnostic(&mut library),
                ResultCode::Ok as i32
            );
            let library = take(library);
            assert!(str::from_utf8(&library)
                .unwrap()
                .contains("bridge call failed"));
            let mut i = AbiInfo {
                abi_version: 1,
                struct_size: size_of::<AbiInfo>() as u32,
                build_identity: 0,
                release_hz: 0,
                command_capacity: 0,
                event_capacity: 0,
                maximum_advance_releases: 0,
                feature_flags: 0,
                catalog_count: 0,
                snapshot_size: 0,
                event_size: 0,
                span_size: 0,
                owned_buffer_size: 0,
                source_commit: [0; 16],
                target_triple: [0; 32],
                catalog_sha256: [0; 32],
            };
            assert_eq!(ksa64_viewer_get_abi_info(&mut i), 0);
            assert_eq!(i.snapshot_size as usize, size_of::<Snapshot>());
            let mut a = buf();
            let mut b = buf();
            assert_eq!(ksa64_viewer_catalog(&mut a), 0);
            assert_eq!(ksa64_viewer_catalog(&mut b), 0);
            let a = take(a);
            assert_eq!(a, take(b));
            assert_eq!(
                a.as_slice(),
                include_bytes!("../../phase11_5/product-catalog-v1.json")
            );
            let j: serde_json::Value = serde_json::from_slice(&a).unwrap();
            assert_eq!(j["experiences"].as_array().unwrap().len(), 13)
        }
    }
    #[test]
    fn rejects_before_enqueue() {
        unsafe {
            let invalid_role = "unknown-role";
            let invalid_role = Span {
                abi_version: 1,
                struct_size: size_of::<Span>() as u32,
                data: invalid_role.as_ptr(),
                length: invalid_role.len() as u64,
            };
            let mut rejected_handle = ptr::dangling_mut::<Handle>();
            assert_eq!(
                ksa64_viewer_start(&invalid_role, &mut rejected_handle),
                ResultCode::Unsupported as i32
            );
            assert!(rejected_handle.is_null());

            let h = start();
            let x = snap(h);
            let oversized = Span {
                abi_version: KSA64_VIEWER_ABI_VERSION,
                struct_size: size_of::<Span>() as u32,
                data: ptr::dangling(),
                length: (KSA64_VIEWER_MAX_CALLER_SPAN as u64) + 1,
            };
            assert_eq!(
                ksa64_viewer_submit_stage(h, &oversized, 0),
                ResultCode::InvalidArgument as i32
            );
            let bad = [0u8; KUL11_LENGTH];
            let s = Span {
                abi_version: 1,
                struct_size: size_of::<Span>() as u32,
                data: bad.as_ptr(),
                length: bad.len() as u64,
            };
            assert_eq!(
                ksa64_viewer_submit_stage(h, &s, 0),
                ResultCode::InvalidArgument as i32
            );
            let mut unchanged = Snapshot::default();
            assert_eq!(
                ksa64_viewer_poll_snapshot(h, &mut unchanged),
                ResultCode::Unchanged as i32
            );
            assert_eq!(unchanged.command_sequence, 0);
            assert_eq!(x.command_sequence, 0);
            let mut forged_output = Snapshot::default();
            assert_eq!(
                ksa64_viewer_poll_snapshot(ptr::dangling::<Handle>(), &mut forged_output),
                ResultCode::InvalidArgument as i32
            );
            let mut catalog = buf();
            assert_eq!(ksa64_viewer_catalog(&mut catalog), 0);
            let duplicate = catalog;
            assert_eq!(ksa64_viewer_free_buffer(&mut catalog), 0);
            let mut duplicate = duplicate;
            assert_eq!(
                ksa64_viewer_free_buffer(&mut duplicate),
                ResultCode::InvalidArgument as i32
            );
            assert_eq!(ksa64_viewer_destroy(h), 0);
            assert_eq!(ksa64_viewer_destroy(h), ResultCode::InvalidArgument as i32)
        }
    }
    #[test]
    fn lifecycle_and_panic_containment() {
        unsafe {
            let h = start();
            let a = snap(h);
            assert_eq!(ksa64_viewer_pause(h), 1);
            let b = wait(h, a.command_sequence);
            assert_eq!(b.lifecycle, 4);
            assert_eq!(ksa64_viewer_step(h), 1);
            let c = wait(h, b.command_sequence);
            assert_eq!(c.release_epoch, 1);
            assert_eq!(ksa64_viewer_test_panic_probe(h), 1);
            let end = Instant::now() + Duration::from_secs(5);
            loop {
                let mut failed = Snapshot::default();
                let result = ksa64_viewer_poll_snapshot(h, &mut failed);
                if result == ResultCode::Panic as i32 {
                    break;
                }
                assert!(matches!(result, 0 | 2 | 3) && Instant::now() < end);
                thread::yield_now();
            }
            assert_eq!(ksa64_viewer_resume(h), ResultCode::Panic as i32);
            let diagnostic = optional(ksa64_viewer_diagnostic, h).expect("panic diagnostic");
            assert!(str::from_utf8(&diagnostic)
                .unwrap()
                .contains("worker panicked"));
            assert_eq!(ksa64_viewer_destroy(h), 0)
        }
    }
    #[test]
    fn ffi_ksb11_matches_direct() {
        unsafe {
            let direct = Ksa64Application::default()
                .start_mission(&MissionRequest {
                    id: "ksa-g10r.operations".into(),
                    scenario: Some("gnss-loss".into()),
                    role: Some("guided-operator".into()),
                    display: MissionDisplay::None,
                    pace: MissionPace::Fast,
                    scripted: false,
                    output: None,
                })
                .unwrap()
                .run_scripted_to_completion()
                .unwrap()
                .bundle;
            let h = start();
            let initial = snap(h);
            let got = complete_with_recommended_actions(h, initial);
            assert_eq!(got, direct);
            assert_eq!(ksa64_viewer_destroy(h), 0)
        }
    }
    #[test]
    fn command_queue_saturation_is_fail_closed_and_preserves_ksb11() {
        unsafe {
            let direct = Ksa64Application::default()
                .start_mission(&MissionRequest {
                    id: "ksa-g10r.operations".into(),
                    scenario: Some("gnss-loss".into()),
                    role: Some("guided-operator".into()),
                    display: MissionDisplay::None,
                    pace: MissionPace::Fast,
                    scripted: false,
                    output: None,
                })
                .unwrap()
                .run_scripted_to_completion()
                .unwrap()
                .bundle;
            let h = start();
            let initial = snap(h);
            let state = href(h).expect("registered handle");
            let (reached_tx, reached_rx) = sync_channel(0);
            let (release_tx, release_rx) = sync_channel(0);
            assert_eq!(
                enqueue(
                    Arc::clone(&state),
                    Command::TestBarrier(reached_tx, release_rx)
                ),
                ResultCode::Queued
            );
            reached_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("worker reached test barrier");
            for _ in 0..KSA64_VIEWER_COMMAND_CAPACITY {
                assert_eq!(
                    enqueue(Arc::clone(&state), Command::Pace(MissionSessionPace::Fast)),
                    ResultCode::Queued
                );
            }
            assert_eq!(
                enqueue(Arc::clone(&state), Command::Pace(MissionSessionPace::Fast)),
                ResultCode::QueueFull
            );
            release_tx.send(()).expect("release test barrier");
            let drained = wait(
                h,
                initial.command_sequence + KSA64_VIEWER_COMMAND_CAPACITY as u64,
            );
            assert_eq!(
                drained.command_sequence,
                initial.command_sequence + KSA64_VIEWER_COMMAND_CAPACITY as u64 + 1
            );
            let got = complete_with_recommended_actions(h, drained);
            assert_eq!(got, direct);
            assert_eq!(ksa64_viewer_destroy(h), ResultCode::Ok as i32);
        }
    }
    #[test]
    fn event_overflow_is_sticky_and_fail_closed() {
        unsafe {
            let h = start();
            let mut snapshot = snap(h);
            for _ in 0..130 {
                assert_eq!(ksa64_viewer_pause(h), ResultCode::Queued as i32);
                snapshot = wait(h, snapshot.command_sequence);
                assert_eq!(ksa64_viewer_resume(h), ResultCode::Queued as i32);
                snapshot = wait(h, snapshot.command_sequence);
            }
            let mut event = Event {
                abi_version: KSA64_VIEWER_ABI_VERSION,
                struct_size: size_of::<Event>() as u32,
                ..Event::default()
            };
            assert_eq!(
                ksa64_viewer_poll_event(h, &mut event),
                ResultCode::EventOverflow as i32
            );
            assert_eq!(
                ksa64_viewer_poll_event(h, &mut event),
                ResultCode::EventOverflow as i32
            );
            let diagnostic = optional(ksa64_viewer_diagnostic, h).expect("overflow diagnostic");
            assert!(str::from_utf8(&diagnostic)
                .unwrap()
                .contains("event queue overflow"));
            assert_eq!(ksa64_viewer_destroy(h), ResultCode::Ok as i32);
        }
    }
    #[test]
    fn phase12b_start_and_role_filtered_views_validate_headers() {
        let _full_session_guard = FULL_SESSION_TEST_LOCK
            .lock()
            .expect("full session test lock");
        unsafe {
            let mut output = ptr::null_mut();
            let mut bad = StartRequestV1 {
                abi_version: 99,
                ..StartRequestV1::default()
            };
            assert_eq!(
                ksa64_viewer_start_v1(&bad, &mut output),
                ResultCode::AbiMismatch as i32
            );
            assert!(output.is_null());
            bad = StartRequestV1::default();
            bad.struct_size -= 1;
            assert_eq!(
                ksa64_viewer_start_v1(&bad, &mut output),
                ResultCode::StructSize as i32
            );
            bad = StartRequestV1::default();
            bad.reserved[0] = 1;
            assert_eq!(
                ksa64_viewer_start_v1(&bad, &mut output),
                ResultCode::InvalidArgument as i32
            );
            bad = StartRequestV1::default();
            bad.role = 99;
            assert_eq!(
                ksa64_viewer_start_v1(&bad, &mut output),
                ResultCode::Unsupported as i32
            );

            let request = StartRequestV1::default();
            assert_eq!(ksa64_viewer_start_v1(&request, &mut output), 0);
            let mut view = OperationalViewV1::default();
            assert_eq!(ksa64_viewer_poll_operational_v1(output, &mut view), 0);
            assert_eq!(view.scenario_identity, SCENARIO_FULL_GNSS_LOSS);
            assert_eq!(view.role, 2);
            assert_eq!(view.pace, 2);
            assert_eq!(view.validity_mask & VIEW_VALID_GROUND_ESTIMATE, 0);
            assert_eq!(
                ksa64_viewer_poll_operational_v1(output, &mut view),
                ResultCode::Unchanged as i32
            );
            let mut procedure = ProcedureViewV1::default();
            assert_eq!(
                ksa64_viewer_procedure_v1(output, &mut procedure),
                ResultCode::NoData as i32
            );
            let mut status = TransportStatusV1::default();
            assert_eq!(ksa64_viewer_transport_status_v1(output, &mut status), 0);
            assert_eq!(status.worker_state, 1);

            let initial = snap(output);
            assert_eq!(ksa64_viewer_advance(output, 1), ResultCode::Queued as i32);
            let advanced = wait(output, initial.command_sequence);
            assert_eq!(advanced.release_epoch, 1);
            view = OperationalViewV1::default();
            assert_eq!(ksa64_viewer_poll_operational_v1(output, &mut view), 0);
            assert_eq!(view.release_epoch, 1);
            assert_eq!(view.role, 2);
            let mut sample = ReleaseSampleV1::default();
            assert_eq!(ksa64_viewer_poll_release_sample_v1(output, &mut sample), 0);
            assert_eq!(
                sample.flags & 1,
                0,
                "guided-operator sample exposed SIM truth"
            );
            assert_eq!(ksa64_viewer_destroy(output), 0);
        }
    }

    #[test]
    fn typed_review_stage_commit_and_receipt_use_the_existing_uplink_boundary() {
        unsafe {
            let request = StartRequestV1 {
                scenario_identity: SCENARIO_LEGACY_GNSS_FIXTURE,
                initial_pace: 1,
                ..StartRequestV1::default()
            };
            let mut handle = ptr::null_mut();
            assert_eq!(ksa64_viewer_start_v1(&request, &mut handle), 0);
            let initial = snap(handle);
            assert_eq!(ksa64_viewer_advance(handle, 1), ResultCode::Queued as i32);
            let advanced = wait(handle, initial.command_sequence);
            assert_eq!(advanced.release_epoch, 1);

            let mut proposal = ActionProposalV1::default();
            assert_eq!(ksa64_viewer_action_proposal_v1(handle, &mut proposal), 0);
            assert_eq!(proposal.proposal_identity, proposal.load_identity);
            assert_eq!(proposal.permitted_operations, 1);
            assert_eq!(
                ksa64_viewer_submit_action_proposal_v1(handle, proposal.proposal_identity ^ 1, 0),
                ResultCode::ActionRejected as i32
            );
            assert_eq!(
                ksa64_viewer_submit_action_proposal_v1(handle, proposal.proposal_identity, 0),
                ResultCode::Queued as i32
            );
            let staged = wait(handle, advanced.command_sequence);
            assert_eq!(staged.command_result, 0);
            let mut receipt = ActionReceiptV1::default();
            assert_eq!(
                ksa64_viewer_poll_action_receipt_v1(handle, &mut receipt),
                ResultCode::Ok as i32
            );
            assert_eq!(receipt.operation, 1);
            assert_eq!(
                receipt.state,
                ksa64_interface::phase11::UplinkState::Staged as u32
            );
            assert_eq!(
                receipt.reason,
                ksa64_interface::phase11::UplinkReasonCode::Accepted as u32
            );
            assert_eq!(receipt.accepted, 1);

            assert_eq!(
                ksa64_viewer_commit_action_v1(handle, proposal.proposal_identity),
                ResultCode::Queued as i32
            );
            let committed = wait(handle, staged.command_sequence);
            assert_eq!(committed.command_result, 0);
            receipt = ActionReceiptV1::default();
            assert_eq!(ksa64_viewer_poll_action_receipt_v1(handle, &mut receipt), 0);
            assert_eq!(receipt.operation, 2);
            assert_eq!(
                receipt.state,
                ksa64_interface::phase11::UplinkState::Committed as u32
            );

            let mut timeline = TimelineEventV1::default();
            assert_eq!(ksa64_viewer_poll_timeline_v1(handle, &mut timeline), 0);
            assert!(timeline.label_length > 0);
            let mut sample = ReleaseSampleV1::default();
            assert_eq!(ksa64_viewer_poll_release_sample_v1(handle, &mut sample), 0);
            assert_eq!(sample.release_epoch, 0);
            assert_eq!(ksa64_viewer_destroy(handle), 0);
        }
    }

    #[test]
    fn async_shutdown_is_observable_before_destroy() {
        let _full_session_guard = FULL_SESSION_TEST_LOCK
            .lock()
            .expect("full session test lock");
        unsafe {
            let mut handle = ptr::null_mut();
            let request = StartRequestV1::default();
            assert_eq!(ksa64_viewer_start_v1(&request, &mut handle), 0);
            assert_eq!(
                ksa64_viewer_request_shutdown_v1(handle),
                ResultCode::Queued as i32
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let mut status = TransportStatusV1::default();
                assert_eq!(ksa64_viewer_transport_status_v1(handle, &mut status), 0);
                if status.worker_state == 2 {
                    assert_eq!(status.shutdown_requested, 1);
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::yield_now();
            }
            let mut finish = FinishStatusV1::default();
            assert_eq!(ksa64_viewer_finish_status_v1(handle, &mut finish), 0);
            assert_eq!(finish.shutdown_state, 2);
            assert_eq!(ksa64_viewer_destroy(handle), 0);
        }
    }
}
