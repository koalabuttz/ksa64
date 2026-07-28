//! Optional additive Phase 12C global-display C ABI function table.
//!
//! The frozen ABI-v1 exports remain untouched. Callers dynamically resolve the
//! single table symbol and treat its absence as GlobalDisplayV1 unavailable.

use super::{
    boundary, handles, href, next_handle_token, owned, validate, Command, Handle, HandleState,
    OwnedBuffer, ResultCode, Shared, KSA64_VIEWER_COMMAND_CAPACITY,
};
use ksa64_presentation::{
    encode_global_display_definition_payload, encode_global_display_path_payload,
    encode_global_display_samples_payload, encode_global_display_transition_payload,
    encode_global_replay_index_payload, GlobalDisplayFrameId, GlobalDisplayPathChunkV1,
    GlobalDisplayPathLod, GlobalDisplayPathPointV1, GlobalDisplaySegment, GlobalDisplaySourceId,
    PresentationRole, GLOBAL_DISCONTINUITY_TERMINAL, GLOBAL_DISPLAY_MODEL_ID,
    GLOBAL_PATH_FLAG_INCOMPLETE, GLOBAL_PATH_FLAG_STALE, GLOBAL_PATH_FLAG_TERMINAL,
    GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION, GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION,
};
use std::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::thread;

pub const KSA64_GLOBAL_DISPLAY_API_VERSION: u32 = 1;
pub const KSA64_GLOBAL_DISPLAY_API_IMPLEMENTED: u32 = 1 << 0;
pub const KSA64_GLOBAL_DISPLAY_API_ROLE_FILTERED: u32 = 1 << 1;
pub const KSA64_GLOBAL_DISPLAY_AVAILABILITY_ACCEPTED_EXACT: u32 = 1 << 0;
pub const KSA64_GLOBAL_DISPLAY_REPLAY_READ_ONLY: u32 = 1 << 0;
pub const KSA64_GLOBAL_DISPLAY_PATH_POINT_LIMIT: usize = 1_024;
pub const KSA64_GLOBAL_DISPLAY_SAMPLE_BATCH_LIMIT: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplayReplayStartRequestV1 {
    pub api_version: u32,
    pub struct_size: u32,
    pub role: u32,
    pub flags: u32,
    pub reserved: [u32; 8],
}
impl Default for GlobalDisplayReplayStartRequestV1 {
    fn default() -> Self {
        Self {
            api_version: KSA64_GLOBAL_DISPLAY_API_VERSION,
            struct_size: size_of::<Self>() as u32,
            role: PresentationRole::SimDirector as u32,
            flags: KSA64_GLOBAL_DISPLAY_REPLAY_READ_ONLY,
            reserved: [0; 8],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplayAvailabilityV1 {
    pub api_version: u32,
    pub struct_size: u32,
    pub flags: u32,
    pub role: u32,
    pub display_identity: u32,
    pub available_source_mask: u32,
    pub available_frame_mask: u32,
    pub sample_count: u32,
    pub transition_count: u32,
    /// Oldest exact release retained for non-destructive range reads.
    pub oldest_sample_release: u32,
    /// Newest exact release retained for non-destructive range reads.
    pub newest_sample_release: u32,
    pub reserved: [u32; 5],
}
impl Default for GlobalDisplayAvailabilityV1 {
    fn default() -> Self {
        Self {
            api_version: KSA64_GLOBAL_DISPLAY_API_VERSION,
            struct_size: size_of::<Self>() as u32,
            flags: 0,
            role: 0,
            display_identity: 0,
            available_source_mask: 0,
            available_frame_mask: 0,
            sample_count: 0,
            transition_count: 0,
            oldest_sample_release: 0,
            newest_sample_release: 0,
            reserved: [0; 5],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplayPathRequestV1 {
    pub api_version: u32,
    pub struct_size: u32,
    pub source: u32,
    pub display_frame: u32,
    pub lod: u32,
    pub chunk_index: u32,
    pub reserved: [u32; 6],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplaySampleRangeRequestV1 {
    pub api_version: u32,
    pub struct_size: u32,
    pub start_release: u32,
    pub max_count: u32,
    pub flags: u32,
    pub reserved: [u32; 7],
}
impl Default for GlobalDisplaySampleRangeRequestV1 {
    fn default() -> Self {
        Self {
            api_version: KSA64_GLOBAL_DISPLAY_API_VERSION,
            struct_size: size_of::<Self>() as u32,
            start_release: 0,
            max_count: 1,
            flags: 0,
            reserved: [0; 7],
        }
    }
}

impl Default for GlobalDisplayPathRequestV1 {
    fn default() -> Self {
        Self {
            api_version: KSA64_GLOBAL_DISPLAY_API_VERSION,
            struct_size: size_of::<Self>() as u32,
            source: GlobalDisplaySourceId::OnboardEstimate as u32,
            display_frame: GlobalDisplayFrameId::EarthFixedEcef as u32,
            lod: GlobalDisplayPathLod::OneSecond as u32,
            chunk_index: 0,
            reserved: [0; 6],
        }
    }
}

pub type GlobalReplayStartFn =
    unsafe extern "C" fn(*const GlobalDisplayReplayStartRequestV1, *mut *mut Handle) -> i32;
pub type GlobalAvailabilityFn =
    unsafe extern "C" fn(*const Handle, *mut GlobalDisplayAvailabilityV1) -> i32;
pub type GlobalPayloadFn = unsafe extern "C" fn(*const Handle, *mut OwnedBuffer) -> i32;
pub type GlobalPathFn =
    unsafe extern "C" fn(*const Handle, *const GlobalDisplayPathRequestV1, *mut OwnedBuffer) -> i32;
pub type GlobalSampleRangeFn = unsafe extern "C" fn(
    *const Handle,
    *const GlobalDisplaySampleRangeRequestV1,
    *mut OwnedBuffer,
) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GlobalDisplayApiV1 {
    pub api_version: u32,
    pub struct_size: u32,
    pub feature_flags: u32,
    pub replay_start_request_size: u32,
    pub availability_size: u32,
    pub path_request_size: u32,
    pub sample_range_request_size: u32,
    pub owned_buffer_size: u32,
    pub start_nominal_replay: Option<GlobalReplayStartFn>,
    pub availability: Option<GlobalAvailabilityFn>,
    pub definition_payload: Option<GlobalPayloadFn>,
    pub poll_sample_payload: Option<GlobalPayloadFn>,
    pub sample_range_payload: Option<GlobalSampleRangeFn>,
    pub poll_transition_payload: Option<GlobalPayloadFn>,
    pub replay_index_payload: Option<GlobalPayloadFn>,
    pub path_chunk_payload: Option<GlobalPathFn>,
    pub reserved: [u64; 6],
}
impl Default for GlobalDisplayApiV1 {
    fn default() -> Self {
        Self {
            api_version: KSA64_GLOBAL_DISPLAY_API_VERSION,
            struct_size: size_of::<Self>() as u32,
            feature_flags: KSA64_GLOBAL_DISPLAY_API_IMPLEMENTED
                | KSA64_GLOBAL_DISPLAY_API_ROLE_FILTERED,
            replay_start_request_size: size_of::<GlobalDisplayReplayStartRequestV1>() as u32,
            availability_size: size_of::<GlobalDisplayAvailabilityV1>() as u32,
            path_request_size: size_of::<GlobalDisplayPathRequestV1>() as u32,
            sample_range_request_size: size_of::<GlobalDisplaySampleRangeRequestV1>() as u32,
            owned_buffer_size: size_of::<OwnedBuffer>() as u32,
            start_nominal_replay: Some(global_start_nominal_replay),
            availability: Some(global_availability),
            definition_payload: Some(global_definition_payload),
            poll_sample_payload: Some(global_poll_sample_payload),
            sample_range_payload: Some(global_sample_range_payload),
            poll_transition_payload: Some(global_poll_transition_payload),
            replay_index_payload: Some(global_replay_index_payload),
            path_chunk_payload: Some(global_path_chunk_payload),
            reserved: [0; 6],
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ksa64_viewer_global_display_api_v1(
    output: *mut GlobalDisplayApiV1,
) -> i32 {
    boundary(|| {
        if output.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*output };
        if header.api_version != KSA64_GLOBAL_DISPLAY_API_VERSION {
            return ResultCode::AbiMismatch;
        }
        if header.struct_size as usize != size_of::<GlobalDisplayApiV1>() {
            return ResultCode::StructSize;
        }
        if header.reserved.iter().any(|value| *value != 0) {
            return ResultCode::InvalidArgument;
        }
        unsafe { *output = GlobalDisplayApiV1::default() };
        ResultCode::Ok
    })
}

unsafe extern "C" fn global_start_nominal_replay(
    request: *const GlobalDisplayReplayStartRequestV1,
    output: *mut *mut Handle,
) -> i32 {
    boundary(|| {
        if request.is_null() || output.is_null() {
            return ResultCode::InvalidArgument;
        }
        unsafe { *output = std::ptr::null_mut() };
        let request = unsafe { &*request };
        if request.api_version != KSA64_GLOBAL_DISPLAY_API_VERSION {
            return ResultCode::AbiMismatch;
        }
        if request.struct_size as usize != size_of::<GlobalDisplayReplayStartRequestV1>() {
            return ResultCode::StructSize;
        }
        if request.flags != KSA64_GLOBAL_DISPLAY_REPLAY_READ_ONLY
            || request.reserved.iter().any(|value| *value != 0)
        {
            return ResultCode::InvalidArgument;
        }
        let Some(role) = PresentationRole::from_raw(request.role as u8) else {
            return ResultCode::Unsupported;
        };
        if request.role > u8::MAX as u32 {
            return ResultCode::Unsupported;
        }

        let (commands, receiver) = sync_channel(KSA64_VIEWER_COMMAND_CAPACITY);
        let shared = Arc::new(Mutex::new(Shared::default()));
        let worker_shared = Arc::clone(&shared);
        let failure_shared = Arc::clone(&shared);
        let pending_commands = Arc::new(AtomicUsize::new(0));
        let worker_pending = Arc::clone(&pending_commands);
        let worker = match thread::Builder::new()
            .name("ksa64-global-replay".into())
            .spawn(move || {
                let built = catch_unwind(AssertUnwindSafe(|| {
                    ksa64_session::global_display::build_nominal_global_display_replay()
                }));
                match built {
                    Ok(Ok(replay)) => {
                        let definition = replay.definition(role);
                        let samples = replay.samples_after(0, role);
                        let transitions = replay.transitions_after(0).to_vec();
                        let index = replay.replay_index();
                        let planned = replay.planned_samples().to_vec();
                        if let Ok(mut state) = worker_shared.lock() {
                            state.global_role = Some(role);
                            state.global_definition = Some(definition);
                            state.global_all_samples = samples;
                            state.global_planned_samples = planned;
                            state.global_transition_count = transitions.len() as u32;
                            state.global_transitions = transitions.into();
                            state.global_replay_index = Some(index);
                            state.global_accepted_exact = true;
                            state.diagnostic =
                                "verified Phase 10 nominal global replay ready".to_owned();
                        }
                    }
                    Ok(Err(error)) => {
                        if let Ok(mut state) = failure_shared.lock() {
                            state.worker_failed = true;
                            state.diagnostic =
                                format!("nominal global replay validation failed: {error:?}");
                        }
                    }
                    Err(_) => {
                        if let Ok(mut state) = failure_shared.lock() {
                            state.worker_failed = true;
                            state.diagnostic =
                                "nominal global replay worker panicked and was contained"
                                    .to_owned();
                        }
                    }
                }
                while let Ok(command) = receiver.recv() {
                    worker_pending.fetch_sub(1, Ordering::AcqRel);
                    if matches!(command, Command::Shutdown) {
                        break;
                    }
                    if let Ok(mut state) = worker_shared.lock() {
                        state.last_command_result = ResultCode::Unsupported as i32;
                    }
                }
            }) {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        let state = Arc::new(HandleState {
            commands,
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
        unsafe { *output = token as *mut Handle };
        ResultCode::Ok
    })
}

unsafe extern "C" fn global_availability(
    handle: *const Handle,
    output: *mut GlobalDisplayAvailabilityV1,
) -> i32 {
    boundary(|| {
        let handle = match href(handle) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if output.is_null() {
            return ResultCode::InvalidArgument;
        }
        let header = unsafe { &*output };
        if header.api_version != KSA64_GLOBAL_DISPLAY_API_VERSION {
            return ResultCode::AbiMismatch;
        }
        if header.struct_size as usize != size_of::<GlobalDisplayAvailabilityV1>() {
            return ResultCode::StructSize;
        }
        if header.reserved.iter().any(|value| *value != 0) {
            return ResultCode::InvalidArgument;
        }
        let state = match handle.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        let Some(definition) = state.global_definition else {
            return ResultCode::NoData;
        };
        unsafe {
            *output = GlobalDisplayAvailabilityV1 {
                flags: if state.global_accepted_exact {
                    KSA64_GLOBAL_DISPLAY_AVAILABILITY_ACCEPTED_EXACT
                } else {
                    0
                },
                role: state.global_role.map_or(0, |role| role as u32),
                display_identity: definition.display_identity,
                available_source_mask: definition.available_source_mask,
                available_frame_mask: u32::from(definition.available_frame_mask),
                sample_count: state.global_all_samples.len().min(u32::MAX as usize) as u32,
                transition_count: state.global_transition_count,
                oldest_sample_release: state
                    .global_all_samples
                    .first()
                    .map_or(0, |sample| sample.release_epoch),
                newest_sample_release: state
                    .global_all_samples
                    .last()
                    .map_or(0, |sample| sample.release_epoch),
                ..GlobalDisplayAvailabilityV1::default()
            }
        };
        ResultCode::Ok
    })
}

unsafe extern "C" fn global_definition_payload(
    handle: *const Handle,
    output: *mut OwnedBuffer,
) -> i32 {
    global_buffer(handle, output, |state, role| {
        let definition = state.global_definition?;
        encode_global_display_definition_payload(definition, role).ok()
    })
}

unsafe extern "C" fn global_poll_sample_payload(
    handle: *const Handle,
    output: *mut OwnedBuffer,
) -> i32 {
    boundary(|| {
        let handle = match href(handle) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if let Err(error) = validate_output(output) {
            return error;
        }
        let mut state = match handle.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.global_sample_overflow {
            return ResultCode::EventOverflow;
        }
        let role = match state.global_role {
            Some(value) => value,
            None => return ResultCode::NoData,
        };
        let Some(sample) = state.global_samples.pop_front() else {
            return ResultCode::NoData;
        };
        write_owned(
            output,
            match encode_global_display_samples_payload(&[sample], role) {
                Ok(value) => value,
                Err(_) => return ResultCode::Internal,
            },
        )
    })
}

unsafe extern "C" fn global_sample_range_payload(
    handle: *const Handle,
    request: *const GlobalDisplaySampleRangeRequestV1,
    output: *mut OwnedBuffer,
) -> i32 {
    boundary(|| {
        if request.is_null() {
            return ResultCode::InvalidArgument;
        }
        let request = unsafe { &*request };
        if request.api_version != KSA64_GLOBAL_DISPLAY_API_VERSION {
            return ResultCode::AbiMismatch;
        }
        if request.struct_size as usize != size_of::<GlobalDisplaySampleRangeRequestV1>() {
            return ResultCode::StructSize;
        }
        if request.flags != 0
            || request.reserved.iter().any(|value| *value != 0)
            || request.max_count == 0
            || request.max_count > KSA64_GLOBAL_DISPLAY_SAMPLE_BATCH_LIMIT as u32
        {
            return ResultCode::InvalidArgument;
        }
        let handle = match href(handle) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if let Err(error) = validate_output(output) {
            return error;
        }
        let state = match handle.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        let role = match state.global_role {
            Some(value) => value,
            None => return ResultCode::NoData,
        };
        let Some(oldest) = state.global_all_samples.first() else {
            return ResultCode::NoData;
        };
        let newest = state.global_all_samples.last().expect("first checked");
        if request.start_release < oldest.release_epoch {
            return ResultCode::EventOverflow;
        }
        if request.start_release > newest.release_epoch {
            return ResultCode::NoData;
        }
        let start = state
            .global_all_samples
            .partition_point(|sample| sample.release_epoch < request.start_release);
        if state.global_all_samples[start].release_epoch != request.start_release {
            return ResultCode::EventOverflow;
        }
        let end = (start + request.max_count as usize).min(state.global_all_samples.len());
        write_owned(
            output,
            match encode_global_display_samples_payload(&state.global_all_samples[start..end], role)
            {
                Ok(value) => value,
                Err(_) => return ResultCode::Internal,
            },
        )
    })
}

unsafe extern "C" fn global_poll_transition_payload(
    handle: *const Handle,
    output: *mut OwnedBuffer,
) -> i32 {
    boundary(|| {
        let handle = match href(handle) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if let Err(error) = validate_output(output) {
            return error;
        }
        let mut state = match handle.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        if state.global_transition_overflow {
            return ResultCode::EventOverflow;
        }
        let Some(transition) = state.global_transitions.pop_front() else {
            return ResultCode::NoData;
        };
        write_owned(
            output,
            match encode_global_display_transition_payload(transition) {
                Ok(value) => value,
                Err(_) => return ResultCode::Internal,
            },
        )
    })
}

unsafe extern "C" fn global_replay_index_payload(
    handle: *const Handle,
    output: *mut OwnedBuffer,
) -> i32 {
    global_buffer(handle, output, |state, _role| {
        encode_global_replay_index_payload(state.global_replay_index.as_ref()?).ok()
    })
}

unsafe extern "C" fn global_path_chunk_payload(
    handle: *const Handle,
    request: *const GlobalDisplayPathRequestV1,
    output: *mut OwnedBuffer,
) -> i32 {
    boundary(|| {
        if request.is_null() {
            return ResultCode::InvalidArgument;
        }
        let request = unsafe { &*request };
        if request.api_version != KSA64_GLOBAL_DISPLAY_API_VERSION {
            return ResultCode::AbiMismatch;
        }
        if request.struct_size as usize != size_of::<GlobalDisplayPathRequestV1>() {
            return ResultCode::StructSize;
        }
        if request.reserved.iter().any(|value| *value != 0) {
            return ResultCode::InvalidArgument;
        }
        let source = match request.source {
            1 => GlobalDisplaySourceId::Planned,
            2 => GlobalDisplaySourceId::OnboardEstimate,
            3 => GlobalDisplaySourceId::GroundEstimate,
            4 => GlobalDisplaySourceId::SimTruth,
            _ => return ResultCode::InvalidArgument,
        };
        let frame = match request.display_frame {
            1 => GlobalDisplayFrameId::LocalEnu,
            2 => GlobalDisplayFrameId::EarthFixedEcef,
            3 => GlobalDisplayFrameId::EarthInertialGcrf,
            _ => return ResultCode::InvalidArgument,
        };
        let lod = match request.lod {
            1 => GlobalDisplayPathLod::Exact,
            2 => GlobalDisplayPathLod::OneSecond,
            3 => GlobalDisplayPathLod::FourSecond,
            _ => return ResultCode::InvalidArgument,
        };
        let chunk_index = match u16::try_from(request.chunk_index) {
            Ok(value) => value,
            Err(_) => return ResultCode::InvalidArgument,
        };
        let handle = match href(handle) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if let Err(error) = validate_output(output) {
            return error;
        }
        let state = match handle.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        let role = match state.global_role {
            Some(value) => value,
            None => return ResultCode::NoData,
        };
        if source == GlobalDisplaySourceId::SimTruth && !role.permits_private_truth() {
            return ResultCode::Unsupported;
        }
        let Some(path) = path_from_samples(&state, source, frame, lod, chunk_index) else {
            return ResultCode::NoData;
        };
        write_owned(
            output,
            match encode_global_display_path_payload(&path, role) {
                Ok(value) => value,
                Err(_) => return ResultCode::Internal,
            },
        )
    })
}

fn global_buffer(
    handle: *const Handle,
    output: *mut OwnedBuffer,
    encode: impl FnOnce(&Shared, PresentationRole) -> Option<Vec<u8>>,
) -> i32 {
    boundary(|| {
        let handle = match href(handle) {
            Ok(value) => value,
            Err(error) => return error,
        };
        if let Err(error) = validate_output(output) {
            return error;
        }
        let state = match handle.shared.lock() {
            Ok(value) => value,
            Err(_) => return ResultCode::Internal,
        };
        let role = match state.global_role {
            Some(value) => value,
            None => return ResultCode::NoData,
        };
        let Some(bytes) = encode(&state, role) else {
            return ResultCode::NoData;
        };
        write_owned(output, bytes)
    })
}

fn validate_output(output: *mut OwnedBuffer) -> Result<(), ResultCode> {
    if output.is_null() {
        return Err(ResultCode::InvalidArgument);
    }
    let value = unsafe { &*output };
    validate(
        value.abi_version,
        value.struct_size,
        size_of::<OwnedBuffer>(),
    )?;
    if !value.data.is_null() || value.length != 0 || value.allocation_id != 0 {
        return Err(ResultCode::InvalidArgument);
    }
    Ok(())
}

fn write_owned(output: *mut OwnedBuffer, bytes: Vec<u8>) -> ResultCode {
    match owned(bytes) {
        Ok(value) => {
            unsafe { *output = value };
            ResultCode::Ok
        }
        Err(error) => error,
    }
}

fn path_from_samples(
    state: &Shared,
    source: GlobalDisplaySourceId,
    frame: GlobalDisplayFrameId,
    lod: GlobalDisplayPathLod,
    chunk_index: u16,
) -> Option<GlobalDisplayPathChunkV1> {
    let samples = if source == GlobalDisplaySourceId::Planned {
        &state.global_planned_samples
    } else {
        &state.global_all_samples
    };
    if source == GlobalDisplaySourceId::Planned && lod == GlobalDisplayPathLod::Exact {
        return None;
    }
    let definition = state.global_definition?;
    let mut points = Vec::new();
    for (sample_index, sample) in samples.iter().enumerate() {
        let pinned = sample.event_mask != 0 || sample.discontinuity_mask != 0;
        let retain = if source == GlobalDisplaySourceId::Planned {
            lod == GlobalDisplayPathLod::OneSecond || sample_index.is_multiple_of(4) || pinned
        } else {
            sample.release_epoch.is_multiple_of(lod.cadence_releases()) || pinned
        };
        if !retain {
            continue;
        }
        let Some(pose) = sample.sources.iter().find(|pose| pose.source == source) else {
            continue;
        };
        let position = match frame {
            GlobalDisplayFrameId::LocalEnu
                if sample.segment == GlobalDisplaySegment::LocalRecovery =>
            {
                if pose.validity_mask & GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION == 0 {
                    continue;
                }
                pose.recovery_enu.position_q12_km
            }
            GlobalDisplayFrameId::LocalEnu => {
                if pose.validity_mask & GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION == 0 {
                    continue;
                }
                pose.launch_enu.position_q12_km
            }
            GlobalDisplayFrameId::EarthFixedEcef => pose.ecef.position_q12_km,
            GlobalDisplayFrameId::EarthInertialGcrf => pose.gcrf.position_q12_km,
        };
        points.push(GlobalDisplayPathPointV1 {
            release_epoch: sample.release_epoch,
            mission_time_q16: sample.mission_time_q16,
            segment: sample.segment,
            event_mask: sample.event_mask,
            anchor_identity: if frame != GlobalDisplayFrameId::LocalEnu {
                0
            } else if sample.segment == GlobalDisplaySegment::LocalRecovery {
                definition.recovery_anchor.identity
            } else {
                definition.launch_anchor.identity
            },
            position_q12_km: position,
        });
    }
    if points.is_empty() {
        return None;
    }
    let chunk_count =
        u16::try_from(points.len().div_ceil(KSA64_GLOBAL_DISPLAY_PATH_POINT_LIMIT)).ok()?;
    if chunk_index >= chunk_count {
        return None;
    }
    let start = usize::from(chunk_index) * KSA64_GLOBAL_DISPLAY_PATH_POINT_LIMIT;
    let end = (start + KSA64_GLOBAL_DISPLAY_PATH_POINT_LIMIT).min(points.len());
    let last = samples.last()?;
    let latest_source_pose = last.sources.iter().find(|pose| pose.source == source);
    let source_pose = samples
        .iter()
        .rev()
        .find_map(|sample| sample.sources.iter().find(|pose| pose.source == source));
    let terminal = source == GlobalDisplaySourceId::Planned
        || last.discontinuity_mask & GLOBAL_DISCONTINUITY_TERMINAL != 0;
    let stale = latest_source_pose.is_none()
        || latest_source_pose.is_some_and(|pose| pose.age_releases > 32);
    let incomplete = !terminal
        || latest_source_pose.is_none()
        || points.first().is_some_and(|point| {
            point.release_epoch
                > samples
                    .first()
                    .map_or(point.release_epoch, |sample| sample.release_epoch)
        });
    let mut flags = 0;
    if stale {
        flags |= GLOBAL_PATH_FLAG_STALE;
    }
    if incomplete {
        flags |= GLOBAL_PATH_FLAG_INCOMPLETE;
    }
    if terminal {
        flags |= GLOBAL_PATH_FLAG_TERMINAL;
    }
    let path_identity = hash_words(&[
        definition.display_identity,
        source as u32,
        frame as u32,
        lod as u32,
    ]);
    Some(GlobalDisplayPathChunkV1 {
        path_identity,
        source,
        display_frame: frame,
        lod,
        flags,
        model_identity: source_pose.map_or(GLOBAL_DISPLAY_MODEL_ID ^ source as u32, |pose| {
            pose.model_identity
        }),
        estimate_identity: source_pose.map_or(0, |pose| pose.estimate_identity),
        source_checksum: source_pose.map_or(0, |pose| pose.checksum),
        continuity_identity: path_identity,
        chunk_index,
        chunk_count,
        points: points[start..end].to_vec(),
    })
}

fn hash_words(words: &[u32]) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for word in words {
        for byte in word.to_le_bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    hash.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_presentation::{
        decode_global_display_path_payload, decode_global_display_samples_payload,
        decode_global_replay_index_payload,
    };
    use std::ptr;
    use std::slice;
    use std::time::{Duration, Instant};

    fn empty_buffer() -> OwnedBuffer {
        OwnedBuffer {
            abi_version: crate::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<OwnedBuffer>() as u32,
            data: ptr::null_mut(),
            length: 0,
            allocation_id: 0,
        }
    }

    unsafe fn take_buffer(mut buffer: OwnedBuffer) -> Vec<u8> {
        let bytes = if buffer.length == 0 {
            Vec::new()
        } else {
            unsafe { slice::from_raw_parts(buffer.data, buffer.length as usize) }.to_vec()
        };
        assert_eq!(unsafe { crate::ksa64_viewer_free_buffer(&mut buffer) }, 0);
        bytes
    }

    unsafe fn start_full(role: u32) -> *mut Handle {
        let mut handle = ptr::null_mut();
        let request = crate::StartRequestV1 {
            role,
            initial_pace: 4,
            flags: 0,
            ..crate::StartRequestV1::default()
        };
        assert_eq!(
            unsafe { crate::ksa64_viewer_start_v1(&request, &mut handle) },
            0
        );
        handle
    }

    unsafe fn snapshot(handle: *mut Handle) -> crate::Snapshot {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let mut value = crate::Snapshot::default();
            match unsafe { crate::ksa64_viewer_poll_snapshot(handle, &mut value) } {
                0 => return value,
                2 | 3 if Instant::now() < end => std::thread::yield_now(),
                result => panic!("snapshot result {result}"),
            }
        }
    }

    unsafe fn wait_after(handle: *mut Handle, command_sequence: u64) -> crate::Snapshot {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let value = unsafe { snapshot(handle) };
            if value.command_sequence > command_sequence {
                return value;
            }
            assert!(Instant::now() < end);
            std::thread::yield_now();
        }
    }

    #[test]
    fn additive_table_and_request_layouts_are_frozen() {
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<GlobalDisplayApiV1>(), 144);
        #[cfg(target_pointer_width = "32")]
        assert_eq!(size_of::<GlobalDisplayApiV1>(), 88);
        assert_eq!(size_of::<GlobalDisplayReplayStartRequestV1>(), 48);
        assert_eq!(size_of::<GlobalDisplayAvailabilityV1>(), 64);
        assert_eq!(size_of::<GlobalDisplayPathRequestV1>(), 48);
        assert_eq!(size_of::<GlobalDisplaySampleRangeRequestV1>(), 48);
        let api = GlobalDisplayApiV1::default();
        assert_eq!(api.api_version, 1);
        assert!(api.availability.is_some());
        assert!(api.path_chunk_payload.is_some());
    }

    #[test]
    fn optional_table_rejects_wrong_version_without_touching_base_abi() {
        let mut api = GlobalDisplayApiV1 {
            api_version: 2,
            ..GlobalDisplayApiV1::default()
        };
        assert_eq!(
            unsafe { ksa64_viewer_global_display_api_v1(&mut api) },
            ResultCode::AbiMismatch as i32
        );
    }

    #[test]
    fn global_table_range_seek_is_role_filtered_and_fail_closed() {
        let _guard = crate::tests::FULL_SESSION_TEST_LOCK
            .lock()
            .expect("full session test lock");
        unsafe {
            let mut table = GlobalDisplayApiV1::default();
            assert_eq!(ksa64_viewer_global_display_api_v1(&mut table), 0);
            let availability = table.availability.expect("availability");
            let range = table.sample_range_payload.expect("sample range");
            let path = table.path_chunk_payload.expect("path");
            let index = table.replay_index_payload.expect("replay index");

            let guided = start_full(PresentationRole::GuidedOperator as u32);
            let before = snapshot(guided);
            let mut initial = GlobalDisplayAvailabilityV1::default();
            assert_eq!(availability(guided, &mut initial), 0);

            let mut invalid = GlobalDisplaySampleRangeRequestV1::default();
            invalid.reserved[0] = 1;
            let mut output = empty_buffer();
            assert_eq!(
                range(guided, &invalid, &mut output),
                ResultCode::InvalidArgument as i32
            );
            assert!(output.data.is_null());
            let mut after_invalid = GlobalDisplayAvailabilityV1::default();
            assert_eq!(availability(guided, &mut after_invalid), 0);
            assert_eq!(initial.sample_count, after_invalid.sample_count);

            assert_eq!(
                crate::ksa64_viewer_advance(guided, 1),
                ResultCode::Queued as i32
            );
            let advanced = wait_after(guided, before.command_sequence);
            assert_eq!(advanced.release_epoch, 1);
            let mut available = GlobalDisplayAvailabilityV1::default();
            assert_eq!(availability(guided, &mut available), 0);
            assert_eq!(available.sample_count, 1);
            assert_eq!(available.oldest_sample_release, 0);
            assert_eq!(available.newest_sample_release, 0);

            let request = GlobalDisplaySampleRangeRequestV1 {
                start_release: 0,
                max_count: 1,
                ..GlobalDisplaySampleRangeRequestV1::default()
            };
            assert_eq!(range(guided, &request, &mut output), 0);
            let samples = decode_global_display_samples_payload(
                &take_buffer(output),
                PresentationRole::GuidedOperator,
            )
            .expect("guided samples");
            assert_eq!(samples.len(), 1);
            assert_eq!(samples[0].release_epoch, 0);
            assert!(samples[0]
                .sources
                .iter()
                .all(|pose| pose.source != GlobalDisplaySourceId::SimTruth));

            let planned_request = GlobalDisplayPathRequestV1 {
                source: GlobalDisplaySourceId::Planned as u32,
                display_frame: GlobalDisplayFrameId::EarthFixedEcef as u32,
                lod: GlobalDisplayPathLod::OneSecond as u32,
                ..GlobalDisplayPathRequestV1::default()
            };
            output = empty_buffer();
            assert_eq!(path(guided, &planned_request, &mut output), 0);
            let planned = decode_global_display_path_payload(
                &take_buffer(output),
                PresentationRole::GuidedOperator,
            )
            .expect("planned path");
            assert_eq!(planned.source, GlobalDisplaySourceId::Planned);
            assert!(planned.points.len() > 16);

            output = empty_buffer();
            assert_eq!(index(guided, &mut output), 0);
            let replay =
                decode_global_replay_index_payload(&take_buffer(output)).expect("replay index");
            assert_eq!(replay.first_release, 0);
            assert_eq!(replay.last_release, 0);
            assert_eq!(crate::ksa64_viewer_destroy(guided), 0);

            let director = start_full(PresentationRole::SimDirector as u32);
            let before = snapshot(director);
            assert_eq!(
                crate::ksa64_viewer_advance(director, 1),
                ResultCode::Queued as i32
            );
            let _ = wait_after(director, before.command_sequence);
            output = empty_buffer();
            assert_eq!(range(director, &request, &mut output), 0);
            let samples = decode_global_display_samples_payload(
                &take_buffer(output),
                PresentationRole::SimDirector,
            )
            .expect("director samples");
            assert!(samples[0]
                .sources
                .iter()
                .any(|pose| pose.source == GlobalDisplaySourceId::SimTruth));
            assert_eq!(crate::ksa64_viewer_destroy(director), 0);
        }
    }
}
