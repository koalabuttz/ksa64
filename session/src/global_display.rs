//! Rust-owned Phase 12C global display publisher for live mission sessions.
//!
//! This derives noncanonical, role-filtered display products from accepted
//! Phase 10 telemetry. It cannot advance or mutate mission authority.

use crate::global_fixtures::GlobalFixtureSet;
use crate::phase10_mission::{mission_update, PHASE10_NOMINAL_CASE_SEED, PHASE10_NOMINAL_SESSION};
use crate::phase10_nominal_compat::{
    audit_phase10_nominal_lineages, Phase10NominalCompatibilityAudit,
    Phase10NominalCompatibilityError, FROZEN_NOMINAL_KPH10, FROZEN_NOMINAL_KPH10_SHA256_HEX,
    FROZEN_NOMINAL_KSR10, FROZEN_NOMINAL_KSR10_SHA256_HEX, FROZEN_NOMINAL_KTT10,
    FROZEN_NOMINAL_KTT10_SHA256_HEX,
};
use ksa64_core::phase10_contract::{GlobalSegment, ReferenceFrameId};
use ksa64_core::phase10_environment::ecef_to_geodetic;
use ksa64_core::phase10_frames::{
    ecef_to_gcrf, ecef_to_local, gcrf_to_ecef, interpolate_transform, local_to_ecef, LocalAnchor,
    LocalKinematicState,
};
use ksa64_core::phase10_geodesy::{enu_to_ecef_rotation, geodetic_to_ecef};
use ksa64_core::phase10_numeric::{
    GlobalAngularRateVec, GlobalKinematicState, GlobalPositionVec, GlobalVelocityVec,
    MissionTimeQ16,
};
use ksa64_core::phase10_telemetry::{
    GlobalEvaluationSummary, GlobalPlotHeader, GlobalPlotPoint, GlobalTelemetryFrame,
    GlobalTelemetryHeader, KPH10_HEADER_LENGTH, KPH10_POINT_LENGTH, KSR10_LENGTH,
    KTT10_FRAME_LENGTH, KTT10_HEADER_LENGTH,
};
use ksa64_core::phase8_numeric::{EnuPosition, EnuVelocity};
use ksa64_core::spatial_numeric::QuaternionQ30;
use ksa64_flight::phase11::KSA_G10R_REFERENCE_OPS_MANIFEST_ID;
use ksa64_interface::phase10::GlobalFrameId;
use ksa64_interface::phase11::GroundEstimate;
use ksa64_presentation::{
    GlobalDisplayAnchorV1, GlobalDisplayDefinitionV1, GlobalDisplayFrameId,
    GlobalDisplayPathChunkV1, GlobalDisplayPathLod, GlobalDisplayPathPointV1,
    GlobalDisplayReplayEntryKind, GlobalDisplayReplayEntryV1, GlobalDisplayResolvedPoseV1,
    GlobalDisplaySampleV1, GlobalDisplaySegment, GlobalDisplaySourceId, GlobalDisplaySourcePoseV1,
    GlobalDisplayTransitionV1, GlobalReplayIndexV1, PresentationRole,
    GLOBAL_DISCONTINUITY_ATTITUDE_RETIRED, GLOBAL_DISCONTINUITY_DEPLOYMENT,
    GLOBAL_DISCONTINUITY_FRAME, GLOBAL_DISCONTINUITY_HISTORY_GAP,
    GLOBAL_DISCONTINUITY_NAVIGATION_RESET, GLOBAL_DISCONTINUITY_SEGMENT,
    GLOBAL_DISCONTINUITY_SOURCE_REPLACED, GLOBAL_DISCONTINUITY_TERMINAL,
    GLOBAL_DISPLAY_MAX_REPLAY_ENTRIES, GLOBAL_DISPLAY_MODEL_ID, GLOBAL_DISPLAY_SOURCE_MASK,
    GLOBAL_DISPLAY_SOURCE_ONBOARD, GLOBAL_DISPLAY_SOURCE_PLANNED, GLOBAL_DISPLAY_SOURCE_SIM_TRUTH,
    GLOBAL_PATH_FLAG_INCOMPLETE, GLOBAL_PATH_FLAG_RESYNC_REQUIRED, GLOBAL_PATH_FLAG_STALE,
    GLOBAL_PATH_FLAG_TERMINAL, GLOBAL_POSE_VALID_ACTIVE_ATTITUDE,
    GLOBAL_POSE_VALID_ACTIVE_POSITION, GLOBAL_POSE_VALID_ACTIVE_VELOCITY,
    GLOBAL_POSE_VALID_ANGULAR_RATE, GLOBAL_POSE_VALID_ECEF_ATTITUDE,
    GLOBAL_POSE_VALID_ECEF_POSITION, GLOBAL_POSE_VALID_ECEF_VELOCITY,
    GLOBAL_POSE_VALID_GCRF_ATTITUDE, GLOBAL_POSE_VALID_GCRF_POSITION,
    GLOBAL_POSE_VALID_GCRF_VELOCITY, GLOBAL_POSE_VALID_LAUNCH_ENU_ATTITUDE,
    GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION, GLOBAL_POSE_VALID_LAUNCH_ENU_VELOCITY,
    GLOBAL_POSE_VALID_RECOVERY_ENU_ATTITUDE, GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION,
    GLOBAL_POSE_VALID_RECOVERY_ENU_VELOCITY,
};
use ksa64_sim::phase10::{
    FrameTransitionRecord, GlobalWorldError, EVENT_DROGUE, EVENT_LANDING, EVENT_MAIN,
};
use ksa64_sim::phase10_avionics::{
    reference_global_flight_config, GlobalAvionicsMission, GlobalSensorFaults,
};

pub const GLOBAL_DISPLAY_CAMERA_DOMAIN_MASK: u16 = 0x00ff;
pub const GLOBAL_DISPLAY_PATH_POINT_LIMIT: usize = 1_024;
pub const ACCEPTED_NOMINAL_REFERENCE_MODEL_ID: u32 = 0x12b5_0001;
/// Current portable Phase 10 implementation, re-executed after the frozen
/// Phase 10 artifact lineage has independently passed strict validation.
pub const CURRENT_NOMINAL_REEXECUTION_MODEL_ID: u32 = 0x12c0_1001;
/// Compact display identity derived from the reviewed current KTT10 SHA-256.
pub const CURRENT_NOMINAL_REEXECUTION_EVIDENCE_ID: u32 = 0x94e8_8790;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalDisplayPublishError {
    Fixture,
    TelemetryArtifact {
        first_diff: usize,
        generated_len: usize,
        accepted_len: usize,
        frame_step: u32,
        generated_checksum: u32,
        accepted_checksum: u32,
    },
    PlotArtifact,
    SummaryArtifact,
    ExactReleaseCount,
    Path,
    NominalCompatibility(Phase10NominalCompatibilityError),
    World,
}

impl From<Phase10NominalCompatibilityError> for GlobalDisplayPublishError {
    fn from(value: Phase10NominalCompatibilityError) -> Self {
        Self::NominalCompatibility(value)
    }
}

impl From<GlobalWorldError> for GlobalDisplayPublishError {
    fn from(_: GlobalWorldError) -> Self {
        Self::World
    }
}

/// Builds one renderer-neutral path chunk from normalized display samples.
///
/// This is shared by the in-process session publisher and the native viewer
/// bridge so transport placement cannot change path retention, flags, lineage,
/// or chunking. `replay_entries` and `pinned_releases` must contain only
/// semantically meaningful bookmarks; routine release notifications are not
/// path pins.
#[allow(clippy::too_many_arguments)]
pub fn build_global_display_path_chunk(
    role: PresentationRole,
    display_identity: u32,
    launch_anchor_identity: u32,
    recovery_anchor_identity: u32,
    path_samples: &[GlobalDisplaySampleV1],
    replay_entries: &[GlobalDisplayReplayEntryV1],
    source: GlobalDisplaySourceId,
    display_frame: GlobalDisplayFrameId,
    lod: GlobalDisplayPathLod,
    chunk_index: u16,
    pinned_releases: &[u32],
) -> Result<GlobalDisplayPathChunkV1, GlobalDisplayPublishError> {
    if source == GlobalDisplaySourceId::SimTruth && !role.permits_private_truth() {
        return Err(GlobalDisplayPublishError::Path);
    }
    if source == GlobalDisplaySourceId::Planned && lod == GlobalDisplayPathLod::Exact {
        return Err(GlobalDisplayPublishError::Path);
    }
    let mut points = Vec::new();
    for (sample_index, sample) in path_samples.iter().enumerate() {
        let pinned = sample.event_mask != 0
            || sample.discontinuity_mask != 0
            || pinned_releases.contains(&sample.release_epoch)
            || replay_entries
                .iter()
                .any(|entry| entry.release_epoch == sample.release_epoch);
        // Frozen planned telemetry is already a sparse stream whose one-based
        // sequence is the accepted release step. Live samples are exact and
        // use their zero-based release epoch. Preserve the first planned point
        // explicitly, then apply the same 32/128-release interval contract.
        let on_cadence = if source == GlobalDisplaySourceId::Planned {
            sample_index == 0
                || sample
                    .sequence
                    .is_multiple_of(u64::from(lod.cadence_releases()))
        } else {
            sample.release_epoch.is_multiple_of(lod.cadence_releases())
        };
        let retain = on_cadence || pinned;
        if !retain {
            continue;
        }
        let Some(pose) = sample.sources.iter().find(|pose| pose.source == source) else {
            continue;
        };
        let Some(position) = resolved_position(pose, display_frame, sample.segment) else {
            continue;
        };
        points.push(GlobalDisplayPathPointV1 {
            release_epoch: sample.release_epoch,
            mission_time_q16: sample.mission_time_q16,
            segment: sample.segment,
            event_mask: sample.event_mask,
            anchor_identity: path_anchor_identity(
                display_frame,
                sample.segment,
                launch_anchor_identity,
                recovery_anchor_identity,
            ),
            position_q12_km: position,
        });
    }
    if points.is_empty() {
        return Err(GlobalDisplayPublishError::Path);
    }
    let chunks = points.len().div_ceil(GLOBAL_DISPLAY_PATH_POINT_LIMIT);
    let chunk_count = u16::try_from(chunks).map_err(|_| GlobalDisplayPublishError::Path)?;
    if chunk_index >= chunk_count {
        return Err(GlobalDisplayPublishError::Path);
    }
    let start = usize::from(chunk_index) * GLOBAL_DISPLAY_PATH_POINT_LIMIT;
    let end = (start + GLOBAL_DISPLAY_PATH_POINT_LIMIT).min(points.len());
    let sample = path_samples.last().ok_or(GlobalDisplayPublishError::Path)?;
    let latest_lineage_pose = sample.sources.iter().find(|pose| pose.source == source);
    let lineage_pose = path_samples
        .iter()
        .rev()
        .find_map(|sample| sample.sources.iter().find(|pose| pose.source == source));
    let terminal = source == GlobalDisplaySourceId::Planned
        || sample.discontinuity_mask & GLOBAL_DISCONTINUITY_TERMINAL != 0;
    let stale = latest_lineage_pose.is_none()
        || latest_lineage_pose.is_some_and(|pose| pose.age_releases > 32);
    // A source may legitimately become valid after mission start (for
    // example, delayed ground tracking). Compare against the first resolvable
    // sample for this source and frame rather than the first multi-source
    // authority sample; a later first retained point still exposes real loss.
    let first_source_release = path_samples.iter().find_map(|sample| {
        let pose = sample.sources.iter().find(|pose| pose.source == source)?;
        resolved_position(pose, display_frame, sample.segment).map(|_| sample.release_epoch)
    });
    let incomplete = !terminal
        || latest_lineage_pose.is_none()
        || first_source_release.is_none()
        || points.first().is_none_or(|point| {
            point.release_epoch > first_source_release.unwrap_or(point.release_epoch)
        });
    let resync_required = path_samples
        .iter()
        .any(|sample| sample.discontinuity_mask & GLOBAL_DISCONTINUITY_HISTORY_GAP != 0);
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
    if resync_required {
        flags |= GLOBAL_PATH_FLAG_RESYNC_REQUIRED;
    }
    let path_identity = hash_words(&[
        display_identity,
        source as u32,
        display_frame as u32,
        lod as u32,
    ]);
    Ok(GlobalDisplayPathChunkV1 {
        path_identity,
        source,
        display_frame,
        lod,
        flags,
        model_identity: lineage_pose.map_or(GLOBAL_DISPLAY_MODEL_ID ^ source as u32, |pose| {
            pose.model_identity
        }),
        estimate_identity: lineage_pose.map_or(0, |pose| pose.estimate_identity),
        source_checksum: lineage_pose.map_or(0, |pose| pose.checksum),
        continuity_identity: path_identity,
        chunk_index,
        chunk_count,
        points: points[start..end].to_vec(),
    })
}

/// Presentation-only state retained by the authority but excluded from every
/// canonical K record and checksum chain.
pub struct GlobalDisplayPublisher {
    definition: GlobalDisplayDefinitionV1,
    launch_anchor: LocalAnchor,
    recovery_anchor: LocalAnchor,
    samples: Vec<GlobalDisplaySampleV1>,
    planned_samples: Vec<GlobalDisplaySampleV1>,
    transitions: Vec<GlobalDisplayTransitionV1>,
    replay_entries: Vec<GlobalDisplayReplayEntryV1>,
    previous_frame: Option<GlobalDisplayFrameId>,
    previous_segment: Option<GlobalDisplaySegment>,
    previous_transition_count: u8,
    source_lineages: [Option<(u32, u32)>; 4],
    attitude_retired: bool,
    pending_discontinuity_mask: u32,
}

impl GlobalDisplayPublisher {
    pub fn new(fixtures: &GlobalFixtureSet) -> Result<Self, GlobalDisplayPublishError> {
        let mission = fixtures.mission;
        let (launch, launch_anchor) = display_anchor(
            mission.identity ^ 0x4c41_554e,
            [
                mission.launch_latitude_q28_rad,
                mission.launch_longitude_q28_rad,
                mission.launch_height_q12_km,
            ],
        )?;
        let (recovery, recovery_anchor) = display_anchor(
            mission.identity ^ 0x5245_4356,
            [
                mission.recovery_latitude_q28_rad,
                mission.recovery_longitude_q28_rad,
                mission.recovery_height_q12_km,
            ],
        )?;
        let planned_samples = accepted_nominal_samples(fixtures, launch_anchor, recovery_anchor)?;
        Ok(Self {
            definition: GlobalDisplayDefinitionV1 {
                display_identity: hash_words(&[
                    GLOBAL_DISPLAY_MODEL_ID,
                    fixtures.earth.identity,
                    fixtures.transforms.identity,
                    mission.identity,
                ]),
                earth_identity: fixtures.earth.identity,
                transform_identity: fixtures.transforms.identity,
                mission_identity: mission.identity,
                epoch_unix_day: fixtures.earth.epoch_unix_day,
                epoch_tai_minus_utc: fixtures.earth.epoch_tai_minus_utc,
                semi_major_q12_km: fixtures.earth.semi_major_q12_km,
                semi_minor_q12_km: fixtures.earth.semi_minor_q12_km,
                inverse_flattening_q20: fixtures.earth.inverse_flattening_q20,
                launch_anchor: launch,
                recovery_anchor: recovery,
                available_source_mask: GLOBAL_DISPLAY_SOURCE_MASK,
                available_frame_mask: 0b111,
                camera_domain_mask: GLOBAL_DISPLAY_CAMERA_DOMAIN_MASK,
            },
            launch_anchor,
            recovery_anchor,
            samples: Vec::new(),
            planned_samples,
            transitions: Vec::new(),
            replay_entries: Vec::new(),
            previous_frame: None,
            previous_segment: None,
            previous_transition_count: 0,
            source_lineages: [None; 4],
            attitude_retired: false,
            pending_discontinuity_mask: 0,
        })
    }

    pub fn definition(&self, role: PresentationRole) -> GlobalDisplayDefinitionV1 {
        self.definition.filter_for_role(role)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &mut self,
        fixtures: &GlobalFixtureSet,
        release_epoch: u32,
        frame: GlobalTelemetryFrame,
        _truth_geodetic_q28_q12: [i32; 3],
        ground: Option<GroundEstimate>,
        transition_records: [FrameTransitionRecord; 4],
        terminal: bool,
    ) {
        let active_frame = display_frame(frame.frame);
        let segment = display_segment(frame.segment);
        let mut discontinuity = 0;
        if self
            .previous_frame
            .is_some_and(|value| value != active_frame)
        {
            discontinuity |= GLOBAL_DISCONTINUITY_FRAME;
        }
        if self.previous_segment.is_some_and(|value| value != segment) {
            discontinuity |= GLOBAL_DISCONTINUITY_SEGMENT;
        }
        if frame.events & (EVENT_DROGUE | EVENT_MAIN) != 0 {
            discontinuity |= GLOBAL_DISCONTINUITY_DEPLOYMENT;
            if !self.attitude_retired {
                discontinuity |= GLOBAL_DISCONTINUITY_ATTITUDE_RETIRED;
                self.attitude_retired = true;
            }
        }
        if terminal || frame.events & EVENT_LANDING != 0 {
            discontinuity |= GLOBAL_DISCONTINUITY_TERMINAL;
        }
        discontinuity |= self.pending_discontinuity_mask;
        self.pending_discontinuity_mask = 0;
        let continuity_identity = hash_words(&[
            self.definition.display_identity,
            u32::from(active_frame as u8),
            u32::from(segment as u8),
            u32::from(frame.transition_count),
        ]);
        let mut sources = Vec::with_capacity(3);
        if let Some(onboard) = pose_from_active(
            fixtures,
            self.launch_anchor,
            self.recovery_anchor,
            GlobalDisplaySourceId::OnboardEstimate,
            active_frame,
            segment,
            frame.navigation_position_q12,
            frame.navigation_velocity_q24,
            frame.navigation_attitude_q30,
            [0; 3],
            frame.mission_time_q16,
            KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
            KSA_G10R_REFERENCE_OPS_MANIFEST_ID ^ 0x4e41_5600,
            frame.checksums[1],
            0,
            !self.attitude_retired,
            false,
        ) {
            sources.push(onboard);
        }
        if let Some(estimate) = ground {
            if let Some(ground_pose) = pose_from_active(
                fixtures,
                self.launch_anchor,
                self.recovery_anchor,
                GlobalDisplaySourceId::GroundEstimate,
                display_interface_frame(estimate.frame),
                segment,
                estimate.position_q12_km,
                estimate.velocity_q24_km_s,
                quaternion_array(QuaternionQ30::IDENTITY),
                [0; 3],
                frame.mission_time_q16,
                estimate.estimator_identity,
                estimate.estimate_identity,
                estimate.estimator_checksum,
                release_epoch.saturating_sub(estimate.production_epoch),
                false,
                false,
            ) {
                sources.push(ground_pose);
            }
        }
        if let Some(truth) = truth_pose(
            fixtures,
            self.launch_anchor,
            self.recovery_anchor,
            frame,
            active_frame,
            segment,
            !self.attitude_retired,
        ) {
            sources.push(truth);
        }
        let mut current_lineages = [None; 4];
        for source in &sources {
            current_lineages[usize::from(source.source as u8 - 1)] =
                Some((source.model_identity, source.estimate_identity));
        }
        if !self.samples.is_empty() && current_lineages != self.source_lineages {
            discontinuity |= GLOBAL_DISCONTINUITY_SOURCE_REPLACED;
        }
        self.source_lineages = current_lineages;
        let (public_geodetic, public_altitude) = sources
            .iter()
            .find(|source| source.source == GlobalDisplaySourceId::OnboardEstimate)
            .and_then(|source| {
                ecef_to_geodetic(GlobalPositionVec::new(
                    source.ecef.position_q12_km[0],
                    source.ecef.position_q12_km[1],
                    source.ecef.position_q12_km[2],
                ))
                .ok()
            })
            .map_or(([0; 3], 0), |value| {
                (
                    [
                        value.latitude_q28_rad,
                        value.longitude_q28_rad,
                        value.height_q12_km,
                    ],
                    value.height_q12_km,
                )
            });
        self.samples.push(GlobalDisplaySampleV1 {
            sequence: u64::from(release_epoch) + 1,
            release_epoch,
            mission_time_q16: frame.mission_time_q16,
            active_frame,
            segment,
            flight_mode: frame.flight_mode,
            transition_count: frame.transition_count,
            event_mask: frame.events,
            discontinuity_mask: discontinuity,
            continuity_identity,
            geodetic_q28_q12: public_geodetic,
            altitude_q12_km: public_altitude,
            mach_q24: frame.mach_q24,
            dynamic_pressure_q14_pa: frame.dynamic_pressure_q14_pa,
            total_mass_q21_kg: frame.total_mass_q21_kg,
            main_propellant_q21_kg: frame.main_propellant_q21_kg,
            rcs_propellant_q21_kg: frame.rcs_propellant_q21_kg,
            gimbal_q15: frame.gimbal_q15,
            rcs_pulses: frame.rcs_pulses,
            command_flags: frame.command_flags,
            command_discrete: frame.command_discrete,
            alarms: frame.alarms,
            sources,
        });
        if frame.transition_count > self.previous_transition_count {
            let index = usize::from(frame.transition_count - 1);
            if let Some(record) = transition_records.get(index).copied() {
                self.push_transition(release_epoch, record, segment);
            }
        }
        if frame.events != 0 {
            self.replay_entries.push(GlobalDisplayReplayEntryV1 {
                release_epoch,
                mission_time_q16: frame.mission_time_q16,
                kind: if terminal || frame.events & EVENT_LANDING != 0 {
                    GlobalDisplayReplayEntryKind::Terminal
                } else {
                    GlobalDisplayReplayEntryKind::MissionEvent
                },
                source_identity: self.definition.mission_identity,
                event_identity: hash_words(&[
                    self.definition.mission_identity,
                    release_epoch,
                    u32::from(frame.events),
                ]),
                detail_identity: u32::from(frame.events),
            });
        }
        self.previous_frame = Some(active_frame);
        self.previous_segment = Some(segment);
        self.previous_transition_count = frame.transition_count;
    }

    pub fn samples_after(&self, index: u32, role: PresentationRole) -> Vec<GlobalDisplaySampleV1> {
        let start = usize::try_from(index)
            .unwrap_or(usize::MAX)
            .min(self.samples.len());
        self.samples[start..]
            .iter()
            .cloned()
            .map(|sample| sample.filter_for_role(role))
            .collect()
    }

    pub fn transitions_after(&self, index: u32) -> &[GlobalDisplayTransitionV1] {
        let start = usize::try_from(index)
            .unwrap_or(usize::MAX)
            .min(self.transitions.len());
        &self.transitions[start..]
    }

    /// Bounded exact samples starting at the first retained release at or after
    /// `start_release`. This is release-addressed rather than index-addressed.
    pub fn samples_from_release(
        &self,
        start_release: u32,
        max_count: usize,
        role: PresentationRole,
    ) -> Vec<GlobalDisplaySampleV1> {
        let start = self
            .samples
            .partition_point(|sample| sample.release_epoch < start_release);
        let end = start.saturating_add(max_count).min(self.samples.len());
        self.samples[start..end]
            .iter()
            .cloned()
            .map(|sample| sample.filter_for_role(role))
            .collect()
    }

    pub const fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn oldest_sample_release(&self) -> Option<u32> {
        self.samples.first().map(|sample| sample.release_epoch)
    }

    pub fn newest_sample_release(&self) -> Option<u32> {
        self.samples.last().map(|sample| sample.release_epoch)
    }

    pub const fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    pub fn transitions_from_release(&self, start_release: u32) -> &[GlobalDisplayTransitionV1] {
        let start = self
            .transitions
            .partition_point(|transition| transition.release_epoch < start_release);
        &self.transitions[start..]
    }

    /// Mark the next emitted sample as an exact onboard-navigation reset. This
    /// is presentation-only and cannot mutate the package or world.
    pub fn mark_navigation_reset(&mut self) {
        self.pending_discontinuity_mask |= GLOBAL_DISCONTINUITY_NAVIGATION_RESET;
    }

    pub fn replay_index(
        &self,
        definition_identity: u32,
        terminal_disposition: u8,
        disposition_axes: [u8; 6],
    ) -> GlobalReplayIndexV1 {
        self.replay_index_with_entries(
            definition_identity,
            terminal_disposition,
            disposition_axes,
            &[],
        )
    }

    pub fn replay_index_with_entries(
        &self,
        definition_identity: u32,
        terminal_disposition: u8,
        disposition_axes: [u8; 6],
        extra_entries: &[GlobalDisplayReplayEntryV1],
    ) -> GlobalReplayIndexV1 {
        let mut entries = self.replay_entries.clone();
        entries.extend_from_slice(extra_entries);
        entries.sort_by_key(|entry| {
            (
                entry.release_epoch,
                entry.kind as u8,
                entry.event_identity,
                entry.detail_identity,
            )
        });
        entries.dedup();
        entries.truncate(GLOBAL_DISPLAY_MAX_REPLAY_ENTRIES);
        GlobalReplayIndexV1 {
            index_identity: hash_words(&[
                GLOBAL_DISPLAY_MODEL_ID,
                definition_identity,
                self.samples.len() as u32,
                entries.len() as u32,
            ]),
            session_definition_identity: definition_identity,
            first_release: self
                .samples
                .first()
                .map_or(0, |sample| sample.release_epoch),
            last_release: self.samples.last().map_or(0, |sample| sample.release_epoch),
            terminal_disposition,
            disposition_axes,
            entries,
        }
    }

    /// Accepted nominal reference samples used only for the labelled planned path.
    pub fn planned_samples(&self) -> &[GlobalDisplaySampleV1] {
        &self.planned_samples
    }

    pub fn path_chunk(
        &self,
        role: PresentationRole,
        source: GlobalDisplaySourceId,
        display_frame: GlobalDisplayFrameId,
        lod: GlobalDisplayPathLod,
        chunk_index: u16,
    ) -> Result<GlobalDisplayPathChunkV1, GlobalDisplayPublishError> {
        self.path_chunk_with_pins(role, source, display_frame, lod, chunk_index, &[])
    }

    pub fn path_chunk_with_pins(
        &self,
        role: PresentationRole,
        source: GlobalDisplaySourceId,
        display_frame: GlobalDisplayFrameId,
        lod: GlobalDisplayPathLod,
        chunk_index: u16,
        pinned_releases: &[u32],
    ) -> Result<GlobalDisplayPathChunkV1, GlobalDisplayPublishError> {
        let path_samples = if source == GlobalDisplaySourceId::Planned {
            &self.planned_samples
        } else {
            &self.samples
        };
        build_global_display_path_chunk(
            role,
            self.definition.display_identity,
            self.launch_anchor.identity,
            self.recovery_anchor.identity,
            path_samples,
            &self.replay_entries,
            source,
            display_frame,
            lod,
            chunk_index,
            pinned_releases,
        )
    }

    fn push_transition(
        &mut self,
        release_epoch: u32,
        record: FrameTransitionRecord,
        to_segment: GlobalDisplaySegment,
    ) {
        let from_segment = self.previous_segment.unwrap_or(to_segment);
        let anchor_identity = match to_segment {
            GlobalDisplaySegment::LocalLaunch => self.launch_anchor.identity,
            GlobalDisplaySegment::LocalRecovery => self.recovery_anchor.identity,
            _ => 0,
        };
        let transition = GlobalDisplayTransitionV1 {
            release_epoch,
            mission_time_q16: record.time.raw(),
            from_frame: display_frame(record.from),
            to_frame: display_frame(record.to),
            from_segment,
            to_segment,
            reason: 1,
            transition_identity: hash_words(&[
                self.definition.transform_identity,
                release_epoch,
                self.transitions.len() as u32,
                record.checksum,
            ]),
            transform_identity: self.definition.transform_identity,
            anchor_identity,
            position_max_delta_raw: record.position_delta_raw,
            velocity_max_delta_raw: record.velocity_delta_raw,
            attitude_max_delta_raw: record.attitude_delta_raw,
            angular_rate_max_delta_raw: record.angular_rate_delta_raw,
            checksum: record.checksum,
        };
        self.transitions.push(transition);
        self.replay_entries.push(GlobalDisplayReplayEntryV1 {
            release_epoch,
            mission_time_q16: record.time.raw(),
            kind: GlobalDisplayReplayEntryKind::FrameTransition,
            source_identity: self.definition.transform_identity,
            event_identity: transition.transition_identity,
            detail_identity: record.checksum,
        });
    }
}

fn accepted_nominal_samples(
    fixtures: &GlobalFixtureSet,
    launch_anchor: LocalAnchor,
    recovery_anchor: LocalAnchor,
) -> Result<Vec<GlobalDisplaySampleV1>, GlobalDisplayPublishError> {
    for (bytes, expected) in [
        (FROZEN_NOMINAL_KTT10, FROZEN_NOMINAL_KTT10_SHA256_HEX),
        (FROZEN_NOMINAL_KPH10, FROZEN_NOMINAL_KPH10_SHA256_HEX),
        (FROZEN_NOMINAL_KSR10, FROZEN_NOMINAL_KSR10_SHA256_HEX),
    ] {
        if crate::phase11_session::sha256(bytes) != decode_sha256(expected)? {
            return Err(GlobalDisplayPublishError::Fixture);
        }
    }
    let header = GlobalTelemetryHeader::decode(&FROZEN_NOMINAL_KTT10[..KTT10_HEADER_LENGTH])
        .map_err(|_| GlobalDisplayPublishError::Fixture)?;
    if header.earth_identity != fixtures.earth.identity
        || header.transform_identity != fixtures.transforms.identity
        || header.mission_identity != fixtures.mission.identity
    {
        return Err(GlobalDisplayPublishError::Fixture);
    }
    let plot_header = GlobalPlotHeader::decode(&FROZEN_NOMINAL_KPH10[..KPH10_HEADER_LENGTH])
        .map_err(|_| GlobalDisplayPublishError::Fixture)?;
    if FROZEN_NOMINAL_KPH10.len()
        != KPH10_HEADER_LENGTH + usize::from(plot_header.point_count) * KPH10_POINT_LENGTH
    {
        return Err(GlobalDisplayPublishError::Fixture);
    }
    for point in FROZEN_NOMINAL_KPH10[KPH10_HEADER_LENGTH..].chunks_exact(KPH10_POINT_LENGTH) {
        GlobalPlotPoint::decode(point).map_err(|_| GlobalDisplayPublishError::Fixture)?;
    }
    if FROZEN_NOMINAL_KSR10.len() != KSR10_LENGTH {
        return Err(GlobalDisplayPublishError::Fixture);
    }
    GlobalEvaluationSummary::decode(FROZEN_NOMINAL_KSR10)
        .map_err(|_| GlobalDisplayPublishError::Fixture)?;

    let body = &FROZEN_NOMINAL_KTT10[KTT10_HEADER_LENGTH..];
    if !body.len().is_multiple_of(KTT10_FRAME_LENGTH) {
        return Err(GlobalDisplayPublishError::Fixture);
    }
    let mut samples = Vec::with_capacity(body.len() / KTT10_FRAME_LENGTH);
    let mut previous_frame = None;
    let mut previous_segment = None;
    let mut attitude_retired = false;
    for bytes in body.chunks_exact(KTT10_FRAME_LENGTH) {
        let frame =
            GlobalTelemetryFrame::decode(bytes).map_err(|_| GlobalDisplayPublishError::Fixture)?;
        let active_frame = display_frame(frame.frame);
        let segment = display_segment(frame.segment);
        let mut discontinuity_mask = 0;
        if previous_frame.is_some_and(|previous| previous != active_frame) {
            discontinuity_mask |= GLOBAL_DISCONTINUITY_FRAME;
        }
        if previous_segment.is_some_and(|previous| previous != segment) {
            discontinuity_mask |= GLOBAL_DISCONTINUITY_SEGMENT;
        }
        if frame.events & (EVENT_DROGUE | EVENT_MAIN) != 0 {
            discontinuity_mask |= GLOBAL_DISCONTINUITY_DEPLOYMENT;
            if !attitude_retired {
                discontinuity_mask |= GLOBAL_DISCONTINUITY_ATTITUDE_RETIRED;
                attitude_retired = true;
            }
        }
        if frame.events & EVENT_LANDING != 0 {
            discontinuity_mask |= GLOBAL_DISCONTINUITY_TERMINAL;
        }
        previous_frame = Some(active_frame);
        previous_segment = Some(segment);
        let mut pose = truth_pose(
            fixtures,
            launch_anchor,
            recovery_anchor,
            frame,
            active_frame,
            segment,
            !attitude_retired,
        )
        .ok_or(GlobalDisplayPublishError::Fixture)?;
        pose.source = GlobalDisplaySourceId::Planned;
        pose.model_identity = ACCEPTED_NOMINAL_REFERENCE_MODEL_ID;
        pose.estimate_identity = header.identity;
        let geodetic = ecef_to_geodetic(GlobalPositionVec::new(
            pose.ecef.position_q12_km[0],
            pose.ecef.position_q12_km[1],
            pose.ecef.position_q12_km[2],
        ))
        .map_err(|_| GlobalDisplayPublishError::Fixture)?;
        samples.push(GlobalDisplaySampleV1 {
            sequence: u64::from(frame.step),
            release_epoch: frame.step.saturating_sub(1),
            mission_time_q16: frame.mission_time_q16,
            active_frame,
            segment,
            flight_mode: frame.flight_mode,
            transition_count: frame.transition_count,
            event_mask: frame.events,
            discontinuity_mask,
            continuity_identity: header.identity,
            geodetic_q28_q12: [
                geodetic.latitude_q28_rad,
                geodetic.longitude_q28_rad,
                geodetic.height_q12_km,
            ],
            altitude_q12_km: geodetic.height_q12_km,
            mach_q24: frame.mach_q24,
            dynamic_pressure_q14_pa: frame.dynamic_pressure_q14_pa,
            total_mass_q21_kg: frame.total_mass_q21_kg,
            main_propellant_q21_kg: frame.main_propellant_q21_kg,
            rcs_propellant_q21_kg: frame.rcs_propellant_q21_kg,
            gimbal_q15: frame.gimbal_q15,
            rcs_pulses: frame.rcs_pulses,
            command_flags: frame.command_flags,
            command_discrete: frame.command_discrete,
            alarms: frame.alarms,
            sources: vec![pose],
        });
    }
    Ok(samples)
}

/// Exact-release, read-only replay products derived by re-executing the frozen
/// Phase 10 nominal mission after its three canonical artifacts are reproduced.
pub struct NominalGlobalDisplayReplay {
    publisher: GlobalDisplayPublisher,
    compatibility: Phase10NominalCompatibilityAudit,
}

impl NominalGlobalDisplayReplay {
    pub fn definition(&self, role: PresentationRole) -> GlobalDisplayDefinitionV1 {
        self.publisher.definition(role)
    }

    pub fn samples_after(&self, index: u32, role: PresentationRole) -> Vec<GlobalDisplaySampleV1> {
        self.publisher.samples_after(index, role)
    }

    pub fn transitions_after(&self, index: u32) -> &[GlobalDisplayTransitionV1] {
        self.publisher.transitions_after(index)
    }

    pub fn replay_index(&self) -> GlobalReplayIndexV1 {
        self.publisher.replay_index(
            self.publisher.definition.display_identity,
            1,
            [1, 1, 1, 1, 1, 1],
        )
    }

    pub fn path_chunk(
        &self,
        role: PresentationRole,
        source: GlobalDisplaySourceId,
        display_frame: GlobalDisplayFrameId,
        lod: GlobalDisplayPathLod,
        chunk_index: u16,
    ) -> Result<GlobalDisplayPathChunkV1, GlobalDisplayPublishError> {
        self.publisher
            .path_chunk(role, source, display_frame, lod, chunk_index)
    }

    pub fn planned_samples(&self) -> &[GlobalDisplaySampleV1] {
        self.publisher.planned_samples()
    }

    pub fn release_count(&self) -> usize {
        self.publisher.samples.len()
    }

    /// Evidence that both the frozen artifact lineage and current portable
    /// re-execution lineage passed before this replay became observable.
    pub const fn compatibility_audit(&self) -> Phase10NominalCompatibilityAudit {
        self.compatibility
    }
}

/// Validate both Phase 10 nominal lineages before exposing a read-only replay.
///
/// The planned/reference path comes from the immutable frozen artifacts. Exact
/// SIM truth comes from the current portable implementation and carries a
/// distinct model/evidence identity. Neither lineage rewrites the other.
pub fn build_nominal_global_display_replay(
) -> Result<NominalGlobalDisplayReplay, GlobalDisplayPublishError> {
    let compatibility = audit_phase10_nominal_lineages()?.audit;

    let fixtures = GlobalFixtureSet::embedded();
    let initial_world = ksa64_sim::phase10::GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )?;
    let flight_config = reference_global_flight_config(
        PHASE10_NOMINAL_SESSION,
        initial_world.active_state()?,
        fixtures.mission,
    )?;
    let mut runner = GlobalAvionicsMission::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
        flight_config,
        GlobalSensorFaults::NONE,
        PHASE10_NOMINAL_CASE_SEED,
    )?;
    let mut publisher = GlobalDisplayPublisher::new(&fixtures)?;
    publisher.definition.available_source_mask = GLOBAL_DISPLAY_SOURCE_PLANNED
        | GLOBAL_DISPLAY_SOURCE_ONBOARD
        | GLOBAL_DISPLAY_SOURCE_SIM_TRUTH;
    let mut release = 0_u32;
    loop {
        let flight = runner.release()?;
        let snapshot = runner.world().snapshot()?;
        let update = mission_update(&runner, snapshot, flight, release.saturating_add(1))?;
        let complete = runner.world().is_complete();
        publisher.publish(
            &fixtures,
            release,
            update.frame,
            [
                update.plot.latitude_q28_rad,
                update.plot.longitude_q28_rad,
                update.plot.altitude_q12_km,
            ],
            None,
            *runner.world().transitions(),
            complete,
        );
        if let Some(truth) = publisher.samples.last_mut().and_then(|sample| {
            sample
                .sources
                .iter_mut()
                .find(|pose| pose.source == GlobalDisplaySourceId::SimTruth)
        }) {
            truth.model_identity = CURRENT_NOMINAL_REEXECUTION_MODEL_ID;
            truth.estimate_identity = CURRENT_NOMINAL_REEXECUTION_EVIDENCE_ID;
        }
        release = release.saturating_add(1);
        if complete {
            break;
        }
        runner.advance_to_next_release()?;
        if release > 460_800 {
            return Err(GlobalDisplayPublishError::World);
        }
    }
    if release != 22_015 || publisher.samples.len() != 22_015 {
        return Err(GlobalDisplayPublishError::ExactReleaseCount);
    }
    Ok(NominalGlobalDisplayReplay {
        publisher,
        compatibility,
    })
}

fn decode_sha256(value: &str) -> Result<[u8; 32], GlobalDisplayPublishError> {
    if value.len() != 64 {
        return Err(GlobalDisplayPublishError::Fixture);
    }
    let mut output = [0; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| GlobalDisplayPublishError::Fixture)?;
    }
    Ok(output)
}

fn display_anchor(
    identity: u32,
    geodetic_q28_q12: [i32; 3],
) -> Result<(GlobalDisplayAnchorV1, LocalAnchor), GlobalDisplayPublishError> {
    let geodetic = ksa64_core::phase10_environment::GeodeticState {
        latitude_q28_rad: geodetic_q28_q12[0],
        longitude_q28_rad: geodetic_q28_q12[1],
        height_q12_km: geodetic_q28_q12[2],
    };
    let ecef = geodetic_to_ecef(geodetic).map_err(|_| GlobalDisplayPublishError::Fixture)?;
    let value = GlobalDisplayAnchorV1 {
        identity,
        geodetic_q28_q12,
        ecef_position_q12_km: [ecef.x(), ecef.y(), ecef.z()],
    };
    Ok((value, anchor(value)?))
}

fn anchor(value: GlobalDisplayAnchorV1) -> Result<LocalAnchor, GlobalDisplayPublishError> {
    let geodetic = ksa64_core::phase10_environment::GeodeticState {
        latitude_q28_rad: value.geodetic_q28_q12[0],
        longitude_q28_rad: value.geodetic_q28_q12[1],
        height_q12_km: value.geodetic_q28_q12[2],
    };
    let origin_ecef = geodetic_to_ecef(geodetic).map_err(|_| GlobalDisplayPublishError::Fixture)?;
    if value.ecef_position_q12_km != [origin_ecef.x(), origin_ecef.y(), origin_ecef.z()] {
        return Err(GlobalDisplayPublishError::Fixture);
    }
    LocalAnchor {
        identity: value.identity,
        origin_ecef,
        enu_to_ecef: enu_to_ecef_rotation(geodetic.latitude_q28_rad, geodetic.longitude_q28_rad)
            .map_err(|_| GlobalDisplayPublishError::Fixture)?,
        reference_meridian_q28_rad: geodetic.longitude_q28_rad,
    }
    .validate()
    .map_err(|_| GlobalDisplayPublishError::Fixture)
}

#[allow(clippy::too_many_arguments)]
fn pose_from_active(
    fixtures: &GlobalFixtureSet,
    launch_anchor: LocalAnchor,
    recovery_anchor: LocalAnchor,
    source: GlobalDisplaySourceId,
    active_frame: GlobalDisplayFrameId,
    segment: GlobalDisplaySegment,
    position: [i32; 3],
    velocity: [i32; 3],
    attitude: [i32; 4],
    angular_rate: [i32; 3],
    mission_time_q16: u32,
    model_identity: u32,
    estimate_identity: u32,
    checksum: u32,
    age_releases: u32,
    has_attitude: bool,
    has_angular_rate: bool,
) -> Option<GlobalDisplaySourcePoseV1> {
    let time = MissionTimeQ16::from_raw(mission_time_q16)?;
    let quaternion = QuaternionQ30::new(attitude[0], attitude[1], attitude[2], attitude[3]);
    let rate = GlobalAngularRateVec::new(angular_rate[0], angular_rate[1], angular_rate[2]);
    let ecef = match active_frame {
        GlobalDisplayFrameId::LocalEnu => {
            let local = LocalKinematicState {
                position: EnuPosition::new(
                    scale_saturating(position[0], 2_000, 1),
                    scale_saturating(position[1], 2_000, 1),
                    scale_saturating(position[2], 2_000, 1),
                ),
                velocity: EnuVelocity::new(
                    scale_saturating(velocity[0], 125, 4),
                    scale_saturating(velocity[1], 125, 4),
                    scale_saturating(velocity[2], 125, 4),
                ),
                attitude: quaternion,
                angular_rate: rate,
                time,
            };
            local_to_ecef(
                if segment == GlobalDisplaySegment::LocalRecovery {
                    recovery_anchor
                } else {
                    launch_anchor
                },
                local,
            )
            .ok()?
        }
        GlobalDisplayFrameId::EarthFixedEcef => GlobalKinematicState::new(
            GlobalPositionVec::new(position[0], position[1], position[2]),
            GlobalVelocityVec::new(velocity[0], velocity[1], velocity[2]),
            quaternion,
            rate,
            time,
        ),
        GlobalDisplayFrameId::EarthInertialGcrf => {
            let state = GlobalKinematicState::new(
                GlobalPositionVec::new(position[0], position[1], position[2]),
                GlobalVelocityVec::new(velocity[0], velocity[1], velocity[2]),
                quaternion,
                rate,
                time,
            );
            let transform = interpolate_transform(&fixtures.transforms, time).ok()?;
            gcrf_to_ecef(transform, state).ok()?
        }
    };
    resolved_pose(
        fixtures,
        launch_anchor,
        recovery_anchor,
        source,
        active_frame,
        segment,
        ecef,
        Some((position, velocity, attitude)),
        model_identity.max(1),
        estimate_identity,
        checksum,
        age_releases,
        has_attitude,
        has_angular_rate,
    )
}

/// Phase 10 telemetry truth fields are produced from the world canonical state.
/// During local segments mission_update has already resolved that truth to ECEF;
/// only navigation fields remain active-frame local.
fn truth_pose(
    fixtures: &GlobalFixtureSet,
    launch_anchor: LocalAnchor,
    recovery_anchor: LocalAnchor,
    frame: GlobalTelemetryFrame,
    active_frame: GlobalDisplayFrameId,
    segment: GlobalDisplaySegment,
    has_attitude: bool,
) -> Option<GlobalDisplaySourcePoseV1> {
    let time = MissionTimeQ16::from_raw(frame.mission_time_q16)?;
    let rate = GlobalAngularRateVec::new(
        frame.truth_angular_rate_q24[0],
        frame.truth_angular_rate_q24[1],
        frame.truth_angular_rate_q24[2],
    );
    let active_attitude = QuaternionQ30::new(
        frame.truth_attitude_q30[0],
        frame.truth_attitude_q30[1],
        frame.truth_attitude_q30[2],
        frame.truth_attitude_q30[3],
    );
    let ecef_attitude = if active_frame == GlobalDisplayFrameId::EarthInertialGcrf {
        let active = GlobalKinematicState::new(
            GlobalPositionVec::new(
                frame.truth_position_q12[0],
                frame.truth_position_q12[1],
                frame.truth_position_q12[2],
            ),
            GlobalVelocityVec::new(
                frame.truth_velocity_q24[0],
                frame.truth_velocity_q24[1],
                frame.truth_velocity_q24[2],
            ),
            active_attitude,
            rate,
            time,
        );
        let transform = interpolate_transform(&fixtures.transforms, time).ok()?;
        gcrf_to_ecef(transform, active).ok()?.attitude
    } else {
        active_attitude
    };
    let ecef = GlobalKinematicState::new(
        GlobalPositionVec::new(
            frame.ecef_position_q12[0],
            frame.ecef_position_q12[1],
            frame.ecef_position_q12[2],
        ),
        GlobalVelocityVec::new(
            frame.ecef_velocity_q24[0],
            frame.ecef_velocity_q24[1],
            frame.ecef_velocity_q24[2],
        ),
        ecef_attitude,
        rate,
        time,
    );
    let active_override = match active_frame {
        GlobalDisplayFrameId::LocalEnu => None,
        _ => Some((
            frame.truth_position_q12,
            frame.truth_velocity_q24,
            frame.truth_attitude_q30,
        )),
    };
    resolved_pose(
        fixtures,
        launch_anchor,
        recovery_anchor,
        GlobalDisplaySourceId::SimTruth,
        active_frame,
        segment,
        ecef,
        active_override,
        GLOBAL_DISPLAY_MODEL_ID ^ 0x5452_5554,
        GLOBAL_DISPLAY_MODEL_ID ^ 0x5452_5554,
        frame.checksums[0],
        0,
        has_attitude,
        has_attitude,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolved_pose(
    fixtures: &GlobalFixtureSet,
    launch_anchor: LocalAnchor,
    recovery_anchor: LocalAnchor,
    source: GlobalDisplaySourceId,
    active_frame: GlobalDisplayFrameId,
    segment: GlobalDisplaySegment,
    ecef: GlobalKinematicState,
    active_override: Option<([i32; 3], [i32; 3], [i32; 4])>,
    model_identity: u32,
    estimate_identity: u32,
    checksum: u32,
    age_releases: u32,
    has_attitude: bool,
    has_angular_rate: bool,
) -> Option<GlobalDisplaySourcePoseV1> {
    let transform = interpolate_transform(&fixtures.transforms, ecef.time).ok()?;
    let gcrf = ecef_to_gcrf(transform, ecef).ok()?;
    let mut launch_pose = ecef_to_local(launch_anchor, ecef).ok().map(local_pose);
    let mut recovery_pose = ecef_to_local(recovery_anchor, ecef).ok().map(local_pose);
    let mut ecef_pose = global_pose(ecef);
    let mut gcrf_pose = global_pose(gcrf);
    let mut active = match active_override {
        Some((position, velocity, attitude)) => GlobalDisplayResolvedPoseV1 {
            position_q12_km: position,
            velocity_q24_km_s: velocity,
            attitude_q30: attitude,
        },
        None => match active_frame {
            GlobalDisplayFrameId::LocalEnu => {
                if segment == GlobalDisplaySegment::LocalRecovery {
                    recovery_pose?
                } else {
                    launch_pose?
                }
            }
            GlobalDisplayFrameId::EarthFixedEcef => ecef_pose,
            GlobalDisplayFrameId::EarthInertialGcrf => gcrf_pose,
        },
    };
    if !has_attitude {
        clear_attitude(&mut active);
        clear_attitude(&mut ecef_pose);
        clear_attitude(&mut gcrf_pose);
        if let Some(pose) = launch_pose.as_mut() {
            clear_attitude(pose);
        }
        if let Some(pose) = recovery_pose.as_mut() {
            clear_attitude(pose);
        }
    }
    let mut validity = GLOBAL_POSE_VALID_ACTIVE_POSITION
        | GLOBAL_POSE_VALID_ACTIVE_VELOCITY
        | GLOBAL_POSE_VALID_ECEF_POSITION
        | GLOBAL_POSE_VALID_ECEF_VELOCITY
        | GLOBAL_POSE_VALID_GCRF_POSITION
        | GLOBAL_POSE_VALID_GCRF_VELOCITY;
    if launch_pose.is_some() {
        validity |= GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION | GLOBAL_POSE_VALID_LAUNCH_ENU_VELOCITY;
    }
    if recovery_pose.is_some() {
        validity |=
            GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION | GLOBAL_POSE_VALID_RECOVERY_ENU_VELOCITY;
    }
    if has_attitude {
        validity |= GLOBAL_POSE_VALID_ACTIVE_ATTITUDE
            | GLOBAL_POSE_VALID_ECEF_ATTITUDE
            | GLOBAL_POSE_VALID_GCRF_ATTITUDE
            | if launch_pose.is_some() {
                GLOBAL_POSE_VALID_LAUNCH_ENU_ATTITUDE
            } else {
                0
            }
            | if recovery_pose.is_some() {
                GLOBAL_POSE_VALID_RECOVERY_ENU_ATTITUDE
            } else {
                0
            };
    }
    if has_angular_rate {
        validity |= GLOBAL_POSE_VALID_ANGULAR_RATE;
    }
    Some(GlobalDisplaySourcePoseV1 {
        source,
        active_frame,
        validity_mask: validity,
        model_identity,
        estimate_identity,
        checksum,
        age_releases,
        active,
        ecef: ecef_pose,
        gcrf: gcrf_pose,
        launch_enu: launch_pose.unwrap_or_default(),
        recovery_enu: recovery_pose.unwrap_or_default(),
        angular_rate_q24: if has_angular_rate {
            [
                ecef.angular_rate.x(),
                ecef.angular_rate.y(),
                ecef.angular_rate.z(),
            ]
        } else {
            [0; 3]
        },
    })
}

fn clear_attitude(pose: &mut GlobalDisplayResolvedPoseV1) {
    pose.attitude_q30 = [0; 4];
}

fn path_anchor_identity(
    frame: GlobalDisplayFrameId,
    segment: GlobalDisplaySegment,
    launch_anchor_identity: u32,
    recovery_anchor_identity: u32,
) -> u32 {
    if frame != GlobalDisplayFrameId::LocalEnu {
        0
    } else if segment == GlobalDisplaySegment::LocalRecovery {
        recovery_anchor_identity
    } else {
        launch_anchor_identity
    }
}

fn resolved_position(
    pose: &GlobalDisplaySourcePoseV1,
    frame: GlobalDisplayFrameId,
    segment: GlobalDisplaySegment,
) -> Option<[i32; 3]> {
    Some(match frame {
        GlobalDisplayFrameId::LocalEnu if segment == GlobalDisplaySegment::LocalRecovery => {
            if pose.validity_mask & GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION == 0 {
                return None;
            }
            pose.recovery_enu.position_q12_km
        }
        GlobalDisplayFrameId::LocalEnu => {
            if pose.validity_mask & GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION == 0 {
                return None;
            }
            pose.launch_enu.position_q12_km
        }
        GlobalDisplayFrameId::EarthFixedEcef => pose.ecef.position_q12_km,
        GlobalDisplayFrameId::EarthInertialGcrf => pose.gcrf.position_q12_km,
    })
}

fn global_pose(state: GlobalKinematicState) -> GlobalDisplayResolvedPoseV1 {
    GlobalDisplayResolvedPoseV1 {
        position_q12_km: [state.position.x(), state.position.y(), state.position.z()],
        velocity_q24_km_s: [state.velocity.x(), state.velocity.y(), state.velocity.z()],
        attitude_q30: quaternion_array(state.attitude),
    }
}

fn local_pose(state: LocalKinematicState) -> GlobalDisplayResolvedPoseV1 {
    GlobalDisplayResolvedPoseV1 {
        position_q12_km: [
            scale_saturating(state.position.x(), 1, 2_000),
            scale_saturating(state.position.y(), 1, 2_000),
            scale_saturating(state.position.z(), 1, 2_000),
        ],
        velocity_q24_km_s: [
            scale_saturating(state.velocity.x(), 4, 125),
            scale_saturating(state.velocity.y(), 4, 125),
            scale_saturating(state.velocity.z(), 4, 125),
        ],
        attitude_q30: quaternion_array(state.attitude),
    }
}

fn quaternion_array(value: QuaternionQ30) -> [i32; 4] {
    [value.w(), value.x(), value.y(), value.z()]
}

fn scale_saturating(value: i32, numerator: i64, denominator: i64) -> i32 {
    let value = i64::from(value) * numerator;
    let half = denominator / 2;
    let rounded = if value >= 0 {
        (value + half) / denominator
    } else {
        (value - half) / denominator
    };
    rounded.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn display_frame(value: ReferenceFrameId) -> GlobalDisplayFrameId {
    match value {
        ReferenceFrameId::LocalEnuV1 => GlobalDisplayFrameId::LocalEnu,
        ReferenceFrameId::EarthFixedEcefV1 => GlobalDisplayFrameId::EarthFixedEcef,
        ReferenceFrameId::EarthInertialEciV1 => GlobalDisplayFrameId::EarthInertialGcrf,
    }
}

fn display_interface_frame(value: GlobalFrameId) -> GlobalDisplayFrameId {
    match value {
        GlobalFrameId::LocalEnuV1 => GlobalDisplayFrameId::LocalEnu,
        GlobalFrameId::EarthFixedEcefV1 => GlobalDisplayFrameId::EarthFixedEcef,
        GlobalFrameId::EarthInertialEciV1 => GlobalDisplayFrameId::EarthInertialGcrf,
    }
}

fn display_segment(value: GlobalSegment) -> GlobalDisplaySegment {
    match value {
        GlobalSegment::LocalLaunch => GlobalDisplaySegment::LocalLaunch,
        GlobalSegment::EcefAscent => GlobalDisplaySegment::EcefAscent,
        GlobalSegment::EciCoast => GlobalDisplaySegment::EciCoast,
        GlobalSegment::EcefEntry => GlobalDisplaySegment::EcefEntry,
        GlobalSegment::LocalRecovery => GlobalDisplaySegment::LocalRecovery,
    }
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
mod exact_replay_tests {
    use super::*;

    #[test]
    fn frozen_planned_overview_pins_every_frame_transition() {
        let fixtures = GlobalFixtureSet::embedded();
        let publisher = GlobalDisplayPublisher::new(&fixtures).unwrap();
        let path = publisher
            .path_chunk(
                PresentationRole::SimDirector,
                GlobalDisplaySourceId::Planned,
                GlobalDisplayFrameId::EarthFixedEcef,
                GlobalDisplayPathLod::FourSecond,
                0,
            )
            .unwrap();
        assert_eq!(path.model_identity, ACCEPTED_NOMINAL_REFERENCE_MODEL_ID);
        let releases: Vec<_> = path
            .points
            .iter()
            .map(|point| point.release_epoch)
            .collect();
        for transition in [29, 3_579, 12_669, 15_255] {
            assert!(releases.contains(&transition));
        }
    }

    #[test]
    fn live_publisher_is_role_safe_and_marks_every_exact_snap_boundary() {
        let fixtures = GlobalFixtureSet::embedded();
        let mut publisher = GlobalDisplayPublisher::new(&fixtures).unwrap();
        let definition = publisher.definition(PresentationRole::GuidedOperator);
        assert_ne!(definition.launch_anchor.ecef_position_q12_km, [0; 3]);
        assert_ne!(definition.recovery_anchor.ecef_position_q12_km, [0; 3]);

        let mut frame = GlobalTelemetryFrame::decode(
            &FROZEN_NOMINAL_KTT10[KTT10_HEADER_LENGTH..KTT10_HEADER_LENGTH + KTT10_FRAME_LENGTH],
        )
        .unwrap();
        frame.step = 1;
        frame.mission_time_q16 = 0;
        frame.frame = ReferenceFrameId::LocalEnuV1;
        frame.segment = GlobalSegment::LocalLaunch;
        frame.transition_count = 0;
        frame.events = 0;
        frame.navigation_position_q12 = [0; 3];
        frame.navigation_velocity_q24 = [0; 3];
        frame.navigation_attitude_q30 = quaternion_array(QuaternionQ30::IDENTITY);
        publisher.publish(
            &fixtures,
            0,
            frame,
            [0; 3],
            None,
            [FrameTransitionRecord::ZERO; 4],
            false,
        );

        let ground = GroundEstimate {
            estimator_identity: 0x1200_0001,
            estimate_identity: 0x1200_0002,
            source_observation_identity: 0x1200_0003,
            measurement_epoch: 1,
            production_epoch: 1,
            frame: GlobalFrameId::LocalEnuV1,
            flags: 1,
            position_q12_km: frame.navigation_position_q12,
            velocity_q24_km_s: frame.navigation_velocity_q24,
            confidence_q16: [1; 3],
            residual_q16: [0; 3],
            estimator_checksum: 0x1200_0004,
        };
        frame.step = 2;
        frame.mission_time_q16 = 2_048;
        frame.segment = GlobalSegment::LocalRecovery;
        frame.events = EVENT_DROGUE;
        publisher.mark_navigation_reset();
        publisher.publish(
            &fixtures,
            1,
            frame,
            [i32::MAX; 3],
            Some(ground),
            [FrameTransitionRecord::ZERO; 4],
            false,
        );

        frame.step = 3;
        frame.mission_time_q16 = 4_096;
        frame.events = 0;
        publisher.publish(
            &fixtures,
            2,
            frame,
            [i32::MIN; 3],
            None,
            [FrameTransitionRecord::ZERO; 4],
            false,
        );

        let samples = publisher.samples_from_release(0, 3, PresentationRole::GuidedOperator);
        assert_eq!(samples.len(), 3);
        assert_eq!(publisher.sample_count(), 3);
        assert_eq!(publisher.oldest_sample_release(), Some(0));
        assert_eq!(publisher.newest_sample_release(), Some(2));
        let first_onboard = samples[0]
            .sources
            .iter()
            .find(|pose| pose.source == GlobalDisplaySourceId::OnboardEstimate)
            .unwrap();
        let expected_geodetic = ecef_to_geodetic(GlobalPositionVec::new(
            first_onboard.ecef.position_q12_km[0],
            first_onboard.ecef.position_q12_km[1],
            first_onboard.ecef.position_q12_km[2],
        ))
        .unwrap();
        assert_eq!(
            samples[0].geodetic_q28_q12,
            [
                expected_geodetic.latitude_q28_rad,
                expected_geodetic.longitude_q28_rad,
                expected_geodetic.height_q12_km,
            ]
        );
        assert_eq!(samples[0].altitude_q12_km, expected_geodetic.height_q12_km);
        assert_eq!(
            [
                samples[0].mach_q24,
                samples[0].dynamic_pressure_q14_pa,
                samples[0].total_mass_q21_kg,
                samples[0].main_propellant_q21_kg,
                samples[0].rcs_propellant_q21_kg,
            ],
            [0; 5]
        );
        assert_ne!(
            samples[1].discontinuity_mask & GLOBAL_DISCONTINUITY_NAVIGATION_RESET,
            0
        );
        assert_ne!(
            samples[1].discontinuity_mask & GLOBAL_DISCONTINUITY_SOURCE_REPLACED,
            0
        );
        assert_ne!(
            samples[1].discontinuity_mask & GLOBAL_DISCONTINUITY_ATTITUDE_RETIRED,
            0
        );
        for pose in &samples[1].sources {
            assert_eq!(pose.validity_mask & GLOBAL_POSE_VALID_ACTIVE_ATTITUDE, 0);
            assert_eq!(pose.validity_mask & GLOBAL_POSE_VALID_ANGULAR_RATE, 0);
            assert_eq!(pose.angular_rate_q24, [0; 3]);
        }

        for (index, sample) in samples.iter().enumerate() {
            let onboard = sample
                .sources
                .iter()
                .find(|pose| pose.source == GlobalDisplaySourceId::OnboardEstimate)
                .unwrap_or_else(|| panic!("missing onboard source at {index}: {sample:?}"));
            let required = if sample.segment == GlobalDisplaySegment::LocalRecovery {
                GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION
            } else {
                GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION
            };
            assert_ne!(
                onboard.validity_mask & required,
                0,
                "sample {index}: {onboard:?}"
            );
        }
        let local_path = publisher
            .path_chunk_with_pins(
                PresentationRole::GuidedOperator,
                GlobalDisplaySourceId::OnboardEstimate,
                GlobalDisplayFrameId::LocalEnu,
                GlobalDisplayPathLod::OneSecond,
                0,
                &[2],
            )
            .unwrap();
        assert_ne!(local_path.flags & GLOBAL_PATH_FLAG_INCOMPLETE, 0);
        assert_eq!(local_path.points.last().unwrap().release_epoch, 2);
        assert_eq!(
            local_path.points[0].anchor_identity,
            definition.launch_anchor.identity
        );
        assert_eq!(
            local_path.points[1].anchor_identity,
            definition.recovery_anchor.identity
        );

        let stale_ground = publisher
            .path_chunk_with_pins(
                PresentationRole::GuidedOperator,
                GlobalDisplaySourceId::GroundEstimate,
                GlobalDisplayFrameId::EarthFixedEcef,
                GlobalDisplayPathLod::OneSecond,
                0,
                &[],
            )
            .unwrap();
        assert_ne!(stale_ground.flags & GLOBAL_PATH_FLAG_STALE, 0);

        // A delayed operational source has a complete path once its retained
        // history reaches the terminal sample; it does not need to fabricate
        // a pose at mission start.
        frame.step = 4;
        frame.mission_time_q16 = 6_144;
        frame.events = EVENT_LANDING;
        let terminal_ground = GroundEstimate {
            measurement_epoch: 3,
            production_epoch: 3,
            ..ground
        };
        publisher.publish(
            &fixtures,
            3,
            frame,
            [0; 3],
            Some(terminal_ground),
            [FrameTransitionRecord::ZERO; 4],
            true,
        );
        let completed_ground = publisher
            .path_chunk_with_pins(
                PresentationRole::GuidedOperator,
                GlobalDisplaySourceId::GroundEstimate,
                GlobalDisplayFrameId::EarthFixedEcef,
                GlobalDisplayPathLod::OneSecond,
                0,
                &[],
            )
            .unwrap();
        assert_ne!(completed_ground.flags & GLOBAL_PATH_FLAG_TERMINAL, 0);
        assert_eq!(completed_ground.flags & GLOBAL_PATH_FLAG_INCOMPLETE, 0);
        assert_eq!(completed_ground.points.first().unwrap().release_epoch, 1);
        assert_eq!(completed_ground.points.last().unwrap().release_epoch, 3);

        let mut omitted_source_start =
            publisher.samples_from_release(0, 4, PresentationRole::GuidedOperator);
        omitted_source_start[1].event_mask = 0;
        omitted_source_start[1].discontinuity_mask = 0;
        let incomplete_ground = build_global_display_path_chunk(
            PresentationRole::GuidedOperator,
            definition.display_identity,
            definition.launch_anchor.identity,
            definition.recovery_anchor.identity,
            &omitted_source_start,
            &[],
            GlobalDisplaySourceId::GroundEstimate,
            GlobalDisplayFrameId::EarthFixedEcef,
            GlobalDisplayPathLod::OneSecond,
            0,
            &[],
        )
        .unwrap();
        assert_ne!(
            incomplete_ground.flags & GLOBAL_PATH_FLAG_INCOMPLETE,
            0,
            "omitting the first resolvable source sample must remain visible"
        );
    }

    #[test]
    #[ignore = "full accepted Phase 10 nominal re-execution"]
    fn exact_nominal_replay_reproduces_frozen_release_boundaries() {
        let replay = build_nominal_global_display_replay().expect("nominal display replay");
        assert_eq!(replay.release_count(), 22_015);
        assert_eq!(
            replay.compatibility_audit().event_identity,
            crate::phase10_nominal_compat::PHASE10_NOMINAL_EVENT_IDENTITY
        );
        let samples = replay.samples_after(0, PresentationRole::SimDirector);
        assert_eq!(samples.first().map(|value| value.release_epoch), Some(0));
        assert_eq!(
            samples[0]
                .sources
                .iter()
                .find(|pose| pose.source == GlobalDisplaySourceId::SimTruth)
                .map(|pose| (pose.model_identity, pose.estimate_identity)),
            Some((
                CURRENT_NOMINAL_REEXECUTION_MODEL_ID,
                CURRENT_NOMINAL_REEXECUTION_EVIDENCE_ID
            ))
        );
        assert_eq!(
            samples.last().map(|value| value.release_epoch),
            Some(22_014)
        );
        let transition_epochs: Vec<_> = replay
            .transitions_after(0)
            .iter()
            .map(|value| value.release_epoch)
            .collect();
        assert_eq!(transition_epochs, vec![29, 3_579, 12_669, 15_255]);
        assert!(
            replay
                .path_chunk(
                    PresentationRole::SimDirector,
                    GlobalDisplaySourceId::Planned,
                    GlobalDisplayFrameId::EarthFixedEcef,
                    GlobalDisplayPathLod::OneSecond,
                    0,
                )
                .expect("verified planned path")
                .points
                .len()
                > 16
        );
    }
}
