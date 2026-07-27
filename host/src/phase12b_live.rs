//! Full-duration Phase 12B KSA-G10R operations session.
//!
//! The accepted Phase 10 world remains authoritative. This module composes it
//! with the accepted Phase 11 package, an independent ground estimator, and a
//! host-only presentation/evidence layer. The compact nine-release Phase 11
//! fixture remains unchanged as a compatibility oracle.

use crate::phase10::GlobalFixtureSet;
use crate::phase10_mission::{
    encode_kph10, encode_ksr10, encode_ktt10, mission_update_with_case, GlobalMissionCapture,
    PHASE10_RECORD_STRIDE_RELEASES,
};
use crate::phase11_authoring::{
    compile_project_source, encode_action_log, push_definition_segments, CompiledMissionProject,
    CompletedMissionSession, MissionScenario, SessionRunEvidence,
};
use crate::phase11_live::{
    MissionActionReceipt, MissionOperatorAction, MissionSessionError, MissionSessionEvent,
    MissionSessionEventKind, MissionSessionLifecycle, MissionSessionPace,
};
use crate::phase11_operations::{
    role_can_act, OperationalMetricSnapshot, ProcedureEngine, ProcedureState, METRIC_GNSS_VALID,
    METRIC_INERTIAL_HEALTHY, METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12,
};
use crate::phase11_prediction::{
    project_ground_estimate, project_onboard_estimate, HostPrediction,
};
use crate::phase11_scenarios::ActionTranscript;
use crate::phase11_session::{SessionBundleBuilder, SessionBundleIdentity, SessionSegmentKind};
use crate::phase12b::{
    classify_disposition, full_gnss_loss_mission_plan, full_gnss_loss_procedure_pack,
    procedure_copy, ActionProposalView, AvionicsDisposition, EvidenceDisposition,
    MissionObjectiveDisposition, OperationalDispositionEvidence, OperationalDispositionView,
    OperatorDisposition, ProcedureDisposition, ProcedurePredicateView, ProcedureView,
    ReleaseSampleView, TimelineEventView, TimelineSource, VehicleDisposition,
    DECISION_WINDOW_CLOSE_RELEASE, DECISION_WINDOW_OPEN_RELEASE, FULL_GNSS_LOSS_SCENARIO_ID,
    GNSS_LOSS_RELEASE, GNSS_QUALIFIED_RELEASE,
};
use ksa64_core::evaluation::EvaluationOutcome;
use ksa64_core::phase10_contract::ReferenceFrameId;
use ksa64_core::phase10_telemetry::{
    GlobalEvaluationSummary, GlobalPlotHeader, GlobalPlotPoint, GlobalTelemetryHeader,
    KPH10_HEADER_LENGTH, KPH10_POINT_LENGTH, KSR10_LENGTH,
};
use ksa64_core::scenario::crc32_ieee;
use ksa64_flight::phase10::{GlobalFlightConfig, GlobalFlightEvidence};
use ksa64_flight::phase11::{
    KsaG10rReferenceOpsV1, EVENT_JOURNAL_CAPACITY, KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
};
use ksa64_interface::phase10::{GlobalFrameId, GLOBAL_AID_GNSS};
use ksa64_interface::phase11::{
    write_kej11, write_kge11, write_kgo11, write_kpp11_header, write_kpp11_point,
    EventJournalRecord, FlightAbiId, GroundEstimate, GroundTrackingObservation, OperationalRole,
    UplinkCommandLoad, UplinkControlKind, UplinkControlRecord, UplinkLoadType, UplinkReasonCode,
    UplinkState, KEJ11_LENGTH, KGE11_LENGTH, KGO11_LENGTH, KPP11_HEADER_LENGTH, KPP11_POINT_LENGTH,
    PACKAGE_CAP_BRANCH_SELECT, PACKAGE_CAP_GROUND_NAV_UPDATE,
};
use ksa64_sim::phase10::GlobalWorldError;
use ksa64_sim::phase10_avionics::{
    reference_global_flight_config, GlobalPackageAvionicsMission, GlobalSensorFaults,
};
use ksa64_sim::phase10_evaluation::{adapt_global_result, GlobalEvaluationRequest};
use ksa64_sim::phase11::{
    synthesize_ground_observation, GroundEstimator, GroundTruthSample, GROUND_ESTIMATOR_ID,
};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::OnceLock;

pub const FULL_GNSS_LOSS_SOURCE: &str = include_str!("../../phase12/examples/gnss-loss-full.json");
pub const FULL_MISSION_CASE_SEED: u32 = 0x4b53_41b2;
pub const FULL_MISSION_SESSION: u16 = 0x12b0;
pub const FULL_TELEMETRY_IDENTITY: u32 = 0x12b1_0001;
pub const FULL_PLOT_IDENTITY: u32 = 0x12b1_0002;
pub const FULL_RECORDING_STRIDE_RELEASES: u32 = 32;
pub const FULL_PRESENTATION_STRIDE_RELEASES: u32 = 8;
pub const GROUND_TRACKING_DELAY_RELEASES: u32 = 4;
pub const UPDATE_STAGE_RELEASE: u32 = 6_080;
pub const UPDATE_COMMIT_RELEASE: u32 = 6_240;
pub const UPDATE_EFFECTIVE_RELEASE: u32 = 6_400;
pub const BRANCH_STAGE_RELEASE: u32 = 6_560;
pub const BRANCH_COMMIT_RELEASE: u32 = 6_720;
pub const BRANCH_EFFECTIVE_RELEASE: u32 = 6_880;
pub const FULL_MISSION_MAX_RELEASES: u32 = 24_000;

static GLOBAL_FIXTURES: OnceLock<GlobalFixtureSet> = OnceLock::new();
const ACCEPTED_NOMINAL_KPH10: &[u8] =
    include_bytes!("../../phase10/evidence/ksa-g10r-nominal.kph10");
const ACCEPTED_NOMINAL_KPH10_SHA256: [u8; 32] = [
    0xcd, 0x66, 0x4e, 0x8b, 0x72, 0xef, 0xf7, 0xaf, 0xf1, 0xe3, 0xc4, 0xa5, 0xb7, 0xfb, 0x68, 0x59,
    0xbb, 0x9d, 0x51, 0x78, 0xd3, 0xb6, 0xb6, 0xd4, 0xc2, 0xc0, 0x6f, 0x2c, 0x61, 0xed, 0x9c, 0xf2,
];

fn fixtures() -> &'static GlobalFixtureSet {
    GLOBAL_FIXTURES.get_or_init(GlobalFixtureSet::embedded)
}

pub const ACCEPTED_NOMINAL_REFERENCE_MODEL_ID: u32 = 0x12b5_0001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlannedTrajectoryError {
    ArtifactHash,
    Header,
    Identity,
    Length,
    Point,
    Time,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannedTrajectoryPoint {
    pub release_epoch: u32,
    pub frame: ReferenceFrameId,
    pub altitude_q12_km: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedTrajectoryReference {
    pub path_identity: u32,
    pub evaluation_identity: u32,
    pub cadence_releases: u16,
    pub artifact_crc32: u32,
    pub points: Vec<PlannedTrajectoryPoint>,
}

/// Strictly decodes the accepted Phase 10 nominal KPH10 into a presentation-only
/// reference path. The current mission's private truth is never consulted, and
/// the KPH10 truth-checksum field is deliberately not copied into this view.
pub fn accepted_nominal_reference_trajectory(
) -> Result<PlannedTrajectoryReference, PlannedTrajectoryError> {
    parse_accepted_nominal_reference_trajectory(ACCEPTED_NOMINAL_KPH10)
}

fn parse_accepted_nominal_reference_trajectory(
    bytes: &[u8],
) -> Result<PlannedTrajectoryReference, PlannedTrajectoryError> {
    if bytes.len() < KPH10_HEADER_LENGTH {
        return Err(PlannedTrajectoryError::Length);
    }
    let header = GlobalPlotHeader::decode(&bytes[..KPH10_HEADER_LENGTH])
        .map_err(|_| PlannedTrajectoryError::Header)?;
    if header.identity != crate::phase10_mission::PHASE10_PLOT_IDENTITY {
        return Err(PlannedTrajectoryError::Identity);
    }
    let expected_length = KPH10_HEADER_LENGTH
        .checked_add(usize::from(header.point_count) * KPH10_POINT_LENGTH)
        .ok_or(PlannedTrajectoryError::Length)?;
    if bytes.len() != expected_length {
        return Err(PlannedTrajectoryError::Length);
    }

    let mut points = Vec::with_capacity(usize::from(header.point_count));
    let mut previous_epoch = None;
    for point_bytes in bytes[KPH10_HEADER_LENGTH..].chunks_exact(KPH10_POINT_LENGTH) {
        let point =
            GlobalPlotPoint::decode(point_bytes).map_err(|_| PlannedTrajectoryError::Point)?;
        if !point.mission_time_q16.is_multiple_of(2_048) {
            return Err(PlannedTrajectoryError::Time);
        }
        let release_epoch = point.mission_time_q16 / 2_048;
        if previous_epoch.is_some_and(|previous| release_epoch <= previous) {
            return Err(PlannedTrajectoryError::Time);
        }
        previous_epoch = Some(release_epoch);
        points.push(PlannedTrajectoryPoint {
            release_epoch,
            frame: point.frame,
            altitude_q12_km: point.altitude_q12_km,
            downrange_q12_km: point.downrange_q12_km,
            crossrange_q12_km: point.crossrange_q12_km,
        });
    }
    if points.len() != usize::from(header.point_count) {
        return Err(PlannedTrajectoryError::Length);
    }
    if crate::phase11_session::sha256(bytes) != ACCEPTED_NOMINAL_KPH10_SHA256 {
        return Err(PlannedTrajectoryError::ArtifactHash);
    }
    Ok(PlannedTrajectoryReference {
        path_identity: header.identity,
        evaluation_identity: header.evaluation_identity,
        cadence_releases: header.stride_releases,
        artifact_crc32: crc32_ieee(bytes),
        points,
    })
}

#[derive(Clone, Debug)]
pub struct FullMissionSnapshot {
    pub lifecycle: MissionSessionLifecycle,
    pub pace: MissionSessionPace,
    pub release_epoch: u32,
    pub mission_time_q16: u32,
    pub role: OperationalRole,
    pub frame: Option<GlobalFrameId>,
    pub flight: Option<GlobalFlightEvidence>,
    pub ground: Option<GroundEstimate>,
    pub procedure: Option<ProcedureView>,
    pub recommended_action: Option<ActionProposalView>,
    pub latest_onboard_prediction: Option<HostPrediction>,
    pub latest_ground_prediction: Option<HostPrediction>,
    pub disposition: Option<OperationalDispositionView>,
    pub action_count: u32,
    pub rejected_loads: u16,
    pub event_count: u32,
    pub current_sample: Option<ReleaseSampleView>,
}

#[derive(Clone, Debug)]
pub struct FullMissionCompletion {
    pub session: CompletedMissionSession,
    pub global_summary: GlobalEvaluationSummary,
    pub disposition: OperationalDispositionView,
    pub ktt10: Vec<u8>,
    pub kph10: Vec<u8>,
    pub ksr10: [u8; KSR10_LENGTH],
}

pub struct FullMissionSession {
    project: CompiledMissionProject,
    role: OperationalRole,
    lifecycle: MissionSessionLifecycle,
    pace: MissionSessionPace,
    resume_pace: MissionSessionPace,
    runner: Option<GlobalPackageAvionicsMission<'static, KsaG10rReferenceOpsV1>>,
    flight_config: Option<GlobalFlightConfig>,
    procedure: Option<ProcedureEngine>,
    ground_estimator: GroundEstimator,
    pending_observations: VecDeque<GroundTrackingObservation>,
    observations: Vec<GroundTrackingObservation>,
    estimates: Vec<GroundEstimate>,
    latest_ground: Option<GroundEstimate>,
    latest_flight: Option<GlobalFlightEvidence>,
    latest_onboard_prediction: Option<HostPrediction>,
    latest_ground_prediction: Option<HostPrediction>,
    prediction_records: Vec<HostPrediction>,
    transcript: ActionTranscript,
    staged_load: Option<UplinkCommandLoad>,
    rejected_loads: u16,
    selected_branch: u8,
    events: Vec<MissionSessionEvent>,
    timeline: Vec<TimelineEventView>,
    release_samples: Vec<ReleaseSampleView>,
    frames: Vec<ksa64_core::phase10_telemetry::GlobalTelemetryFrame>,
    plot_points: Vec<ksa64_core::phase10_telemetry::GlobalPlotPoint>,
    release_epoch: u32,
    gnss_missing_fixes: u8,
    last_barometer_q12_km: i32,
    last_transition_count: u8,
    completed: Option<FullMissionCompletion>,
}

impl FullMissionSession {
    pub fn new(role: OperationalRole) -> Result<Self, MissionSessionError> {
        // Compile one immutable mission definition. Role and hints are runtime
        // presentation policy and therefore cannot alter canonical session identity.
        let project = compile_project_source(FULL_GNSS_LOSS_SOURCE)
            .map_err(|_| MissionSessionError::Authoring)?;
        Self::compiled_with_role(project, role)
    }

    pub fn compiled(project: CompiledMissionProject) -> Result<Self, MissionSessionError> {
        let role = project.role;
        Self::compiled_with_role(project, role)
    }

    fn compiled_with_role(
        project: CompiledMissionProject,
        role: OperationalRole,
    ) -> Result<Self, MissionSessionError> {
        if project.scenario != MissionScenario::GnssLossFull {
            return Err(MissionSessionError::Unsupported);
        }
        let mut value = Self {
            project,
            role,
            lifecycle: MissionSessionLifecycle::Compiled,
            pace: MissionSessionPace::Realtime,
            resume_pace: MissionSessionPace::Realtime,
            runner: None,
            flight_config: None,
            procedure: None,
            ground_estimator: GroundEstimator::new(),
            pending_observations: VecDeque::new(),
            observations: Vec::new(),
            estimates: Vec::new(),
            latest_ground: None,
            latest_flight: None,
            latest_onboard_prediction: None,
            latest_ground_prediction: None,
            prediction_records: Vec::new(),
            transcript: ActionTranscript::new(),
            staged_load: None,
            rejected_loads: 0,
            selected_branch: 0,
            events: Vec::new(),
            timeline: Vec::new(),
            release_samples: Vec::new(),
            frames: Vec::new(),
            plot_points: Vec::new(),
            release_epoch: 0,
            gnss_missing_fixes: 0,
            last_barometer_q12_km: 0,
            last_transition_count: 0,
            completed: None,
        };
        value.record(
            MissionSessionEventKind::Compiled,
            value.project.definition_identity,
        );
        Ok(value)
    }

    pub fn prepare(&mut self) -> Result<(), MissionSessionError> {
        if self.lifecycle != MissionSessionLifecycle::Compiled {
            return Err(MissionSessionError::Lifecycle);
        }
        let data = fixtures();
        let world = ksa64_sim::phase10::GlobalWorldMachine::new(
            &data.earth,
            &data.transforms,
            &data.atmosphere,
            &data.vehicle,
            data.mission,
        )
        .map_err(world_error)?;
        let config = reference_global_flight_config(
            FULL_MISSION_SESSION,
            world.active_state().map_err(world_error)?,
            data.mission,
        )
        .map_err(world_error)?;
        let mut package =
            KsaG10rReferenceOpsV1::new(config).ok_or(MissionSessionError::Unsupported)?;
        if !package.initialize_mission_plan(full_gnss_loss_mission_plan()) {
            return Err(MissionSessionError::Unsupported);
        }
        let faults = GlobalSensorFaults {
            gnss_dropout_from_release: GNSS_LOSS_RELEASE as u16,
            gnss_dropout_until_release: u16::MAX,
            ..GlobalSensorFaults::NONE
        };
        let runner = GlobalPackageAvionicsMission::with_package(
            &data.earth,
            &data.transforms,
            &data.atmosphere,
            &data.vehicle,
            data.mission,
            config,
            package,
            faults,
            FULL_MISSION_CASE_SEED,
        )
        .map_err(world_error)?;
        self.runner = Some(runner);
        self.flight_config = Some(config);
        self.lifecycle = MissionSessionLifecycle::Ready;
        self.record(
            MissionSessionEventKind::Prepared,
            self.project.definition_identity,
        );
        self.timeline_event(0, TimelineSource::Evidence, 0, 0x12b4_0001, "Mission ready");
        Ok(())
    }

    pub const fn lifecycle(&self) -> MissionSessionLifecycle {
        self.lifecycle
    }

    pub const fn role(&self) -> OperationalRole {
        self.role
    }

    pub fn hints_enabled(&self) -> bool {
        self.role == OperationalRole::GuidedOperator
    }

    pub fn set_pace(&mut self, pace: MissionSessionPace) -> Result<(), MissionSessionError> {
        if matches!(
            self.lifecycle,
            MissionSessionLifecycle::Completed | MissionSessionLifecycle::Aborted
        ) {
            return Err(MissionSessionError::Lifecycle);
        }
        if pace == MissionSessionPace::Paused {
            return self.pause();
        }
        self.pace = pace;
        self.resume_pace = if pace == MissionSessionPace::SingleStep {
            MissionSessionPace::Realtime
        } else {
            pace
        };
        self.record(MissionSessionEventKind::PaceChanged, pace as u32);
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), MissionSessionError> {
        if !matches!(
            self.lifecycle,
            MissionSessionLifecycle::Ready | MissionSessionLifecycle::Running
        ) {
            return Err(MissionSessionError::Lifecycle);
        }
        if self.pace != MissionSessionPace::Paused {
            self.resume_pace = self.pace;
        }
        self.pace = MissionSessionPace::Paused;
        self.lifecycle = MissionSessionLifecycle::Paused;
        self.record(MissionSessionEventKind::Paused, 0);
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), MissionSessionError> {
        if self.lifecycle != MissionSessionLifecycle::Paused {
            return Err(MissionSessionError::Lifecycle);
        }
        self.pace = self.resume_pace;
        self.lifecycle = MissionSessionLifecycle::Running;
        self.record(MissionSessionEventKind::Resumed, self.pace as u32);
        Ok(())
    }

    pub fn advance_one_release(&mut self) -> Result<FullMissionSnapshot, MissionSessionError> {
        if self.lifecycle == MissionSessionLifecycle::Paused {
            return Err(MissionSessionError::Lifecycle);
        }
        self.advance_internal(false)
    }

    pub fn step_one_release(&mut self) -> Result<FullMissionSnapshot, MissionSessionError> {
        if !matches!(
            self.lifecycle,
            MissionSessionLifecycle::Ready
                | MissionSessionLifecycle::Running
                | MissionSessionLifecycle::Paused
        ) {
            return Err(MissionSessionError::Lifecycle);
        }
        let snapshot = self.advance_internal(true)?;
        if self.lifecycle != MissionSessionLifecycle::Completed {
            self.lifecycle = MissionSessionLifecycle::Paused;
            self.pace = MissionSessionPace::Paused;
            self.record(MissionSessionEventKind::Paused, 0);
        }
        Ok(snapshot)
    }

    pub fn advance_bounded(
        &mut self,
        maximum_releases: u32,
    ) -> Result<FullMissionSnapshot, MissionSessionError> {
        if maximum_releases == 0 || self.pace == MissionSessionPace::Paused {
            return Ok(self.snapshot());
        }
        let budget = match self.pace {
            MissionSessionPace::Fast => maximum_releases.min(256),
            MissionSessionPace::Realtime | MissionSessionPace::SingleStep => 1,
            MissionSessionPace::Paused => 0,
        };
        for _ in 0..budget {
            self.advance_internal(self.pace == MissionSessionPace::SingleStep)?;
            if matches!(
                self.lifecycle,
                MissionSessionLifecycle::Completed | MissionSessionLifecycle::Paused
            ) {
                break;
            }
        }
        Ok(self.snapshot())
    }

    pub fn snapshot(&self) -> FullMissionSnapshot {
        FullMissionSnapshot {
            lifecycle: self.lifecycle,
            pace: self.pace,
            release_epoch: self.release_epoch,
            mission_time_q16: self
                .latest_flight
                .map_or(0, |_| self.release_epoch.saturating_sub(1) * 2_048),
            role: self.role,
            frame: self.latest_flight.map(|value| value.navigation.frame),
            flight: self.latest_flight,
            ground: self.latest_ground,
            procedure: self.procedure_view(),
            recommended_action: self.recommended_action(),
            latest_onboard_prediction: self.latest_onboard_prediction.clone(),
            latest_ground_prediction: self.latest_ground_prediction.clone(),
            disposition: self.completed.as_ref().map(|value| value.disposition),
            action_count: self.transcript.records.len() as u32,
            rejected_loads: self.rejected_loads,
            event_count: self.events.len() as u32,
            current_sample: self.release_samples.last().copied(),
        }
    }

    pub fn events_after(&self, sequence: u32) -> &[MissionSessionEvent] {
        let start = usize::try_from(sequence)
            .unwrap_or(usize::MAX)
            .min(self.events.len());
        &self.events[start..]
    }

    pub fn timeline_after(&self, sequence: u32) -> &[TimelineEventView] {
        let start = usize::try_from(sequence)
            .unwrap_or(usize::MAX)
            .min(self.timeline.len());
        &self.timeline[start..]
    }

    pub fn release_samples_after(&self, index: u32) -> &[ReleaseSampleView] {
        let start = usize::try_from(index)
            .unwrap_or(usize::MAX)
            .min(self.release_samples.len());
        &self.release_samples[start..]
    }

    pub fn recommended_load(&self) -> Option<UplinkCommandLoad> {
        if self.staged_load.is_some() || self.release_epoch > DECISION_WINDOW_CLOSE_RELEASE {
            return None;
        }
        let step = self.procedure.as_ref()?.current_step();
        match step {
            4 if self.release_epoch >= UPDATE_STAGE_RELEASE => Some(self.ground_update_load(
                self.release_epoch,
                UPDATE_EFFECTIVE_RELEASE.max(self.release_epoch + 160),
            )?),
            5 if self.release_epoch >= BRANCH_STAGE_RELEASE => Some(self.branch_load(
                self.release_epoch,
                BRANCH_EFFECTIVE_RELEASE.max(self.release_epoch + 160),
            )?),
            _ => None,
        }
    }

    pub fn recommended_action(&self) -> Option<ActionProposalView> {
        let load = self.recommended_load()?;
        Some(ActionProposalView {
            proposal_identity: load.load_identity,
            load_type: load.load_type,
            earliest_commit_epoch: load.stage_epoch.saturating_add(2),
            activation_epoch: load.requested_effective_epoch,
            expires_epoch: load.expires_epoch,
            payload_checksum: hash_load(&load),
        })
    }

    /// Build the declared conservative-recovery branch from current public state.
    /// The branch preserves autonomous entry and recovery; it never commands the
    /// low-level safe state or any effector directly.
    pub fn safe_recovery_load(&self) -> Option<UplinkCommandLoad> {
        if self.staged_load.is_some()
            || self.release_epoch > DECISION_WINDOW_CLOSE_RELEASE
            || self.procedure.as_ref()?.current_step() != 5
        {
            return None;
        }
        let mut load = self.branch_load(
            self.release_epoch,
            BRANCH_EFFECTIVE_RELEASE.max(self.release_epoch.saturating_add(160)),
        )?;
        load.load_identity = 0x12b5_0003;
        load.arguments[0] = 2;
        Some(load)
    }

    pub fn commit_request_for_staged(&self) -> Option<UplinkControlRecord> {
        self.staged_load.map(|load| UplinkControlRecord {
            kind: UplinkControlKind::CommitRequest,
            control_identity: load.load_identity ^ 0x55aa_0000,
            load_identity: load.load_identity,
            package_manifest_identity: load.package_manifest_identity,
            plan_identity: load.plan_identity,
            request_epoch: self.release_epoch,
            effective_epoch: load.requested_effective_epoch,
            state: UplinkState::Staged,
            reason: UplinkReasonCode::Accepted,
            receipt_checksum: 0,
        })
    }

    pub fn cancel_request_for_staged(&self) -> Option<UplinkControlRecord> {
        let mut request = self.commit_request_for_staged()?;
        request.kind = UplinkControlKind::Cancellation;
        Some(request)
    }

    pub fn submit_operator_action(
        &mut self,
        action: MissionOperatorAction,
    ) -> Result<MissionActionReceipt, MissionSessionError> {
        if !matches!(
            self.lifecycle,
            MissionSessionLifecycle::Ready
                | MissionSessionLifecycle::Running
                | MissionSessionLifecycle::Paused
        ) {
            return Err(MissionSessionError::Lifecycle);
        }
        if !role_can_act(self.role) {
            return Err(MissionSessionError::ActionRejected);
        }
        if let MissionOperatorAction::Stage { load, .. } = action {
            match load.load_type {
                UplinkLoadType::GroundNavigationUpdate => {
                    // A navigation update is ground-estimator evidence, not a caller-authored
                    // state vector. Bind the staged bytes to the exact latest public estimate
                    // before the flight package performs its independent residual checks.
                    let ground = self
                        .latest_ground
                        .ok_or(MissionSessionError::ActionRejected)?;
                    if load.source_estimator_identity != GROUND_ESTIMATOR_ID
                        || load.source_estimator_checksum != ground.estimator_checksum
                        || load.frame != ground.frame
                        || load.arguments[..3] != ground.position_q12_km
                        || load.arguments[3..6] != ground.velocity_q24_km_s
                    {
                        return Err(MissionSessionError::ActionRejected);
                    }
                }
                UplinkLoadType::ContingencyBranch => {
                    let plan = full_gnss_loss_mission_plan();
                    let declared = plan.branches[..usize::from(plan.branch_count)]
                        .iter()
                        .any(|branch| i32::from(branch.branch_id) == load.arguments[0]);
                    if !declared {
                        return Err(MissionSessionError::ActionRejected);
                    }
                }
                _ => {}
            }
        }
        if matches!(action, MissionOperatorAction::Commit(_)) {
            let load = self
                .staged_load
                .ok_or(MissionSessionError::ActionUnavailable)?;
            let accepts = self.procedure.as_ref().is_some_and(|procedure| {
                procedure.state() == ProcedureState::Active
                    && procedure
                        .current()
                        .is_some_and(|step| step.action == Some(load.load_type))
            });
            if !accepts {
                return Err(MissionSessionError::ActionRejected);
            }
        }
        let step = self
            .procedure
            .as_ref()
            .map_or(0, ProcedureEngine::current_step);
        let runner = self.runner.as_mut().ok_or(MissionSessionError::Lifecycle)?;
        let epoch = self.release_epoch;
        let record = match action {
            MissionOperatorAction::Stage {
                load,
                completed_event_mask,
            } => {
                let receipt = runner
                    .package_mut()
                    .stage_uplink(load, completed_event_mask)
                    .ok_or(MissionSessionError::ActionRejected)?;
                if receipt.kind == UplinkControlKind::StageReceipt
                    && receipt.load_identity == load.load_identity
                    && receipt.state == UplinkState::Staged
                    && receipt.reason == UplinkReasonCode::Accepted
                {
                    self.staged_load = Some(load);
                } else {
                    self.rejected_loads = self.rejected_loads.saturating_add(1);
                }
                receipt
            }
            MissionOperatorAction::Commit(request) => {
                let load = self
                    .staged_load
                    .ok_or(MissionSessionError::ActionUnavailable)?;
                let receipt = runner
                    .package_mut()
                    .commit_uplink(&request)
                    .ok_or(MissionSessionError::ActionRejected)?;
                if receipt.kind == UplinkControlKind::CommitReceipt
                    && receipt.load_identity == load.load_identity
                    && receipt.state == UplinkState::Committed
                    && receipt.reason == UplinkReasonCode::Accepted
                {
                    if load.load_type == UplinkLoadType::ContingencyBranch {
                        self.selected_branch = load.arguments[0].clamp(0, u8::MAX as i32) as u8;
                    }
                    if let Some(procedure) = self.procedure.as_mut() {
                        procedure
                            .accept_action(self.role, epoch, load.load_type, true)
                            .map_err(|_| MissionSessionError::Procedure)?;
                    }
                    self.staged_load = None;
                } else {
                    self.rejected_loads = self.rejected_loads.saturating_add(1);
                }
                receipt
            }
            MissionOperatorAction::Cancel(request) => {
                let receipt = runner
                    .package_mut()
                    .cancel_uplink(&request)
                    .ok_or(MissionSessionError::ActionRejected)?;
                if receipt.kind == UplinkControlKind::Cancellation
                    && receipt.load_identity == request.load_identity
                    && receipt.state == UplinkState::Cancelled
                    && receipt.reason == UplinkReasonCode::Accepted
                {
                    self.staged_load = None;
                } else {
                    self.rejected_loads = self.rejected_loads.saturating_add(1);
                }
                receipt
            }
        };
        self.transcript.record(epoch, step, record);
        let accepted = matches!(
            (record.kind, record.state, record.reason),
            (
                UplinkControlKind::StageReceipt,
                UplinkState::Staged,
                UplinkReasonCode::Accepted
            ) | (
                UplinkControlKind::CommitReceipt,
                UplinkState::Committed,
                UplinkReasonCode::Accepted
            ) | (
                UplinkControlKind::Cancellation,
                UplinkState::Cancelled,
                UplinkReasonCode::Accepted
            )
        );
        let kind = match (accepted, record.state) {
            (true, UplinkState::Staged) => MissionSessionEventKind::ActionStaged,
            (true, UplinkState::Committed) => MissionSessionEventKind::ActionCommitted,
            (true, UplinkState::Cancelled) => MissionSessionEventKind::ActionCancelled,
            _ => MissionSessionEventKind::ActionRejected,
        };
        self.record(kind, record.load_identity);
        self.timeline_event(
            epoch,
            TimelineSource::Operator,
            u8::from(!accepted),
            record.load_identity,
            action_label(record),
        );
        Ok(MissionActionReceipt { record, accepted })
    }

    pub fn abort(&mut self, reason_identity: u32) -> Result<(), MissionSessionError> {
        if matches!(
            self.lifecycle,
            MissionSessionLifecycle::Completed | MissionSessionLifecycle::Aborted
        ) {
            return Err(MissionSessionError::Lifecycle);
        }
        self.lifecycle = MissionSessionLifecycle::Aborted;
        self.record(MissionSessionEventKind::Aborted, reason_identity);
        Ok(())
    }

    pub fn finish(self) -> Result<FullMissionCompletion, MissionSessionError> {
        if self.lifecycle != MissionSessionLifecycle::Completed {
            return Err(MissionSessionError::NotCompleted);
        }
        self.completed.ok_or(MissionSessionError::NotCompleted)
    }

    pub fn run_scripted_to_completion(
        mut self,
    ) -> Result<FullMissionCompletion, MissionSessionError> {
        if self.lifecycle == MissionSessionLifecycle::Compiled {
            self.prepare()?;
        }
        self.set_pace(MissionSessionPace::Fast)?;
        while self.lifecycle != MissionSessionLifecycle::Completed {
            match self.release_epoch {
                UPDATE_STAGE_RELEASE | BRANCH_STAGE_RELEASE => {
                    if let Some(load) = self.recommended_load() {
                        self.submit_operator_action(MissionOperatorAction::Stage {
                            load,
                            completed_event_mask: 0,
                        })?;
                    }
                }
                UPDATE_COMMIT_RELEASE | BRANCH_COMMIT_RELEASE => {
                    if let Some(request) = self.commit_request_for_staged() {
                        self.submit_operator_action(MissionOperatorAction::Commit(request))?;
                    }
                }
                _ => {}
            }
            self.advance_internal(false)?;
        }
        self.finish()
    }

    fn advance_internal(
        &mut self,
        single_step: bool,
    ) -> Result<FullMissionSnapshot, MissionSessionError> {
        if !matches!(
            self.lifecycle,
            MissionSessionLifecycle::Ready
                | MissionSessionLifecycle::Running
                | MissionSessionLifecycle::Paused
        ) {
            return Err(MissionSessionError::Lifecycle);
        }
        if self.release_epoch >= FULL_MISSION_MAX_RELEASES {
            return Err(MissionSessionError::Lifecycle);
        }
        self.lifecycle = MissionSessionLifecycle::Running;
        let epoch = self.release_epoch;
        let (bundle, world_snapshot, update, world_complete) = {
            let runner = self.runner.as_mut().ok_or(MissionSessionError::Lifecycle)?;
            let bundle = runner.release_bundle().map_err(world_error)?;
            let world_snapshot = runner.world().snapshot().map_err(world_error)?;
            let update = mission_update_with_case(
                runner,
                world_snapshot,
                bundle.evidence,
                epoch + 1,
                FULL_MISSION_CASE_SEED,
            )
            .map_err(world_error)?;
            let world_complete = runner.world().is_complete();
            (bundle, world_snapshot, update, world_complete)
        };
        self.latest_flight = Some(bundle.evidence);
        if self
            .staged_load
            .is_some_and(|load| epoch > load.expires_epoch)
        {
            let expired = self.staged_load.take().expect("staged load checked");
            self.timeline_event(
                epoch,
                TimelineSource::Operator,
                1,
                expired.load_identity,
                "Staged command load expired without commit",
            );
        }
        if let Some(aid) = bundle.aid {
            if aid.validity & ksa64_interface::phase10::GLOBAL_AID_BAROMETER != 0 {
                self.last_barometer_q12_km = aid.barometer_q12_km;
            }
            if epoch.is_multiple_of(32) {
                if aid.validity & GLOBAL_AID_GNSS == 0 && epoch >= GNSS_LOSS_RELEASE {
                    self.gnss_missing_fixes = self.gnss_missing_fixes.saturating_add(1);
                } else {
                    self.gnss_missing_fixes = 0;
                }
            }
        }
        if epoch.is_multiple_of(4) {
            let state = world_snapshot.state;
            let observation = synthesize_ground_observation(
                GroundTruthSample {
                    epoch,
                    frame: interface_frame(world_snapshot.frame),
                    position_q12_km: [state.position.x(), state.position.y(), state.position.z()],
                    velocity_q24_km_s: [state.velocity.x(), state.velocity.y(), state.velocity.z()],
                },
                GROUND_TRACKING_DELAY_RELEASES,
                FULL_MISSION_CASE_SEED ^ 0x11e0_0001,
            );
            self.observations.push(observation);
            self.pending_observations.push_back(observation);
        }
        while self
            .pending_observations
            .front()
            .is_some_and(|value| value.receipt_epoch <= epoch)
        {
            let observation = self
                .pending_observations
                .pop_front()
                .expect("front checked");
            if let Some(estimate) = self.ground_estimator.update(observation, epoch) {
                self.latest_ground = Some(estimate);
                self.estimates.push(estimate);
            }
        }
        if epoch == GNSS_LOSS_RELEASE {
            self.timeline_event(
                epoch,
                TimelineSource::Avionics,
                1,
                0x12b4_1001,
                "GNSS observations missing",
            );
        }
        if self.gnss_missing_fixes >= 3 && epoch == GNSS_QUALIFIED_RELEASE {
            self.timeline_event(
                epoch,
                TimelineSource::Procedure,
                1,
                0x12b4_1002,
                "GNSS loss qualified after three missing fixes",
            );
        }
        if epoch == DECISION_WINDOW_OPEN_RELEASE {
            self.procedure = Some(
                ProcedureEngine::new(
                    full_gnss_loss_procedure_pack(full_gnss_loss_mission_plan().plan_identity),
                    epoch,
                )
                .map_err(|_| MissionSessionError::Procedure)?,
            );
            self.timeline_event(
                epoch,
                TimelineSource::Procedure,
                1,
                0x12b4_1003,
                "GNSS-loss response window open",
            );
        }
        self.tick_procedure(epoch)?;
        if epoch.is_multiple_of(32) {
            self.update_predictions(epoch, bundle.evidence)?;
        }
        let important = epoch == 0
            || (epoch + 1).is_multiple_of(FULL_RECORDING_STRIDE_RELEASES)
            || world_snapshot.events != 0
            || world_snapshot.transition_count != self.last_transition_count
            || world_complete;
        if important {
            self.frames.push(update.frame);
            self.plot_points.push(update.plot);
        }
        if epoch.is_multiple_of(FULL_PRESENTATION_STRIDE_RELEASES) || important {
            self.release_samples
                .push(self.operational_sample(epoch, &update));
        }
        if world_snapshot.transition_count != self.last_transition_count {
            self.timeline_event(
                epoch,
                TimelineSource::World,
                0,
                0x12b4_2000 | u32::from(world_snapshot.transition_count),
                "Reference frame ownership transition",
            );
            self.last_transition_count = world_snapshot.transition_count;
        }
        self.release_epoch = self.release_epoch.saturating_add(1);
        self.record(MissionSessionEventKind::Release, self.release_epoch);
        if world_complete {
            self.complete()?;
        } else {
            self.runner
                .as_mut()
                .ok_or(MissionSessionError::Lifecycle)?
                .advance_to_next_release()
                .map_err(world_error)?;
        }
        if single_step && self.lifecycle != MissionSessionLifecycle::Completed {
            self.lifecycle = MissionSessionLifecycle::Paused;
        }
        Ok(self.snapshot())
    }

    fn tick_procedure(&mut self, epoch: u32) -> Result<(), MissionSessionError> {
        if epoch > DECISION_WINDOW_CLOSE_RELEASE {
            if let Some(procedure) = self.procedure.as_mut() {
                procedure
                    .expire_at(epoch)
                    .map_err(|_| MissionSessionError::Procedure)?;
            }
        }
        let mut snapshot = OperationalMetricSnapshot::new(epoch);
        snapshot.set(METRIC_GNSS_VALID, i32::from(self.gnss_missing_fixes < 3));
        snapshot.set(
            METRIC_INERTIAL_HEALTHY,
            i32::from(self.latest_flight.is_some_and(|value| !value.safe)),
        );
        if let (Some(flight), Some(ground)) = (self.latest_flight, self.latest_ground) {
            snapshot.set(
                METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12,
                maximum_difference(flight.navigation.position_q12, ground.position_q12_km),
            );
        }
        let change = {
            let Some(procedure) = self.procedure.as_mut() else {
                return Ok(());
            };
            let previous_step = procedure.current_step();
            let previous_state = procedure.state();
            procedure
                .tick(&snapshot)
                .map_err(|_| MissionSessionError::Procedure)?;
            (procedure.current_step() != previous_step || procedure.state() != previous_state)
                .then_some((procedure.current_step(), procedure.state()))
        };
        if let Some((step, state)) = change {
            let (title, _) = procedure_copy(step);
            self.timeline_event(
                epoch,
                TimelineSource::Procedure,
                u8::from(matches!(
                    state,
                    ProcedureState::Failed | ProcedureState::Mistimed
                )),
                0x12b4_3000 | u32::from(step),
                title,
            );
        }
        Ok(())
    }

    fn update_predictions(
        &mut self,
        epoch: u32,
        flight: GlobalFlightEvidence,
    ) -> Result<(), MissionSessionError> {
        let data = fixtures();
        let plan = full_gnss_loss_mission_plan();
        if let Ok(prediction) =
            project_onboard_estimate(flight.navigation, epoch, epoch, &plan, &data.earth)
        {
            self.latest_onboard_prediction = Some(prediction.clone());
            if epoch.is_multiple_of(256) || epoch == GNSS_QUALIFIED_RELEASE {
                self.prediction_records.push(prediction);
            }
        }
        if let Some(ground) = self.latest_ground {
            if let Ok(prediction) = project_ground_estimate(ground, epoch, &plan, &data.earth) {
                self.latest_ground_prediction = Some(prediction.clone());
                if epoch.is_multiple_of(256) || epoch == GNSS_QUALIFIED_RELEASE {
                    self.prediction_records.push(prediction);
                }
            }
        }
        Ok(())
    }

    fn procedure_view(&self) -> Option<ProcedureView> {
        let procedure = self.procedure.as_ref()?;
        let current = procedure.current()?;
        let (title, instruction) = procedure_copy(current.step_id);
        let predicates = current.predicates[..usize::from(current.predicate_count)]
            .iter()
            .map(|predicate| ProcedurePredicateView {
                predicate_id: predicate.metric_id,
                satisfied: self.predicate_satisfied(predicate.metric_id, predicate.threshold),
                valid: true,
            })
            .collect();
        Some(ProcedureView {
            procedure_identity: 0x12b3_1001,
            active_step: current.step_id,
            step_count: 7,
            entered_epoch: procedure.entered_epoch(),
            deadline_epoch: procedure
                .entered_epoch()
                .saturating_add(current.timeout_epochs),
            title: title.to_owned(),
            instruction: instruction.to_owned(),
            predicates,
        })
    }

    fn predicate_satisfied(&self, metric: u16, threshold: i32) -> bool {
        match metric {
            METRIC_GNSS_VALID => self.gnss_missing_fixes >= 3,
            METRIC_INERTIAL_HEALTHY => self.latest_flight.is_some_and(|value| !value.safe),
            METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12 => {
                match (self.latest_flight, self.latest_ground) {
                    (Some(flight), Some(ground)) => {
                        maximum_difference(flight.navigation.position_q12, ground.position_q12_km)
                            <= threshold
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn operational_sample(
        &self,
        epoch: u32,
        update: &crate::phase10_mission::GlobalMissionUpdate,
    ) -> ReleaseSampleView {
        let onboard = self
            .latest_onboard_prediction
            .as_ref()
            .and_then(|value| value.points.first());
        let ground = self
            .latest_ground_prediction
            .as_ref()
            .and_then(|value| value.points.first());
        let sim_director = self.role == OperationalRole::SimDirector;
        ReleaseSampleView {
            epoch,
            frame: update.frame.frame as u8,
            mission_time_q16: update.frame.mission_time_q16,
            altitude_q12_km: if sim_director {
                update.plot.altitude_q12_km
            } else {
                ground.map_or(self.last_barometer_q12_km, |value| value.altitude_q12_km)
            },
            speed_q24_km_s: if sim_director {
                update.plot.speed_q24_km_s
            } else {
                0
            },
            downrange_q12_km: if sim_director {
                update.plot.downrange_q12_km
            } else {
                ground.map_or(0, |value| value.downrange_q12_km)
            },
            crossrange_q12_km: if sim_director {
                update.plot.crossrange_q12_km
            } else {
                ground.map_or(0, |value| value.crossrange_q12_km)
            },
            onboard_altitude_q12_km: onboard.map_or(0, |value| value.altitude_q12_km),
            ground_altitude_q12_km: ground.map_or(0, |value| value.altitude_q12_km),
            flags: u32::from(sim_director) | (u32::from(self.gnss_missing_fixes >= 3) << 1),
        }
    }

    fn ground_update_load(
        &self,
        stage_epoch: u32,
        effective_epoch: u32,
    ) -> Option<UplinkCommandLoad> {
        let ground = self.latest_ground?;
        let flight = self.latest_flight?;
        let position_residual =
            maximum_difference(flight.navigation.position_q12, ground.position_q12_km);
        let velocity_residual =
            maximum_difference(flight.navigation.velocity_q24, ground.velocity_q24_km_s);
        if position_residual > 262_144 || velocity_residual > 16_777_216 {
            return None;
        }
        let mut arguments = [0; 16];
        arguments[..3].copy_from_slice(&ground.position_q12_km);
        arguments[3..6].copy_from_slice(&ground.velocity_q24_km_s);
        Some(UplinkCommandLoad {
            load_identity: 0x12b5_0001,
            package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
            plan_identity: full_gnss_loss_mission_plan().plan_identity,
            abi: FlightAbiId::GlobalKlr10V1,
            source_estimator_identity: GROUND_ESTIMATOR_ID,
            source_estimator_checksum: ground.estimator_checksum,
            stage_epoch,
            not_before_epoch: stage_epoch + 2,
            expires_epoch: effective_epoch + 64,
            requested_effective_epoch: effective_epoch,
            required_capabilities: PACKAGE_CAP_GROUND_NAV_UPDATE,
            prerequisite_event_mask: 0,
            position_residual_limit_q12: position_residual.saturating_add(4_096).min(262_144),
            velocity_residual_limit_q24: velocity_residual.saturating_add(65_536).min(16_777_216),
            frame: ground.frame,
            load_type: UplinkLoadType::GroundNavigationUpdate,
            arguments,
        })
    }

    fn branch_load(&self, stage_epoch: u32, effective_epoch: u32) -> Option<UplinkCommandLoad> {
        let flight = self.latest_flight?;
        let mut arguments = [0; 16];
        arguments[0] = 1;
        Some(UplinkCommandLoad {
            load_identity: 0x12b5_0002,
            package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
            plan_identity: full_gnss_loss_mission_plan().plan_identity,
            abi: FlightAbiId::GlobalKlr10V1,
            source_estimator_identity: GROUND_ESTIMATOR_ID,
            source_estimator_checksum: self
                .latest_ground
                .map_or(0x12b5_1002, |value| value.estimator_checksum),
            stage_epoch,
            not_before_epoch: stage_epoch + 2,
            expires_epoch: effective_epoch + 64,
            requested_effective_epoch: effective_epoch,
            required_capabilities: PACKAGE_CAP_BRANCH_SELECT,
            prerequisite_event_mask: 0,
            position_residual_limit_q12: 0,
            velocity_residual_limit_q24: 0,
            frame: flight.navigation.frame,
            load_type: UplinkLoadType::ContingencyBranch,
            arguments,
        })
    }

    fn complete(&mut self) -> Result<(), MissionSessionError> {
        let data = fixtures();
        let runner = self.runner.as_ref().ok_or(MissionSessionError::Lifecycle)?;
        let run = runner.completed_summary().map_err(world_error)?;
        let config = self.flight_config.ok_or(MissionSessionError::Lifecycle)?;
        let summary = adapt_global_result(
            GlobalEvaluationRequest {
                earth: &data.earth,
                transforms: &data.transforms,
                atmosphere: &data.atmosphere,
                vehicle: &data.vehicle,
                mission: data.mission,
                avionics: config,
                uncertainty: GlobalSensorFaults {
                    gnss_dropout_from_release: GNSS_LOSS_RELEASE as u16,
                    gnss_dropout_until_release: u16::MAX,
                    ..GlobalSensorFaults::NONE
                },
                case_seed: FULL_MISSION_CASE_SEED,
            },
            run,
        )
        .map_err(world_error)?;
        let capture = GlobalMissionCapture {
            telemetry_header: GlobalTelemetryHeader {
                identity: FULL_TELEMETRY_IDENTITY,
                earth_identity: data.earth.identity,
                transform_identity: data.transforms.identity,
                atmosphere_identity: data.atmosphere.identity,
                vehicle_identity: data.vehicle.identity,
                mission_identity: data.mission.identity,
                avionics_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
                case_seed: FULL_MISSION_CASE_SEED,
                telemetry_period_q16: u32::from(PHASE10_RECORD_STRIDE_RELEASES) * 2_048,
                max_mission_time_q16: data.mission.max_mission_time_q16_s,
            },
            plot_identity: FULL_PLOT_IDENTITY,
            frames: self.frames.clone(),
            plot_points: self.plot_points.clone(),
            summary,
            transition_records: run.transition_records,
            releases: self.release_epoch,
            wall_seconds: 0.0,
        };
        let ktt10 = encode_ktt10(&capture).map_err(|_| MissionSessionError::Authoring)?;
        let kph10 = encode_kph10(&capture).map_err(|_| MissionSessionError::Authoring)?;
        let ksr10 = encode_ksr10(&capture).map_err(|_| MissionSessionError::Authoring)?;
        let procedure_state = self
            .procedure
            .as_ref()
            .map_or(ProcedureState::Skipped, ProcedureEngine::state);
        let procedure_chain = self
            .procedure
            .as_ref()
            .and_then(|value| value.evidence().last())
            .map_or(0, |value| value.chain);
        let (journal_bytes, journal_chain) = self.encode_journal()?;
        let prediction_checksum = self
            .latest_flight
            .and_then(|_| runner.package().prediction_summary())
            .map_or(0, |value| value.prediction_checksum);
        let evidence_identity = hash_words(&[
            FULL_GNSS_LOSS_SCENARIO_ID,
            self.release_epoch,
            run.flight.flight_checksum,
            run.flight.navigation.checksum,
            run.command_checksum,
            prediction_checksum,
            procedure_chain,
            journal_chain,
            self.transcript.chain,
            summary.common.outcome as u32,
        ]);
        let evidence = SessionRunEvidence {
            scenario_identity: FULL_GNSS_LOSS_SCENARIO_ID,
            releases: self.release_epoch,
            flight_checksum: run.flight.flight_checksum,
            navigation_checksum: run.flight.navigation.checksum,
            command_checksum: run.command_checksum,
            prediction_checksum,
            procedure_chain,
            journal_chain,
            action_chain: self.transcript.chain,
            rejected_loads: self.rejected_loads,
            safe: run.flight.safe,
            evidence_identity,
            actions: self.transcript.records.clone(),
        };
        let recovered = summary.common.outcome == EvaluationOutcome::GroundContact;
        let alternate = self.selected_branch == 2;
        let disposition = classify_disposition(OperationalDispositionEvidence {
            objective: if !recovered {
                MissionObjectiveDisposition::NotAchieved
            } else if alternate {
                MissionObjectiveDisposition::ContingencyAchieved
            } else {
                MissionObjectiveDisposition::PrimaryAchieved
            },
            vehicle: if !recovered {
                VehicleDisposition::Lost
            } else if alternate {
                VehicleDisposition::Recovered
            } else {
                VehicleDisposition::Nominal
            },
            procedure: if alternate {
                ProcedureDisposition::AlternateBranch
            } else {
                procedure_disposition(procedure_state)
            },
            operator: operator_disposition(&evidence, procedure_state, self.selected_branch),
            avionics: if run.flight.safe {
                AvionicsDisposition::Failed
            } else if alternate {
                AvionicsDisposition::SafeRecovery
            } else {
                AvionicsDisposition::DegradedOperational
            },
            evidence: EvidenceDisposition::Complete,
        });
        let prediction_bytes = encode_predictions(&self.prediction_records)?;
        let ground_bytes = encode_ground(&self.observations, &self.estimates)?;
        let procedure_bytes = serde_json::to_vec(&json!({
            "schema": "ksa64.phase12b.procedure-evidence.v1",
            "state": format!("{:?}", procedure_state),
            "chain": format!("0x{procedure_chain:08x}"),
            "records": self.procedure.as_ref().map(|value| value.evidence().len()).unwrap_or(0)
        }))
        .map_err(|_| MissionSessionError::Authoring)?;
        let debrief_bytes = serde_json::to_vec_pretty(&json!({
            "schema": "ksa64.phase12b.operations-debrief.v1",
            "overall": format!("{:?}", disposition.overall),
            "objective": format!("{:?}", disposition.axes.objective),
            "vehicle": format!("{:?}", disposition.axes.vehicle),
            "procedure": format!("{:?}", disposition.axes.procedure),
            "operator": format!("{:?}", disposition.axes.operator),
            "avionics": format!("{:?}", disposition.axes.avionics),
            "evidence": format!("{:?}", disposition.axes.evidence),
            "mission_can_succeed_off_plan": true,
            "physical_outcome": format!("{:?}", summary.common.outcome),
            "limitations": "engineering simulation only; not launch approval or safety authority"
        }))
        .map_err(|_| MissionSessionError::Authoring)?;
        let bundle = self.build_bundle(
            &evidence,
            &ktt10,
            &kph10,
            &ksr10,
            ground_bytes,
            prediction_bytes,
            journal_bytes,
            procedure_bytes,
            debrief_bytes,
        )?;
        let completed = CompletedMissionSession {
            evidence,
            bundle,
            debrief: None,
        };
        self.completed = Some(FullMissionCompletion {
            session: completed,
            global_summary: summary,
            disposition,
            ktt10,
            kph10,
            ksr10,
        });
        self.lifecycle = MissionSessionLifecycle::Completed;
        self.record(MissionSessionEventKind::Completed, self.release_epoch);
        self.timeline_event(
            self.release_epoch,
            TimelineSource::Evidence,
            0,
            0x12b4_ffff,
            "Mission evidence sealed",
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn build_bundle(
        &self,
        evidence: &SessionRunEvidence,
        ktt10: &[u8],
        kph10: &[u8],
        ksr10: &[u8; KSR10_LENGTH],
        ground: Vec<u8>,
        predictions: Vec<u8>,
        journal: Vec<u8>,
        procedure: Vec<u8>,
        debrief: Vec<u8>,
    ) -> Result<Vec<u8>, MissionSessionError> {
        let mut builder = SessionBundleBuilder::new(SessionBundleIdentity {
            definition: self.project.definition_identity,
            actions: evidence.action_chain.max(1),
            completed_evidence: evidence.evidence_identity.max(1),
        })
        .map_err(|_| MissionSessionError::Authoring)?;
        push_definition_segments(&mut builder, &self.project)
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::GroundObservations, ground)
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::CanonicalTelemetry, ktt10.to_vec())
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::CanonicalTelemetry, kph10.to_vec())
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::CanonicalTelemetry, ksr10.to_vec())
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::PredictionProducts, predictions)
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(
                SessionSegmentKind::ActionLog,
                encode_action_log(&self.project, evidence)
                    .map_err(|_| MissionSessionError::Authoring)?,
            )
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::PackageJournal, journal)
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::ProcedureEvidence, procedure)
            .map_err(|_| MissionSessionError::Authoring)?;
        builder
            .push(SessionSegmentKind::Debrief, debrief)
            .map_err(|_| MissionSessionError::Authoring)?;
        builder.encode().map_err(|_| MissionSessionError::Authoring)
    }

    fn encode_journal(&self) -> Result<(Vec<u8>, u32), MissionSessionError> {
        let runner = self.runner.as_ref().ok_or(MissionSessionError::Lifecycle)?;
        let mut records = [EventJournalRecord::EMPTY; EVENT_JOURNAL_CAPACITY];
        let count = runner.package().recover_journal_after(0, &mut records);
        let mut bytes = vec![0; count * KEJ11_LENGTH];
        for (index, record) in records[..count].iter().enumerate() {
            write_kej11(
                record,
                &mut bytes[index * KEJ11_LENGTH..(index + 1) * KEJ11_LENGTH],
            )
            .map_err(|_| MissionSessionError::Authoring)?;
        }
        Ok((
            bytes,
            count.checked_sub(1).map_or(0, |index| records[index].chain),
        ))
    }

    fn record(&mut self, kind: MissionSessionEventKind, detail_identity: u32) {
        self.events.push(MissionSessionEvent {
            sequence: self.events.len() as u32 + 1,
            release_epoch: self.release_epoch,
            kind,
            detail_identity,
        });
    }

    fn timeline_event(
        &mut self,
        epoch: u32,
        source: TimelineSource,
        severity: u8,
        identity: u32,
        label: &str,
    ) {
        self.timeline.push(TimelineEventView {
            epoch,
            source,
            severity,
            event_identity: identity,
            label: label.to_owned(),
        });
    }
}

pub fn run_full_gnss_loss_scripted_evidence(
    project: &CompiledMissionProject,
) -> Result<SessionRunEvidence, crate::phase11_authoring::AuthoringError> {
    let mut session = FullMissionSession::compiled(project.clone())
        .map_err(|_| crate::phase11_authoring::AuthoringError::Compatibility)?;
    session
        .prepare()
        .map_err(|_| crate::phase11_authoring::AuthoringError::Replay)?;
    session
        .run_scripted_to_completion()
        .map(|value| value.session.evidence)
        .map_err(|_| crate::phase11_authoring::AuthoringError::Replay)
}

fn encode_ground(
    observations: &[GroundTrackingObservation],
    estimates: &[GroundEstimate],
) -> Result<Vec<u8>, MissionSessionError> {
    let mut output =
        Vec::with_capacity(observations.len() * KGO11_LENGTH + estimates.len() * KGE11_LENGTH);
    for observation in observations {
        let mut bytes = vec![0; KGO11_LENGTH];
        write_kgo11(observation, &mut bytes).map_err(|_| MissionSessionError::Authoring)?;
        output.extend_from_slice(&bytes);
    }
    for estimate in estimates {
        let mut bytes = vec![0; KGE11_LENGTH];
        write_kge11(estimate, &mut bytes).map_err(|_| MissionSessionError::Authoring)?;
        output.extend_from_slice(&bytes);
    }
    Ok(output)
}

fn encode_predictions(predictions: &[HostPrediction]) -> Result<Vec<u8>, MissionSessionError> {
    let mut output = Vec::new();
    for prediction in predictions {
        let mut header = vec![0; KPP11_HEADER_LENGTH];
        write_kpp11_header(&prediction.header, &mut header)
            .map_err(|_| MissionSessionError::Authoring)?;
        output.extend_from_slice(&header);
        for point in &prediction.points {
            let mut bytes = vec![0; KPP11_POINT_LENGTH];
            write_kpp11_point(point, &mut bytes).map_err(|_| MissionSessionError::Authoring)?;
            output.extend_from_slice(&bytes);
        }
    }
    Ok(output)
}

fn maximum_difference(left: [i32; 3], right: [i32; 3]) -> i32 {
    left.into_iter()
        .zip(right)
        .map(|(a, b)| (i64::from(a) - i64::from(b)).unsigned_abs())
        .max()
        .unwrap_or(0)
        .min(i32::MAX as u64) as i32
}

fn interface_frame(frame: ReferenceFrameId) -> GlobalFrameId {
    match frame {
        ReferenceFrameId::LocalEnuV1 => GlobalFrameId::LocalEnuV1,
        ReferenceFrameId::EarthFixedEcefV1 => GlobalFrameId::EarthFixedEcefV1,
        ReferenceFrameId::EarthInertialEciV1 => GlobalFrameId::EarthInertialEciV1,
    }
}

fn procedure_disposition(state: ProcedureState) -> ProcedureDisposition {
    match state {
        ProcedureState::Active | ProcedureState::Skipped => ProcedureDisposition::Skipped,
        ProcedureState::Completed => ProcedureDisposition::Completed,
        ProcedureState::Failed => ProcedureDisposition::Failed,
        ProcedureState::Mistimed => ProcedureDisposition::Mistimed,
        ProcedureState::ManuallyOverridden => ProcedureDisposition::Overridden,
    }
}

fn operator_disposition(
    evidence: &SessionRunEvidence,
    state: ProcedureState,
    selected_branch: u8,
) -> OperatorDisposition {
    let committed = evidence
        .actions
        .iter()
        .filter(|record| record.state == UplinkState::Committed)
        .collect::<Vec<_>>();
    if committed.is_empty() {
        if evidence.rejected_loads != 0 {
            OperatorDisposition::RejectedAction
        } else {
            OperatorDisposition::NoAction
        }
    } else if committed
        .iter()
        .any(|record| record.epoch > DECISION_WINDOW_CLOSE_RELEASE)
        || state != ProcedureState::Completed
    {
        OperatorDisposition::DelayedValid
    } else if selected_branch == 2 {
        OperatorDisposition::TimelyAlternate
    } else {
        OperatorDisposition::TimelyReference
    }
}

fn action_label(record: UplinkControlRecord) -> &'static str {
    match record.state {
        UplinkState::Staged => "Command load staged",
        UplinkState::Committed => "Command load committed",
        UplinkState::Cancelled => "Command load cancelled",
        UplinkState::Executed => "Command load executed",
        UplinkState::Rejected => "Command load rejected",
        UplinkState::Expired => "Command load expired",
        UplinkState::Empty => "No command load",
    }
}

fn hash_load(load: &UplinkCommandLoad) -> u32 {
    hash_words(&[
        load.load_identity,
        load.stage_epoch,
        load.requested_effective_epoch,
        load.load_type as u32,
        load.source_estimator_checksum,
    ])
}

fn hash_words(values: &[u32]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for value in values {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    hash.max(1)
}

fn world_error(_: GlobalWorldError) -> MissionSessionError {
    MissionSessionError::Lifecycle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase11_authoring::complete_project_session;
    use crate::phase11_session::verify_complete_session;

    fn advance_to(session: &mut FullMissionSession, release_epoch: u32) {
        session.set_pace(MissionSessionPace::Fast).unwrap();
        while session.release_epoch < release_epoch {
            session
                .advance_bounded((release_epoch - session.release_epoch).min(256))
                .unwrap();
        }
        assert_eq!(session.release_epoch, release_epoch);
    }

    fn journal_snapshot(session: &FullMissionSession) -> Vec<EventJournalRecord> {
        let mut records = [EventJournalRecord::EMPTY; EVENT_JOURNAL_CAPACITY];
        let count = session
            .runner
            .as_ref()
            .unwrap()
            .package()
            .recover_journal_after(0, &mut records);
        records[..count].to_vec()
    }

    #[test]
    fn accepted_nominal_reference_trajectory_is_strict_and_truth_free() {
        let reference = accepted_nominal_reference_trajectory().unwrap();
        assert_eq!(
            reference.path_identity,
            crate::phase10_mission::PHASE10_PLOT_IDENTITY
        );
        assert_ne!(reference.evaluation_identity, 0);
        assert_eq!(reference.cadence_releases, PHASE10_RECORD_STRIDE_RELEASES);
        assert_eq!(reference.points.len(), 697);
        assert_eq!(reference.artifact_crc32, crc32_ieee(ACCEPTED_NOMINAL_KPH10));
        assert!(reference
            .points
            .windows(2)
            .all(|points| { points[0].release_epoch < points[1].release_epoch }));
        assert!(reference.points.iter().all(|point| matches!(
            point.frame,
            ReferenceFrameId::LocalEnuV1
                | ReferenceFrameId::EarthFixedEcefV1
                | ReferenceFrameId::EarthInertialEciV1
        )));

        let mut corrupt_point = ACCEPTED_NOMINAL_KPH10.to_vec();
        corrupt_point[KPH10_HEADER_LENGTH + 7] ^= 1;
        assert_eq!(
            parse_accepted_nominal_reference_trajectory(&corrupt_point),
            Err(PlannedTrajectoryError::Point)
        );
        assert_eq!(
            parse_accepted_nominal_reference_trajectory(
                &ACCEPTED_NOMINAL_KPH10[..ACCEPTED_NOMINAL_KPH10.len() - 1]
            ),
            Err(PlannedTrajectoryError::Length)
        );

        let mut valid_but_unaccepted = ACCEPTED_NOMINAL_KPH10.to_vec();
        let point_range = KPH10_HEADER_LENGTH..KPH10_HEADER_LENGTH + KPH10_POINT_LENGTH;
        let mut changed =
            GlobalPlotPoint::decode(&valid_but_unaccepted[point_range.clone()]).unwrap();
        changed.altitude_q12_km = changed.altitude_q12_km.saturating_add(1);
        let mut changed_bytes = [0; KPH10_POINT_LENGTH];
        changed.encode(&mut changed_bytes).unwrap();
        valid_but_unaccepted[point_range].copy_from_slice(&changed_bytes);
        assert_eq!(
            parse_accepted_nominal_reference_trajectory(&valid_but_unaccepted),
            Err(PlannedTrajectoryError::ArtifactHash)
        );
    }

    #[test]
    fn full_session_starts_realtime_and_qualifies_gnss_loss_on_schedule() {
        let mut session = FullMissionSession::new(OperationalRole::GuidedOperator).unwrap();
        session.prepare().unwrap();
        assert_eq!(session.snapshot().pace, MissionSessionPace::Realtime);
        session.set_pace(MissionSessionPace::Fast).unwrap();
        while session.release_epoch <= GNSS_QUALIFIED_RELEASE {
            session.advance_bounded(256).unwrap();
        }
        assert!(session.gnss_missing_fixes >= 3);
        assert!(session
            .timeline
            .iter()
            .any(|value| value.event_identity == 0x12b4_1002));
        while session.release_epoch <= UPDATE_STAGE_RELEASE {
            session.advance_bounded(256).unwrap();
        }
        let residual = maximum_difference(
            session.latest_flight.unwrap().navigation.position_q12,
            session.latest_ground.unwrap().position_q12_km,
        );
        assert_eq!(
            session.procedure.as_ref().unwrap().current_step(),
            4,
            "residual={residual}"
        );
        assert!(session.recommended_load().is_some());
        assert!(session.snapshot().flight.is_some());
    }

    #[test]
    fn observer_cannot_stage_an_action_or_mutate_the_package() {
        let mut session = FullMissionSession::new(OperationalRole::Observer).unwrap();
        session.prepare().unwrap();
        advance_to(&mut session, UPDATE_STAGE_RELEASE);
        let load = session
            .recommended_load()
            .expect("the public procedure offers the ground update");
        let journal_before = journal_snapshot(&session);
        let events_before = session.events.len();

        assert_eq!(
            session.submit_operator_action(MissionOperatorAction::Stage {
                load,
                completed_event_mask: 0,
            }),
            Err(MissionSessionError::ActionRejected)
        );
        assert!(session.staged_load.is_none());
        assert!(session.transcript.records.is_empty());
        assert_eq!(session.rejected_loads, 0);
        assert_eq!(session.events.len(), events_before);
        assert_eq!(journal_snapshot(&session), journal_before);
    }

    #[test]
    fn ground_update_must_match_current_estimator_evidence_exactly() {
        let mut session = FullMissionSession::new(OperationalRole::GuidedOperator).unwrap();
        session.prepare().unwrap();
        advance_to(&mut session, UPDATE_STAGE_RELEASE);
        let original = session.recommended_load().unwrap();

        let mut forged_state = original;
        forged_state.arguments[0] = forged_state.arguments[0].saturating_add(1);
        assert_eq!(
            session.submit_operator_action(MissionOperatorAction::Stage {
                load: forged_state,
                completed_event_mask: 0,
            }),
            Err(MissionSessionError::ActionRejected)
        );

        let mut forged_source = original;
        forged_source.source_estimator_checksum ^= 1;
        assert_eq!(
            session.submit_operator_action(MissionOperatorAction::Stage {
                load: forged_source,
                completed_event_mask: 0,
            }),
            Err(MissionSessionError::ActionRejected)
        );
        assert!(session.staged_load.is_none());
        assert!(session.transcript.records.is_empty());
    }

    #[test]
    fn expired_staged_load_cannot_be_committed_later() {
        let mut session = FullMissionSession::new(OperationalRole::GuidedOperator).unwrap();
        session.prepare().unwrap();
        advance_to(&mut session, UPDATE_STAGE_RELEASE);
        let mut load = session.recommended_load().unwrap();
        load.requested_effective_epoch = session.release_epoch + 2;
        load.expires_epoch = load.requested_effective_epoch;
        let receipt = session
            .submit_operator_action(MissionOperatorAction::Stage {
                load,
                completed_event_mask: 0,
            })
            .unwrap();
        assert!(receipt.accepted);
        let commit = session.commit_request_for_staged().unwrap();

        advance_to(&mut session, load.expires_epoch + 2);
        assert!(session.staged_load.is_none());
        assert_eq!(
            session.submit_operator_action(MissionOperatorAction::Commit(commit)),
            Err(MissionSessionError::ActionUnavailable)
        );
        assert_eq!(
            session
                .runner
                .as_ref()
                .unwrap()
                .package()
                .frozen_inner()
                .navigation()
                .checksum,
            session.latest_flight.unwrap().navigation.checksum
        );
        assert!(session.timeline.iter().any(|event| {
            event.event_identity == load.load_identity
                && event.label == "Staged command load expired without commit"
        }));
    }

    #[test]
    fn role_filtered_samples_expose_truth_only_to_sim_director() {
        let mut guided = FullMissionSession::new(OperationalRole::GuidedOperator).unwrap();
        let mut director = FullMissionSession::new(OperationalRole::SimDirector).unwrap();
        guided.prepare().unwrap();
        director.prepare().unwrap();
        guided.advance_one_release().unwrap();
        director.advance_one_release().unwrap();

        let guided_sample = guided.snapshot().current_sample.unwrap();
        let director_sample = director.snapshot().current_sample.unwrap();
        assert_eq!(guided_sample.flags & 1, 0);
        assert_eq!(guided_sample.speed_q24_km_s, 0);
        assert_eq!(director_sample.flags & 1, 1);
        assert_eq!(guided.role(), OperationalRole::GuidedOperator);
        assert_eq!(director.role(), OperationalRole::SimDirector);
        assert_eq!(
            guided.latest_flight.unwrap(),
            director.latest_flight.unwrap(),
            "role-filtering must not alter the accepted avionics path"
        );
        assert_eq!(
            guided.runner.as_ref().unwrap().world().snapshot().unwrap(),
            director
                .runner
                .as_ref()
                .unwrap()
                .world()
                .snapshot()
                .unwrap(),
            "role-filtering must not alter authoritative world state"
        );
    }

    #[test]
    fn full_mission_identity_excludes_runtime_role_and_hints() {
        let guided = compile_project_source(FULL_GNSS_LOSS_SOURCE).unwrap();
        let scripted_source = FULL_GNSS_LOSS_SOURCE
            .replace("guided-operator", "scripted-operator")
            .replace("\"hints\": true", "\"hints\": false");
        let scripted = compile_project_source(&scripted_source).unwrap();
        assert_eq!(guided.definition_identity, scripted.definition_identity);
        assert_eq!(guided.definition_pack, scripted.definition_pack);
        assert_eq!(guided.canonical_source, scripted.canonical_source);
        assert_ne!(guided.role, scripted.role);
    }

    #[test]
    fn hints_are_enabled_only_for_the_guided_operator() {
        for role in [
            OperationalRole::Observer,
            OperationalRole::FlightController,
            OperationalRole::FlightSoftwareEngineer,
            OperationalRole::SimDirector,
            OperationalRole::ScriptedOperator,
        ] {
            assert!(!FullMissionSession::new(role).unwrap().hints_enabled());
        }
        assert!(FullMissionSession::new(OperationalRole::GuidedOperator)
            .unwrap()
            .hints_enabled());
    }

    #[test]
    #[ignore = "full Phase 12B mission acceptance"]
    fn scripted_full_mission_seals_exact_evidence_and_succeeds() {
        let mut session = FullMissionSession::new(OperationalRole::ScriptedOperator).unwrap();
        session.prepare().unwrap();
        let completed = session.run_scripted_to_completion().unwrap();
        assert_eq!(
            completed.global_summary.common.outcome,
            EvaluationOutcome::GroundContact
        );
        assert_eq!(
            completed.disposition.axes.objective,
            MissionObjectiveDisposition::PrimaryAchieved
        );
        assert!(
            verify_complete_session(&completed.session.bundle)
                .unwrap()
                .sealed
        );
        assert_eq!(
            completed.session.evidence.actions.len(),
            4,
            "{:?}",
            completed.session.evidence.actions
        );
        assert_eq!(completed.session.evidence.releases, 21_591);
        assert_eq!(completed.session.bundle.len(), 2_911_464);
        assert_eq!(
            crate::phase11_session::sha256(&completed.session.bundle),
            [
                0x75, 0x54, 0x11, 0x1f, 0x28, 0xd8, 0xf3, 0x62, 0x8a, 0xe3, 0xca, 0x9d, 0x06, 0x9f,
                0xad, 0x34, 0x20, 0x4e, 0x12, 0xf8, 0x62, 0x52, 0xef, 0xd0, 0x0e, 0xcf, 0x74, 0x4c,
                0x0e, 0xe0, 0xfc, 0xd4,
            ]
        );
        assert_eq!(completed.ktt10.len(), 175_232);
        assert_eq!(
            crate::phase11_session::sha256(&completed.ktt10),
            [
                0x45, 0x6c, 0x51, 0x28, 0x25, 0x38, 0x8b, 0x7d, 0xf1, 0xd6, 0x5c, 0x1f, 0xa8, 0xf0,
                0x8a, 0x0c, 0x08, 0x6c, 0x4b, 0xe7, 0x94, 0xc6, 0x91, 0x2c, 0xc7, 0xe1, 0x22, 0x3c,
                0xd4, 0x06, 0xe2, 0xe1,
            ]
        );
        assert_eq!(completed.kph10.len(), 32_896);
        assert_eq!(
            crate::phase11_session::sha256(&completed.kph10),
            [
                0xce, 0xf0, 0x9c, 0x40, 0xf9, 0x5f, 0xd7, 0x5f, 0x52, 0xec, 0x7a, 0x15, 0xf8, 0xe9,
                0xdb, 0x0e, 0x12, 0xf9, 0xd2, 0xff, 0xd1, 0x2b, 0x6c, 0x10, 0x7b, 0xbc, 0x4c, 0x6c,
                0xfb, 0x85, 0x32, 0x23,
            ]
        );
        assert_eq!(
            crate::phase11_session::sha256(&completed.ksr10),
            [
                0x6a, 0xee, 0x34, 0x46, 0x1c, 0xc0, 0xda, 0x65, 0xb7, 0x9b, 0xa1, 0x95, 0x4a, 0x48,
                0xa6, 0xad, 0x90, 0x80, 0x3d, 0x29, 0x85, 0x7b, 0xf4, 0x44, 0xa5, 0x39, 0x98, 0xae,
                0x9d, 0xe6, 0x22, 0xd1,
            ]
        );
        assert_eq!(
            completed.disposition.overall,
            crate::phase12b::OverallMissionDisposition::DegradedSuccess
        );
    }

    #[test]
    #[ignore = "full Phase 12B mission acceptance"]
    fn no_action_can_finish_as_degraded_success() {
        let mut session = FullMissionSession::new(OperationalRole::Observer).unwrap();
        session.prepare().unwrap();
        session.set_pace(MissionSessionPace::Fast).unwrap();
        while session.lifecycle() != MissionSessionLifecycle::Completed {
            session.advance_bounded(256).unwrap();
        }
        let completed = session.finish().unwrap();
        assert_eq!(
            completed.global_summary.common.outcome,
            EvaluationOutcome::GroundContact
        );
        assert_eq!(
            completed.disposition.overall,
            crate::phase12b::OverallMissionDisposition::DegradedSuccess
        );
        assert!(completed.session.evidence.actions.is_empty());
    }
    #[test]
    #[ignore = "full Phase 12B alternate-branch acceptance"]
    fn safe_recovery_branch_completes_as_contingency_success() {
        let mut session = FullMissionSession::new(OperationalRole::ScriptedOperator).unwrap();
        session.prepare().unwrap();
        session.set_pace(MissionSessionPace::Fast).unwrap();
        while session.lifecycle() != MissionSessionLifecycle::Completed {
            match session.release_epoch {
                UPDATE_STAGE_RELEASE => {
                    let load = session.recommended_load().unwrap();
                    session
                        .submit_operator_action(MissionOperatorAction::Stage {
                            load,
                            completed_event_mask: 0,
                        })
                        .unwrap();
                }
                UPDATE_COMMIT_RELEASE | BRANCH_COMMIT_RELEASE => {
                    let request = session.commit_request_for_staged().unwrap();
                    session
                        .submit_operator_action(MissionOperatorAction::Commit(request))
                        .unwrap();
                }
                BRANCH_STAGE_RELEASE => {
                    let load = session.safe_recovery_load().unwrap();
                    session
                        .submit_operator_action(MissionOperatorAction::Stage {
                            load,
                            completed_event_mask: 0,
                        })
                        .unwrap();
                }
                _ => {}
            }
            session.advance_internal(false).unwrap();
        }
        let completed = session.finish().unwrap();
        assert_eq!(
            completed.disposition.overall,
            crate::phase12b::OverallMissionDisposition::ContingencySuccess
        );
        assert_eq!(
            completed.disposition.axes.objective,
            MissionObjectiveDisposition::ContingencyAchieved
        );
        assert_eq!(
            completed.disposition.axes.vehicle,
            VehicleDisposition::Recovered
        );
        assert_eq!(
            completed.disposition.axes.procedure,
            ProcedureDisposition::AlternateBranch
        );
        assert_eq!(
            completed.disposition.axes.operator,
            OperatorDisposition::TimelyAlternate
        );
        assert_eq!(
            completed.disposition.axes.avionics,
            AvionicsDisposition::SafeRecovery
        );
    }

    #[test]
    #[ignore = "two full Phase 12B executions prove role-neutral replay parity"]
    fn guided_and_scripted_action_transcripts_are_byte_identical() {
        let mut guided = FullMissionSession::new(OperationalRole::GuidedOperator).unwrap();
        guided.prepare().unwrap();
        let guided = guided.run_scripted_to_completion().unwrap();

        let mut scripted = FullMissionSession::new(OperationalRole::ScriptedOperator).unwrap();
        scripted.prepare().unwrap();
        let scripted = scripted.run_scripted_to_completion().unwrap();

        assert_eq!(guided.session.evidence, scripted.session.evidence);
        assert_eq!(guided.session.bundle, scripted.session.bundle);
    }

    #[test]
    #[ignore = "two full Phase 12B executions prove SDK/direct bundle parity"]
    fn full_authoring_sdk_bundle_matches_direct_full_session() {
        let project = compile_project_source(FULL_GNSS_LOSS_SOURCE).unwrap();
        let mut direct = FullMissionSession::compiled(project.clone()).unwrap();
        direct.prepare().unwrap();
        let direct = direct.run_scripted_to_completion().unwrap();
        let sdk = complete_project_session(&project, true).unwrap();
        assert_eq!(sdk.evidence, direct.session.evidence);
        assert_eq!(sdk.bundle, direct.session.bundle);
    }
}
