//! Renderer-neutral Phase 12C global-display products.
//!
//! These records are noncanonical presentation data. They carry fixed-width
//! values derived by Rust from accepted mission state. Renderers may rebase or
//! interpolate compatible samples but never own frames, truth, or evidence.

use crate::{Kps1Error, PresentationRole, KPS1_MAX_PAYLOAD_LENGTH};
use alloc::vec::Vec;

pub const GLOBAL_DISPLAY_MODEL_ID: u32 = 0x12c0_0001;
pub const GLOBAL_DISPLAY_MAX_SOURCES: usize = 4;
pub const GLOBAL_DISPLAY_MAX_PATH_POINTS: usize = 4_096;
pub const GLOBAL_DISPLAY_MAX_REPLAY_ENTRIES: usize = 512;

pub const GLOBAL_DISPLAY_SOURCE_PLANNED: u32 = 1 << 0;
pub const GLOBAL_DISPLAY_SOURCE_ONBOARD: u32 = 1 << 1;
pub const GLOBAL_DISPLAY_SOURCE_GROUND: u32 = 1 << 2;
pub const GLOBAL_DISPLAY_SOURCE_SIM_TRUTH: u32 = 1 << 3;
pub const GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK: u32 =
    GLOBAL_DISPLAY_SOURCE_PLANNED | GLOBAL_DISPLAY_SOURCE_ONBOARD | GLOBAL_DISPLAY_SOURCE_GROUND;
pub const GLOBAL_DISPLAY_SOURCE_MASK: u32 =
    GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK | GLOBAL_DISPLAY_SOURCE_SIM_TRUTH;

pub const GLOBAL_POSE_VALID_ACTIVE_POSITION: u32 = 1 << 0;
pub const GLOBAL_POSE_VALID_ACTIVE_VELOCITY: u32 = 1 << 1;
pub const GLOBAL_POSE_VALID_ACTIVE_ATTITUDE: u32 = 1 << 2;
pub const GLOBAL_POSE_VALID_ANGULAR_RATE: u32 = 1 << 3;
pub const GLOBAL_POSE_VALID_ECEF_POSITION: u32 = 1 << 4;
pub const GLOBAL_POSE_VALID_ECEF_VELOCITY: u32 = 1 << 5;
pub const GLOBAL_POSE_VALID_ECEF_ATTITUDE: u32 = 1 << 6;
pub const GLOBAL_POSE_VALID_GCRF_POSITION: u32 = 1 << 7;
pub const GLOBAL_POSE_VALID_GCRF_VELOCITY: u32 = 1 << 8;
pub const GLOBAL_POSE_VALID_GCRF_ATTITUDE: u32 = 1 << 9;
pub const GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION: u32 = 1 << 10;
pub const GLOBAL_POSE_VALID_LAUNCH_ENU_VELOCITY: u32 = 1 << 11;
pub const GLOBAL_POSE_VALID_LAUNCH_ENU_ATTITUDE: u32 = 1 << 12;
pub const GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION: u32 = 1 << 13;
pub const GLOBAL_POSE_VALID_RECOVERY_ENU_VELOCITY: u32 = 1 << 14;
pub const GLOBAL_POSE_VALID_RECOVERY_ENU_ATTITUDE: u32 = 1 << 15;
pub const GLOBAL_POSE_VALID_MASK: u32 = (1 << 16) - 1;

pub const GLOBAL_DISCONTINUITY_FRAME: u32 = 1 << 0;
pub const GLOBAL_DISCONTINUITY_SEGMENT: u32 = 1 << 1;
pub const GLOBAL_DISCONTINUITY_DEPLOYMENT: u32 = 1 << 2;
pub const GLOBAL_DISCONTINUITY_ATTITUDE_RETIRED: u32 = 1 << 3;
pub const GLOBAL_DISCONTINUITY_NAVIGATION_RESET: u32 = 1 << 4;
pub const GLOBAL_DISCONTINUITY_SOURCE_REPLACED: u32 = 1 << 5;
pub const GLOBAL_DISCONTINUITY_HISTORY_GAP: u32 = 1 << 6;
pub const GLOBAL_DISCONTINUITY_REPLAY_SEEK: u32 = 1 << 7;
pub const GLOBAL_DISCONTINUITY_TERMINAL: u32 = 1 << 8;
pub const GLOBAL_DISCONTINUITY_MASK: u32 = (1 << 9) - 1;

pub const GLOBAL_PATH_FLAG_STALE: u16 = 1 << 0;
pub const GLOBAL_PATH_FLAG_INCOMPLETE: u16 = 1 << 1;
pub const GLOBAL_PATH_FLAG_TERMINAL: u16 = 1 << 2;
pub const GLOBAL_PATH_FLAG_RESYNC_REQUIRED: u16 = 1 << 3;
pub const GLOBAL_PATH_FLAG_MASK: u16 = (1 << 4) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalDisplayFrameId {
    LocalEnu = 1,
    EarthFixedEcef = 2,
    EarthInertialGcrf = 3,
}
impl GlobalDisplayFrameId {
    pub const fn from_raw(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::LocalEnu),
            2 => Some(Self::EarthFixedEcef),
            3 => Some(Self::EarthInertialGcrf),
            _ => None,
        }
    }
    pub const fn mask(self) -> u8 {
        1 << (self as u8 - 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalDisplaySegment {
    LocalLaunch = 1,
    EcefAscent = 2,
    EciCoast = 3,
    EcefEntry = 4,
    LocalRecovery = 5,
}
impl GlobalDisplaySegment {
    pub const fn from_raw(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::LocalLaunch),
            2 => Some(Self::EcefAscent),
            3 => Some(Self::EciCoast),
            4 => Some(Self::EcefEntry),
            5 => Some(Self::LocalRecovery),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalDisplaySourceId {
    Planned = 1,
    OnboardEstimate = 2,
    GroundEstimate = 3,
    SimTruth = 4,
}
impl GlobalDisplaySourceId {
    pub const fn from_raw(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Planned),
            2 => Some(Self::OnboardEstimate),
            3 => Some(Self::GroundEstimate),
            4 => Some(Self::SimTruth),
            _ => None,
        }
    }
    pub const fn mask(self) -> u32 {
        1 << (self as u8 - 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalDisplayPathLod {
    Exact = 1,
    OneSecond = 2,
    FourSecond = 3,
}
impl GlobalDisplayPathLod {
    pub const fn from_raw(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Exact),
            2 => Some(Self::OneSecond),
            3 => Some(Self::FourSecond),
            _ => None,
        }
    }
    pub const fn cadence_releases(self) -> u32 {
        match self {
            Self::Exact => 1,
            Self::OneSecond => 32,
            Self::FourSecond => 128,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalDisplayReplayEntryKind {
    MissionEvent = 1,
    FrameTransition = 2,
    ProcedureAction = 3,
    Fault = 4,
    Terminal = 5,
}
impl GlobalDisplayReplayEntryKind {
    pub const fn from_raw(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::MissionEvent),
            2 => Some(Self::FrameTransition),
            3 => Some(Self::ProcedureAction),
            4 => Some(Self::Fault),
            5 => Some(Self::Terminal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobalDisplayAnchorV1 {
    pub identity: u32,
    pub geodetic_q28_q12: [i32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplayDefinitionV1 {
    pub display_identity: u32,
    pub earth_identity: u32,
    pub transform_identity: u32,
    pub mission_identity: u32,
    pub epoch_unix_day: i32,
    pub epoch_tai_minus_utc: i16,
    pub semi_major_q12_km: i32,
    pub semi_minor_q12_km: i32,
    pub inverse_flattening_q20: i32,
    pub launch_anchor: GlobalDisplayAnchorV1,
    pub recovery_anchor: GlobalDisplayAnchorV1,
    pub available_source_mask: u32,
    pub available_frame_mask: u8,
    pub camera_domain_mask: u16,
}
impl GlobalDisplayDefinitionV1 {
    pub fn filter_for_role(mut self, role: PresentationRole) -> Self {
        if !role.permits_private_truth() {
            self.available_source_mask &= GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK;
        }
        self
    }
    pub fn validate(self, role: PresentationRole) -> Result<(), Kps1Error> {
        if self.display_identity == 0
            || self.earth_identity == 0
            || self.transform_identity == 0
            || self.mission_identity == 0
            || self.launch_anchor.identity == 0
            || self.recovery_anchor.identity == 0
            || self.semi_major_q12_km <= 0
            || self.semi_minor_q12_km <= 0
            || self.inverse_flattening_q20 <= 0
            || self.available_source_mask == 0
            || self.available_source_mask & !GLOBAL_DISPLAY_SOURCE_MASK != 0
            || self.available_frame_mask == 0
            || self.available_frame_mask & !7 != 0
            || self.camera_domain_mask == 0
        {
            return Err(Kps1Error::Identity);
        }
        if !role.permits_private_truth()
            && self.available_source_mask & GLOBAL_DISPLAY_SOURCE_SIM_TRUTH != 0
        {
            return Err(Kps1Error::Enum);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobalDisplayResolvedPoseV1 {
    pub position_q12_km: [i32; 3],
    pub velocity_q24_km_s: [i32; 3],
    pub attitude_q30: [i32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplaySourcePoseV1 {
    pub source: GlobalDisplaySourceId,
    pub active_frame: GlobalDisplayFrameId,
    pub validity_mask: u32,
    pub model_identity: u32,
    pub estimate_identity: u32,
    pub checksum: u32,
    pub age_releases: u32,
    pub active: GlobalDisplayResolvedPoseV1,
    pub ecef: GlobalDisplayResolvedPoseV1,
    pub gcrf: GlobalDisplayResolvedPoseV1,
    pub launch_enu: GlobalDisplayResolvedPoseV1,
    pub recovery_enu: GlobalDisplayResolvedPoseV1,
    pub angular_rate_q24: [i32; 3],
}
impl GlobalDisplaySourcePoseV1 {
    pub fn validate(self, role: PresentationRole) -> Result<(), Kps1Error> {
        if self.source == GlobalDisplaySourceId::SimTruth && !role.permits_private_truth() {
            return Err(Kps1Error::Enum);
        }
        if self.model_identity == 0
            || self.validity_mask == 0
            || self.validity_mask & !GLOBAL_POSE_VALID_MASK != 0
            || self.validity_mask & GLOBAL_POSE_VALID_ACTIVE_POSITION == 0
        {
            return Err(Kps1Error::Identity);
        }
        if self.validity_mask & GLOBAL_POSE_VALID_ACTIVE_ATTITUDE != 0
            && self.active.attitude_q30 == [0; 4]
        {
            return Err(Kps1Error::Identity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalDisplaySampleV1 {
    pub sequence: u64,
    pub release_epoch: u32,
    pub mission_time_q16: u32,
    pub active_frame: GlobalDisplayFrameId,
    pub segment: GlobalDisplaySegment,
    pub flight_mode: u8,
    pub transition_count: u8,
    pub event_mask: u16,
    pub discontinuity_mask: u32,
    pub continuity_identity: u32,
    pub geodetic_q28_q12: [i32; 3],
    pub altitude_q12_km: i32,
    pub mach_q24: i32,
    pub dynamic_pressure_q14_pa: i32,
    pub total_mass_q21_kg: i32,
    pub main_propellant_q21_kg: i32,
    pub rcs_propellant_q21_kg: i32,
    pub gimbal_q15: [i16; 2],
    pub rcs_pulses: [u8; 12],
    pub command_flags: u8,
    pub command_discrete: u8,
    pub alarms: u16,
    pub sources: Vec<GlobalDisplaySourcePoseV1>,
}
impl GlobalDisplaySampleV1 {
    pub fn filter_for_role(mut self, role: PresentationRole) -> Self {
        if !role.permits_private_truth() {
            self.sources
                .retain(|p| p.source != GlobalDisplaySourceId::SimTruth)
        }
        self
    }
    pub const fn requires_exact_snap(&self) -> bool {
        self.discontinuity_mask != 0
    }
    pub fn validate(&self, role: PresentationRole) -> Result<(), Kps1Error> {
        if self.sequence == 0
            || self.continuity_identity == 0
            || self.flight_mode > 7
            || self.transition_count > 4
            || self.discontinuity_mask & !GLOBAL_DISCONTINUITY_MASK != 0
            || self.sources.is_empty()
            || self.sources.len() > GLOBAL_DISPLAY_MAX_SOURCES
        {
            return Err(Kps1Error::Identity);
        }
        let mut seen = 0;
        for p in &self.sources {
            p.validate(role)?;
            if seen & p.source.mask() != 0 {
                return Err(Kps1Error::Identity);
            }
            seen |= p.source.mask();
        }
        Ok(())
    }
    pub fn interpolation_compatible(&self, next: &Self) -> bool {
        self.release_epoch < next.release_epoch
            && self.active_frame == next.active_frame
            && self.segment == next.segment
            && self.continuity_identity == next.continuity_identity
            && self.discontinuity_mask == 0
            && next.discontinuity_mask == 0
            && self.sources.len() == next.sources.len()
            && self.sources.iter().zip(&next.sources).all(|(a, b)| {
                a.source == b.source
                    && a.model_identity == b.model_identity
                    && a.validity_mask == b.validity_mask
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplayPathPointV1 {
    pub release_epoch: u32,
    pub mission_time_q16: u32,
    pub segment: GlobalDisplaySegment,
    pub event_mask: u16,
    pub position_q12_km: [i32; 3],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalDisplayPathChunkV1 {
    pub path_identity: u32,
    pub source: GlobalDisplaySourceId,
    pub display_frame: GlobalDisplayFrameId,
    pub lod: GlobalDisplayPathLod,
    pub flags: u16,
    pub model_identity: u32,
    pub estimate_identity: u32,
    pub source_checksum: u32,
    pub continuity_identity: u32,
    pub chunk_index: u16,
    pub chunk_count: u16,
    pub points: Vec<GlobalDisplayPathPointV1>,
}
impl GlobalDisplayPathChunkV1 {
    pub fn validate(&self, role: PresentationRole) -> Result<(), Kps1Error> {
        if self.source == GlobalDisplaySourceId::SimTruth && !role.permits_private_truth() {
            return Err(Kps1Error::Enum);
        }
        if self.path_identity == 0
            || self.model_identity == 0
            || self.continuity_identity == 0
            || self.chunk_count == 0
            || self.chunk_index >= self.chunk_count
            || self.flags & !GLOBAL_PATH_FLAG_MASK != 0
            || self.points.is_empty()
            || self.points.len() > GLOBAL_DISPLAY_MAX_PATH_POINTS
        {
            return Err(Kps1Error::Identity);
        }
        let mut prev = None;
        for p in &self.points {
            if prev.is_some_and(|(e, t)| p.release_epoch <= e || p.mission_time_q16 <= t) {
                return Err(Kps1Error::Sequence);
            }
            prev = Some((p.release_epoch, p.mission_time_q16));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplayTransitionV1 {
    pub release_epoch: u32,
    pub mission_time_q16: u32,
    pub from_frame: GlobalDisplayFrameId,
    pub to_frame: GlobalDisplayFrameId,
    pub from_segment: GlobalDisplaySegment,
    pub to_segment: GlobalDisplaySegment,
    pub reason: u8,
    pub transition_identity: u32,
    pub transform_identity: u32,
    pub anchor_identity: u32,
    pub position_delta_q12_km: [i32; 3],
    pub velocity_delta_q24_km_s: [i32; 3],
    pub attitude_delta_q30: i32,
    pub angular_rate_delta_q24: [i32; 3],
    pub checksum: u32,
}
impl GlobalDisplayTransitionV1 {
    pub fn validate(self) -> Result<(), Kps1Error> {
        if self.release_epoch == 0
            || self.from_frame == self.to_frame
            || self.from_segment == self.to_segment
            || self.reason == 0
            || self.transition_identity == 0
            || self.transform_identity == 0
            || self.checksum == 0
        {
            return Err(Kps1Error::Identity);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDisplayReplayEntryV1 {
    pub release_epoch: u32,
    pub mission_time_q16: u32,
    pub kind: GlobalDisplayReplayEntryKind,
    pub source_identity: u32,
    pub event_identity: u32,
    pub detail_identity: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalReplayIndexV1 {
    pub index_identity: u32,
    pub session_definition_identity: u32,
    pub first_release: u32,
    pub last_release: u32,
    pub terminal_disposition: u8,
    pub disposition_axes: [u8; 6],
    pub entries: Vec<GlobalDisplayReplayEntryV1>,
}
impl GlobalReplayIndexV1 {
    pub fn validate(&self) -> Result<(), Kps1Error> {
        if self.index_identity == 0
            || self.session_definition_identity == 0
            || self.first_release > self.last_release
            || self.terminal_disposition > 5
            || self.entries.len() > GLOBAL_DISPLAY_MAX_REPLAY_ENTRIES
        {
            return Err(Kps1Error::Identity);
        }
        let mut p = None;
        for e in &self.entries {
            if e.release_epoch < self.first_release
                || e.release_epoch > self.last_release
                || e.event_identity == 0
                || p.is_some_and(|(r, t)| e.release_epoch < r || e.mission_time_q16 < t)
            {
                return Err(Kps1Error::Sequence);
            }
            p = Some((e.release_epoch, e.mission_time_q16));
        }
        Ok(())
    }
    pub fn next_entry(&self, e: u32) -> Option<GlobalDisplayReplayEntryV1> {
        self.entries.iter().copied().find(|v| v.release_epoch > e)
    }
    pub fn previous_entry(&self, e: u32) -> Option<GlobalDisplayReplayEntryV1> {
        self.entries
            .iter()
            .rev()
            .copied()
            .find(|v| v.release_epoch < e)
    }
}

pub fn encode_global_display_definition_payload(
    v: GlobalDisplayDefinitionV1,
    role: PresentationRole,
) -> Result<Vec<u8>, Kps1Error> {
    v.validate(role)?;
    let mut w = Writer::new(*b"PGD1");
    w.u32(v.display_identity);
    w.u32(v.earth_identity);
    w.u32(v.transform_identity);
    w.u32(v.mission_identity);
    w.i32(v.epoch_unix_day);
    w.i16(v.epoch_tai_minus_utc);
    w.zeros(2);
    w.i32(v.semi_major_q12_km);
    w.i32(v.semi_minor_q12_km);
    w.i32(v.inverse_flattening_q20);
    write_anchor(&mut w, v.launch_anchor);
    write_anchor(&mut w, v.recovery_anchor);
    w.u32(v.available_source_mask);
    w.u8(v.available_frame_mask);
    w.zeros(1);
    w.u16(v.camera_domain_mask);
    w.finish()
}
pub fn decode_global_display_definition_payload(
    input: &[u8],
    role: PresentationRole,
) -> Result<GlobalDisplayDefinitionV1, Kps1Error> {
    let mut r = Reader::new(input, *b"PGD1")?;
    let display_identity = r.u32()?;
    let earth_identity = r.u32()?;
    let transform_identity = r.u32()?;
    let mission_identity = r.u32()?;
    let epoch_unix_day = r.i32()?;
    let epoch_tai_minus_utc = r.i16()?;
    r.reserved(2)?;
    let v = GlobalDisplayDefinitionV1 {
        display_identity,
        earth_identity,
        transform_identity,
        mission_identity,
        epoch_unix_day,
        epoch_tai_minus_utc,
        semi_major_q12_km: r.i32()?,
        semi_minor_q12_km: r.i32()?,
        inverse_flattening_q20: r.i32()?,
        launch_anchor: read_anchor(&mut r)?,
        recovery_anchor: read_anchor(&mut r)?,
        available_source_mask: r.u32()?,
        available_frame_mask: r.u8()?,
        camera_domain_mask: {
            r.reserved(1)?;
            r.u16()?
        },
    };
    r.finish()?;
    v.validate(role)?;
    Ok(v)
}

pub fn encode_global_display_samples_payload(
    v: &[GlobalDisplaySampleV1],
    role: PresentationRole,
) -> Result<Vec<u8>, Kps1Error> {
    if v.is_empty() || v.len() > u16::MAX as usize {
        return Err(Kps1Error::Length);
    }
    let mut w = Writer::new(*b"PGS1");
    w.u16(v.len() as u16);
    w.zeros(2);
    let mut p = None;
    for s in v {
        s.validate(role)?;
        if p.is_some_and(|x| s.sequence <= x) {
            return Err(Kps1Error::Sequence);
        }
        p = Some(s.sequence);
        write_sample(&mut w, s);
    }
    w.finish()
}
pub fn decode_global_display_samples_payload(
    input: &[u8],
    role: PresentationRole,
) -> Result<Vec<GlobalDisplaySampleV1>, Kps1Error> {
    let mut r = Reader::new(input, *b"PGS1")?;
    let n = r.u16()? as usize;
    r.reserved(2)?;
    if n == 0 {
        return Err(Kps1Error::Length);
    }
    let mut out = Vec::with_capacity(n);
    let mut p = None;
    for _ in 0..n {
        let s = read_sample(&mut r)?;
        s.validate(role)?;
        if p.is_some_and(|x| s.sequence <= x) {
            return Err(Kps1Error::Sequence);
        }
        p = Some(s.sequence);
        out.push(s)
    }
    r.finish()?;
    Ok(out)
}

pub fn encode_global_display_path_payload(
    v: &GlobalDisplayPathChunkV1,
    role: PresentationRole,
) -> Result<Vec<u8>, Kps1Error> {
    v.validate(role)?;
    let mut w = Writer::new(*b"PGP1");
    w.u32(v.path_identity);
    w.u8(v.source as u8);
    w.u8(v.display_frame as u8);
    w.u8(v.lod as u8);
    w.zeros(1);
    w.u16(v.flags);
    w.u16(v.chunk_index);
    w.u16(v.chunk_count);
    w.u16(v.points.len() as u16);
    w.u32(v.model_identity);
    w.u32(v.estimate_identity);
    w.u32(v.source_checksum);
    w.u32(v.continuity_identity);
    for p in &v.points {
        w.u32(p.release_epoch);
        w.u32(p.mission_time_q16);
        w.u8(p.segment as u8);
        w.zeros(1);
        w.u16(p.event_mask);
        w.i32s(&p.position_q12_km)
    }
    w.finish()
}
pub fn decode_global_display_path_payload(
    input: &[u8],
    role: PresentationRole,
) -> Result<GlobalDisplayPathChunkV1, Kps1Error> {
    let mut r = Reader::new(input, *b"PGP1")?;
    let path_identity = r.u32()?;
    let source = GlobalDisplaySourceId::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let display_frame = GlobalDisplayFrameId::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let lod = GlobalDisplayPathLod::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    r.reserved(1)?;
    let flags = r.u16()?;
    let chunk_index = r.u16()?;
    let chunk_count = r.u16()?;
    let n = r.u16()? as usize;
    if n > GLOBAL_DISPLAY_MAX_PATH_POINTS {
        return Err(Kps1Error::PayloadTooLarge);
    }
    let model_identity = r.u32()?;
    let estimate_identity = r.u32()?;
    let source_checksum = r.u32()?;
    let continuity_identity = r.u32()?;
    let mut points = Vec::with_capacity(n);
    for _ in 0..n {
        let release_epoch = r.u32()?;
        let mission_time_q16 = r.u32()?;
        let segment = GlobalDisplaySegment::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
        r.reserved(1)?;
        let event_mask = r.u16()?;
        points.push(GlobalDisplayPathPointV1 {
            release_epoch,
            mission_time_q16,
            segment,
            event_mask,
            position_q12_km: r.i32s()?,
        })
    }
    r.finish()?;
    let v = GlobalDisplayPathChunkV1 {
        path_identity,
        source,
        display_frame,
        lod,
        flags,
        model_identity,
        estimate_identity,
        source_checksum,
        continuity_identity,
        chunk_index,
        chunk_count,
        points,
    };
    v.validate(role)?;
    Ok(v)
}

pub fn encode_global_display_transition_payload(
    v: GlobalDisplayTransitionV1,
) -> Result<Vec<u8>, Kps1Error> {
    v.validate()?;
    let mut w = Writer::new(*b"PGT1");
    w.u32(v.release_epoch);
    w.u32(v.mission_time_q16);
    w.u8(v.from_frame as u8);
    w.u8(v.to_frame as u8);
    w.u8(v.from_segment as u8);
    w.u8(v.to_segment as u8);
    w.u8(v.reason);
    w.zeros(3);
    w.u32(v.transition_identity);
    w.u32(v.transform_identity);
    w.u32(v.anchor_identity);
    w.i32s(&v.position_delta_q12_km);
    w.i32s(&v.velocity_delta_q24_km_s);
    w.i32(v.attitude_delta_q30);
    w.i32s(&v.angular_rate_delta_q24);
    w.u32(v.checksum);
    w.finish()
}
pub fn decode_global_display_transition_payload(
    input: &[u8],
) -> Result<GlobalDisplayTransitionV1, Kps1Error> {
    let mut r = Reader::new(input, *b"PGT1")?;
    let release_epoch = r.u32()?;
    let mission_time_q16 = r.u32()?;
    let from_frame = GlobalDisplayFrameId::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let to_frame = GlobalDisplayFrameId::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let from_segment = GlobalDisplaySegment::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let to_segment = GlobalDisplaySegment::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let reason = r.u8()?;
    r.reserved(3)?;
    let v = GlobalDisplayTransitionV1 {
        release_epoch,
        mission_time_q16,
        from_frame,
        to_frame,
        from_segment,
        to_segment,
        reason,
        transition_identity: r.u32()?,
        transform_identity: r.u32()?,
        anchor_identity: r.u32()?,
        position_delta_q12_km: r.i32s()?,
        velocity_delta_q24_km_s: r.i32s()?,
        attitude_delta_q30: r.i32()?,
        angular_rate_delta_q24: r.i32s()?,
        checksum: r.u32()?,
    };
    r.finish()?;
    v.validate()?;
    Ok(v)
}

pub fn encode_global_replay_index_payload(v: &GlobalReplayIndexV1) -> Result<Vec<u8>, Kps1Error> {
    v.validate()?;
    let mut w = Writer::new(*b"PGI1");
    w.u32(v.index_identity);
    w.u32(v.session_definition_identity);
    w.u32(v.first_release);
    w.u32(v.last_release);
    w.u8(v.terminal_disposition);
    w.bytes(&v.disposition_axes);
    w.zeros(1);
    w.u16(v.entries.len() as u16);
    w.zeros(2);
    for e in &v.entries {
        w.u32(e.release_epoch);
        w.u32(e.mission_time_q16);
        w.u8(e.kind as u8);
        w.zeros(3);
        w.u32(e.source_identity);
        w.u32(e.event_identity);
        w.u32(e.detail_identity)
    }
    w.finish()
}
pub fn decode_global_replay_index_payload(input: &[u8]) -> Result<GlobalReplayIndexV1, Kps1Error> {
    let mut r = Reader::new(input, *b"PGI1")?;
    let index_identity = r.u32()?;
    let session_definition_identity = r.u32()?;
    let first_release = r.u32()?;
    let last_release = r.u32()?;
    let terminal_disposition = r.u8()?;
    let mut disposition_axes = [0; 6];
    disposition_axes.copy_from_slice(r.take(6)?);
    r.reserved(1)?;
    let n = r.u16()? as usize;
    r.reserved(2)?;
    if n > GLOBAL_DISPLAY_MAX_REPLAY_ENTRIES {
        return Err(Kps1Error::PayloadTooLarge);
    }
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let release_epoch = r.u32()?;
        let mission_time_q16 = r.u32()?;
        let kind = GlobalDisplayReplayEntryKind::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
        r.reserved(3)?;
        entries.push(GlobalDisplayReplayEntryV1 {
            release_epoch,
            mission_time_q16,
            kind,
            source_identity: r.u32()?,
            event_identity: r.u32()?,
            detail_identity: r.u32()?,
        })
    }
    r.finish()?;
    let v = GlobalReplayIndexV1 {
        index_identity,
        session_definition_identity,
        first_release,
        last_release,
        terminal_disposition,
        disposition_axes,
        entries,
    };
    v.validate()?;
    Ok(v)
}

fn write_anchor(w: &mut Writer, v: GlobalDisplayAnchorV1) {
    w.u32(v.identity);
    w.i32s(&v.geodetic_q28_q12)
}
fn read_anchor(r: &mut Reader<'_>) -> Result<GlobalDisplayAnchorV1, Kps1Error> {
    Ok(GlobalDisplayAnchorV1 {
        identity: r.u32()?,
        geodetic_q28_q12: r.i32s()?,
    })
}
fn write_pose(w: &mut Writer, v: GlobalDisplayResolvedPoseV1) {
    w.i32s(&v.position_q12_km);
    w.i32s(&v.velocity_q24_km_s);
    w.i32s(&v.attitude_q30)
}
fn read_pose(r: &mut Reader<'_>) -> Result<GlobalDisplayResolvedPoseV1, Kps1Error> {
    Ok(GlobalDisplayResolvedPoseV1 {
        position_q12_km: r.i32s()?,
        velocity_q24_km_s: r.i32s()?,
        attitude_q30: r.i32s()?,
    })
}
fn write_source(w: &mut Writer, v: GlobalDisplaySourcePoseV1) {
    w.u8(v.source as u8);
    w.u8(v.active_frame as u8);
    w.zeros(2);
    w.u32(v.validity_mask);
    w.u32(v.model_identity);
    w.u32(v.estimate_identity);
    w.u32(v.checksum);
    w.u32(v.age_releases);
    write_pose(w, v.active);
    write_pose(w, v.ecef);
    write_pose(w, v.gcrf);
    write_pose(w, v.launch_enu);
    write_pose(w, v.recovery_enu);
    w.i32s(&v.angular_rate_q24)
}
fn read_source(r: &mut Reader<'_>) -> Result<GlobalDisplaySourcePoseV1, Kps1Error> {
    let source = GlobalDisplaySourceId::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let active_frame = GlobalDisplayFrameId::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    r.reserved(2)?;
    Ok(GlobalDisplaySourcePoseV1 {
        source,
        active_frame,
        validity_mask: r.u32()?,
        model_identity: r.u32()?,
        estimate_identity: r.u32()?,
        checksum: r.u32()?,
        age_releases: r.u32()?,
        active: read_pose(r)?,
        ecef: read_pose(r)?,
        gcrf: read_pose(r)?,
        launch_enu: read_pose(r)?,
        recovery_enu: read_pose(r)?,
        angular_rate_q24: r.i32s()?,
    })
}
fn write_sample(w: &mut Writer, v: &GlobalDisplaySampleV1) {
    w.u64(v.sequence);
    w.u32(v.release_epoch);
    w.u32(v.mission_time_q16);
    w.u8(v.active_frame as u8);
    w.u8(v.segment as u8);
    w.u8(v.flight_mode);
    w.u8(v.transition_count);
    w.u16(v.event_mask);
    w.u16(v.sources.len() as u16);
    w.u32(v.discontinuity_mask);
    w.u32(v.continuity_identity);
    w.i32s(&v.geodetic_q28_q12);
    w.i32(v.altitude_q12_km);
    w.i32(v.mach_q24);
    w.i32(v.dynamic_pressure_q14_pa);
    w.i32(v.total_mass_q21_kg);
    w.i32(v.main_propellant_q21_kg);
    w.i32(v.rcs_propellant_q21_kg);
    w.i16(v.gimbal_q15[0]);
    w.i16(v.gimbal_q15[1]);
    w.bytes(&v.rcs_pulses);
    w.u8(v.command_flags);
    w.u8(v.command_discrete);
    w.u16(v.alarms);
    for p in &v.sources {
        write_source(w, *p)
    }
}
fn read_sample(r: &mut Reader<'_>) -> Result<GlobalDisplaySampleV1, Kps1Error> {
    let sequence = r.u64()?;
    let release_epoch = r.u32()?;
    let mission_time_q16 = r.u32()?;
    let active_frame = GlobalDisplayFrameId::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let segment = GlobalDisplaySegment::from_raw(r.u8()?).ok_or(Kps1Error::Enum)?;
    let flight_mode = r.u8()?;
    let transition_count = r.u8()?;
    let event_mask = r.u16()?;
    let n = r.u16()? as usize;
    if n == 0 || n > GLOBAL_DISPLAY_MAX_SOURCES {
        return Err(Kps1Error::Length);
    }
    let discontinuity_mask = r.u32()?;
    let continuity_identity = r.u32()?;
    let geodetic_q28_q12 = r.i32s()?;
    let altitude_q12_km = r.i32()?;
    let mach_q24 = r.i32()?;
    let dynamic_pressure_q14_pa = r.i32()?;
    let total_mass_q21_kg = r.i32()?;
    let main_propellant_q21_kg = r.i32()?;
    let rcs_propellant_q21_kg = r.i32()?;
    let gimbal_q15 = [r.i16()?, r.i16()?];
    let mut rcs_pulses = [0; 12];
    rcs_pulses.copy_from_slice(r.take(12)?);
    let command_flags = r.u8()?;
    let command_discrete = r.u8()?;
    let alarms = r.u16()?;
    let mut sources = Vec::with_capacity(n);
    for _ in 0..n {
        sources.push(read_source(r)?)
    }
    Ok(GlobalDisplaySampleV1 {
        sequence,
        release_epoch,
        mission_time_q16,
        active_frame,
        segment,
        flight_mode,
        transition_count,
        event_mask,
        discontinuity_mask,
        continuity_identity,
        geodetic_q28_q12,
        altitude_q12_km,
        mach_q24,
        dynamic_pressure_q14_pa,
        total_mass_q21_kg,
        main_propellant_q21_kg,
        rcs_propellant_q21_kg,
        gimbal_q15,
        rcs_pulses,
        command_flags,
        command_discrete,
        alarms,
        sources,
    })
}

struct Writer {
    bytes: Vec<u8>,
}
impl Writer {
    fn new(m: [u8; 4]) -> Self {
        let mut b = alloc::vec![0;12];
        b[..4].copy_from_slice(&m);
        b[4..6].copy_from_slice(&1u16.to_le_bytes());
        b[6..8].copy_from_slice(&12u16.to_le_bytes());
        Self { bytes: b }
    }
    fn bytes(&mut self, v: &[u8]) {
        self.bytes.extend_from_slice(v)
    }
    fn zeros(&mut self, n: usize) {
        self.bytes.resize(self.bytes.len() + n, 0)
    }
    fn u8(&mut self, v: u8) {
        self.bytes.push(v)
    }
    fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes())
    }
    fn i16(&mut self, v: i16) {
        self.bytes(&v.to_le_bytes())
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes())
    }
    fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes())
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes())
    }
    fn i32s<const N: usize>(&mut self, v: &[i32; N]) {
        for x in v {
            self.i32(*x)
        }
    }
    fn finish(mut self) -> Result<Vec<u8>, Kps1Error> {
        if self.bytes.len() > KPS1_MAX_PAYLOAD_LENGTH {
            return Err(Kps1Error::PayloadTooLarge);
        }
        let n = u32::try_from(self.bytes.len()).map_err(|_| Kps1Error::PayloadTooLarge)?;
        self.bytes[8..12].copy_from_slice(&n.to_le_bytes());
        Ok(self.bytes)
    }
}
struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(input: &'a [u8], m: [u8; 4]) -> Result<Self, Kps1Error> {
        if input.len() < 12 || input.len() > KPS1_MAX_PAYLOAD_LENGTH {
            return Err(Kps1Error::Length);
        }
        if input[..4] != m {
            return Err(Kps1Error::Magic);
        }
        if u16::from_le_bytes([input[4], input[5]]) != 1 {
            return Err(Kps1Error::Version);
        }
        if u16::from_le_bytes([input[6], input[7]]) != 12 {
            return Err(Kps1Error::HeaderLength);
        }
        if u32::from_le_bytes([input[8], input[9], input[10], input[11]]) as usize != input.len() {
            return Err(Kps1Error::Length);
        }
        Ok(Self { input, at: 12 })
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Kps1Error> {
        let e = self.at.checked_add(n).ok_or(Kps1Error::Length)?;
        let v = self.input.get(self.at..e).ok_or(Kps1Error::Length)?;
        self.at = e;
        Ok(v)
    }
    fn reserved(&mut self, n: usize) -> Result<(), Kps1Error> {
        if self.take(n)?.iter().any(|b| *b != 0) {
            return Err(Kps1Error::Reserved);
        }
        Ok(())
    }
    fn u8(&mut self) -> Result<u8, Kps1Error> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Kps1Error> {
        let v = self.take(2)?;
        Ok(u16::from_le_bytes([v[0], v[1]]))
    }
    fn i16(&mut self) -> Result<i16, Kps1Error> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, Kps1Error> {
        let v = self.take(4)?;
        Ok(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }
    fn i32(&mut self) -> Result<i32, Kps1Error> {
        Ok(self.u32()? as i32)
    }
    fn u64(&mut self) -> Result<u64, Kps1Error> {
        let v = self.take(8)?;
        Ok(u64::from_le_bytes([
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7],
        ]))
    }
    fn i32s<const N: usize>(&mut self) -> Result<[i32; N], Kps1Error> {
        let mut o = [0; N];
        for x in &mut o {
            *x = self.i32()?
        }
        Ok(o)
    }
    fn finish(self) -> Result<(), Kps1Error> {
        if self.at != self.input.len() {
            return Err(Kps1Error::Length);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pose(source: GlobalDisplaySourceId) -> GlobalDisplaySourcePoseV1 {
        GlobalDisplaySourcePoseV1 {
            source,
            active_frame: GlobalDisplayFrameId::EarthFixedEcef,
            validity_mask: GLOBAL_POSE_VALID_ACTIVE_POSITION | GLOBAL_POSE_VALID_ACTIVE_ATTITUDE,
            model_identity: 1,
            estimate_identity: 2,
            checksum: 3,
            age_releases: 0,
            active: GlobalDisplayResolvedPoseV1 {
                position_q12_km: [1, 2, 3],
                velocity_q24_km_s: [4, 5, 6],
                attitude_q30: [1 << 30, 0, 0, 0],
            },
            ecef: GlobalDisplayResolvedPoseV1::default(),
            gcrf: GlobalDisplayResolvedPoseV1::default(),
            launch_enu: GlobalDisplayResolvedPoseV1::default(),
            recovery_enu: GlobalDisplayResolvedPoseV1::default(),
            angular_rate_q24: [0; 3],
        }
    }
    fn sample(role: PresentationRole) -> GlobalDisplaySampleV1 {
        let mut sources = alloc::vec![
            pose(GlobalDisplaySourceId::OnboardEstimate),
            pose(GlobalDisplaySourceId::GroundEstimate)
        ];
        if role.permits_private_truth() {
            sources.push(pose(GlobalDisplaySourceId::SimTruth))
        }
        GlobalDisplaySampleV1 {
            sequence: 1,
            release_epoch: 32,
            mission_time_q16: 65_536,
            active_frame: GlobalDisplayFrameId::EarthFixedEcef,
            segment: GlobalDisplaySegment::EcefAscent,
            flight_mode: 2,
            transition_count: 1,
            event_mask: 4,
            discontinuity_mask: 0,
            continuity_identity: 1,
            geodetic_q28_q12: [1, 2, 3],
            altitude_q12_km: 4,
            mach_q24: 5,
            dynamic_pressure_q14_pa: 6,
            total_mass_q21_kg: 7,
            main_propellant_q21_kg: 8,
            rcs_propellant_q21_kg: 9,
            gimbal_q15: [10, -11],
            rcs_pulses: [1; 12],
            command_flags: 12,
            command_discrete: 13,
            alarms: 14,
            sources,
        }
    }
    #[test]
    fn sample_round_trip_and_truth_filter_are_strict() {
        let v = alloc::vec![sample(PresentationRole::SimDirector)];
        let b = encode_global_display_samples_payload(&v, PresentationRole::SimDirector).unwrap();
        assert_eq!(
            decode_global_display_samples_payload(&b, PresentationRole::SimDirector),
            Ok(v.clone())
        );
        assert_eq!(
            encode_global_display_samples_payload(&v, PresentationRole::GuidedOperator),
            Err(Kps1Error::Enum)
        );
        assert_eq!(
            v[0].clone()
                .filter_for_role(PresentationRole::GuidedOperator)
                .sources
                .len(),
            2
        )
    }
    #[test]
    fn every_other_global_payload_round_trips() {
        let role = PresentationRole::Observer;
        let d = GlobalDisplayDefinitionV1 {
            display_identity: GLOBAL_DISPLAY_MODEL_ID,
            earth_identity: 1,
            transform_identity: 2,
            mission_identity: 3,
            epoch_unix_day: 19_723,
            epoch_tai_minus_utc: 37,
            semi_major_q12_km: 26_124_165,
            semi_minor_q12_km: 26_036_734,
            inverse_flattening_q20: 313_883_719,
            launch_anchor: GlobalDisplayAnchorV1 {
                identity: 4,
                geodetic_q28_q12: [5, 6, 7],
            },
            recovery_anchor: GlobalDisplayAnchorV1 {
                identity: 8,
                geodetic_q28_q12: [9, 10, 11],
            },
            available_source_mask: GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK,
            available_frame_mask: 7,
            camera_domain_mask: 0xff,
        };
        let b = encode_global_display_definition_payload(d, role).unwrap();
        assert_eq!(decode_global_display_definition_payload(&b, role), Ok(d));
        let typed = crate::PresentationPayload::GlobalDisplayDefinition(d);
        let typed_bytes = crate::encode_typed_payload(&typed, role).unwrap();
        assert_eq!(
            crate::decode_typed_payload(typed.kind(), &typed_bytes, role),
            Ok(typed)
        );
        let p = GlobalDisplayPathChunkV1 {
            path_identity: 1,
            source: GlobalDisplaySourceId::GroundEstimate,
            display_frame: GlobalDisplayFrameId::EarthFixedEcef,
            lod: GlobalDisplayPathLod::OneSecond,
            flags: GLOBAL_PATH_FLAG_TERMINAL,
            model_identity: 2,
            estimate_identity: 3,
            source_checksum: 4,
            continuity_identity: 5,
            chunk_index: 0,
            chunk_count: 1,
            points: alloc::vec![GlobalDisplayPathPointV1 {
                release_epoch: 32,
                mission_time_q16: 65_536,
                segment: GlobalDisplaySegment::EcefAscent,
                event_mask: 0,
                position_q12_km: [1, 2, 3]
            }],
        };
        let b = encode_global_display_path_payload(&p, role).unwrap();
        assert_eq!(decode_global_display_path_payload(&b, role), Ok(p));
        let t = GlobalDisplayTransitionV1 {
            release_epoch: 29,
            mission_time_q16: 59_392,
            from_frame: GlobalDisplayFrameId::LocalEnu,
            to_frame: GlobalDisplayFrameId::EarthFixedEcef,
            from_segment: GlobalDisplaySegment::LocalLaunch,
            to_segment: GlobalDisplaySegment::EcefAscent,
            reason: 1,
            transition_identity: 1,
            transform_identity: 2,
            anchor_identity: 3,
            position_delta_q12_km: [0; 3],
            velocity_delta_q24_km_s: [0; 3],
            attitude_delta_q30: 1,
            angular_rate_delta_q24: [0; 3],
            checksum: 4,
        };
        let b = encode_global_display_transition_payload(t).unwrap();
        assert_eq!(decode_global_display_transition_payload(&b), Ok(t));
        let i = GlobalReplayIndexV1 {
            index_identity: 1,
            session_definition_identity: 2,
            first_release: 0,
            last_release: 100,
            terminal_disposition: 2,
            disposition_axes: [1, 2, 3, 4, 1, 1],
            entries: alloc::vec![GlobalDisplayReplayEntryV1 {
                release_epoch: 29,
                mission_time_q16: 59_392,
                kind: GlobalDisplayReplayEntryKind::FrameTransition,
                source_identity: 3,
                event_identity: 4,
                detail_identity: 5
            }],
        };
        let b = encode_global_replay_index_payload(&i).unwrap();
        assert_eq!(decode_global_replay_index_payload(&b), Ok(i));
    }
    #[test]
    fn interpolation_requires_one_compatible_continuity_domain() {
        let a = sample(PresentationRole::GuidedOperator);
        let mut b = a.clone();
        b.sequence = 2;
        b.release_epoch += 1;
        b.mission_time_q16 += 2_048;
        assert!(a.interpolation_compatible(&b));
        b.discontinuity_mask = GLOBAL_DISCONTINUITY_FRAME;
        assert!(!a.interpolation_compatible(&b));
    }
}
