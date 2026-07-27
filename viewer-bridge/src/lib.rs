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

use ksa64_host::application::{Ksa64Application, MissionDisplay, MissionPace, MissionRequest};
use ksa64_host::phase11_live::{
    LiveMissionSession, MissionOperatorAction, MissionSessionEvent, MissionSessionEventKind,
    MissionSessionLifecycle, MissionSessionPace, MissionSessionSnapshot,
};
use ksa64_interface::phase11::{
    parse_kua11, parse_kul11, write_kua11, write_kul11, UplinkCommandLoad, UplinkControlRecord,
    KUA11_LENGTH, KUL11_LENGTH,
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
pub const KSA64_VIEWER_BUILD_IDENTITY: u32 = 0x120a_0001;
pub const KSA64_VIEWER_COMMAND_CAPACITY: usize = 32;
pub const KSA64_VIEWER_EVENT_CAPACITY: usize = 256;
pub const KSA64_VIEWER_MAX_ADVANCE_RELEASES: u32 = 64;
pub const KSA64_VIEWER_MAX_CALLER_SPAN: usize = 16 * 1024 * 1024;
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
    events: VecDeque<Event>,
    recommended: Option<Vec<u8>>,
    commit: Option<Vec<u8>>,
    bundle: Option<Vec<u8>>,
    diagnostic: String,
    snapshot_publication: u64,
    event_overflow: bool,
    worker_failed: bool,
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
    let mut state = shared.lock().map_err(|_| ResultCode::Internal)?;
    state.snapshot = Some(bridge_snapshot(&snap, role, seq, result));
    state.snapshot_publication = state.snapshot_publication.saturating_add(1);
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
        *cursor = event.sequence;
    }
    state.recommended = recommended;
    state.commit = commit;
    Ok(())
}
fn apply(command: Command, live: &mut LiveMissionSession) -> Result<bool, ResultCode> {
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
            live.submit_operator_action(MissionOperatorAction::Stage {
                load: x,
                completed_event_mask: m,
            })
            .map_err(map_error)?;
        }
        Command::Commit(x) => {
            live.submit_operator_action(MissionOperatorAction::Commit(x))
                .map_err(map_error)?;
        }
        Command::Cancel(x) => {
            live.submit_operator_action(MissionOperatorAction::Cancel(x))
                .map_err(map_error)?;
        }
        Command::Abort(x) => live.abort(x).map_err(map_error)?,
        #[cfg(any(test, feature = "panic-probe"))]
        Command::PanicProbe => panic!("contained viewer bridge panic probe"),
        Command::Shutdown => return Ok(false),
    }
    Ok(true)
}
fn worker_main(
    role: String,
    role_identity: u32,
    rx: std::sync::mpsc::Receiver<Command>,
    shared: Arc<Mutex<Shared>>,
    initialized: SyncSender<Result<(), ResultCode>>,
) {
    let req = MissionRequest {
        id: "ksa-g10r.operations".into(),
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
            return;
        }
    };
    let mut seq = 0u64;
    let mut cursor = 0u32;
    if let Err(error) = publish(
        &shared,
        &live,
        role_identity,
        seq,
        ResultCode::Ok,
        &mut cursor,
    ) {
        diag(&shared, format!("initial publication failed: {error:?}"));
        let _ = initialized.send(Err(error));
        return;
    }
    if initialized.send(Ok(())).is_err() {
        return;
    }
    while let Ok(cmd) = rx.recv() {
        seq = seq.saturating_add(1);
        let result = match apply(cmd, &mut live) {
            Ok(true) => ResultCode::Ok,
            Ok(false) => break,
            Err(e) => {
                diag(&shared, format!("command {seq} failed: {e:?}"));
                e
            }
        };
        if let Err(error) = publish(&shared, &live, role_identity, seq, result, &mut cursor) {
            if let Ok(mut state) = shared.lock() {
                state.worker_failed = true;
                state.diagnostic = format!("snapshot publication failed: {error:?}");
            }
            return;
        }
        if live.lifecycle() == MissionSessionLifecycle::Completed {
            match live.finish() {
                Ok(done) => {
                    if let Ok(mut state) = shared.lock() {
                        state.bundle = Some(done.bundle)
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
    match h.commands.try_send(c) {
        Ok(()) => ResultCode::Queued,
        Err(TrySendError::Full(_)) => ResultCode::QueueFull,
        Err(TrySendError::Disconnected(_)) => ResultCode::Closed,
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
                feature_flags: u32::from(cfg!(feature = "panic-probe")),
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
#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_start(role: *const Span, out: *mut *mut Handle) -> i32 {
    boundary(|| {
        if out.is_null() {
            return ResultCode::InvalidArgument;
        }
        unsafe { *out = ptr::null_mut() }
        let b = match unsafe { copy_span(role) } {
            Ok(x) => x,
            Err(e) => return e,
        };
        let s = match str::from_utf8(&b) {
            Ok(x) => x,
            Err(_) => return ResultCode::InvalidUtf8,
        };
        let id = match role_id(s) {
            Some(x) => x,
            None => return ResultCode::Unsupported,
        };
        let (tx, rx) = sync_channel(KSA64_VIEWER_COMMAND_CAPACITY);
        let (initialized_tx, initialized_rx) = sync_channel(0);
        let shared = Arc::new(Mutex::new(Shared::default()));
        let worker_shared = Arc::clone(&shared);
        let failure_shared = Arc::clone(&shared);
        let role = s.to_owned();
        let worker = match thread::Builder::new()
            .name("ksa64-viewer-session".into())
            .spawn(move || {
                if catch_unwind(AssertUnwindSafe(|| {
                    worker_main(role, id, rx, worker_shared, initialized_tx)
                }))
                .is_err()
                {
                    if let Ok(mut state) = failure_shared.lock() {
                        state.worker_failed = true;
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
        let _ = h.commands.send(Command::Shutdown);
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
            let mut x = snap(h);
            while x.lifecycle != 5 {
                if let Some(load) = optional(ksa64_viewer_recommended_load, h) {
                    let s = Span {
                        abi_version: 1,
                        struct_size: size_of::<Span>() as u32,
                        data: load.as_ptr(),
                        length: load.len() as u64,
                    };
                    assert_eq!(ksa64_viewer_submit_stage(h, &s, 0), 1);
                    x = wait(h, x.command_sequence);
                    let commit = optional(ksa64_viewer_commit_request, h).unwrap();
                    let s = Span {
                        abi_version: 1,
                        struct_size: size_of::<Span>() as u32,
                        data: commit.as_ptr(),
                        length: commit.len() as u64,
                    };
                    assert_eq!(ksa64_viewer_submit_commit(h, &s), 1);
                    x = wait(h, x.command_sequence)
                }
                assert_eq!(ksa64_viewer_advance(h, 32), 1);
                x = wait(h, x.command_sequence);
                assert_eq!(x.command_result, 0)
            }
            let end = Instant::now() + Duration::from_secs(5);
            let got = loop {
                if let Some(b) = optional(ksa64_viewer_completed_ksb11, h) {
                    break b;
                }
                assert!(Instant::now() < end);
                thread::yield_now()
            };
            assert_eq!(got, direct);
            assert_eq!(ksa64_viewer_destroy(h), 0)
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
}
