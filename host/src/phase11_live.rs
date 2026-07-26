//! Incremental deterministic Phase 11 operations session for host application clients.
//!
//! This is an application orchestration boundary, not a second simulator. It
//! advances the accepted flight package exactly one 32 Hz release at a time,
//! exposes truth-blind operational snapshots, and finalizes through KSB11.

use crate::phase11_authoring::{
    complete_project_session_from_evidence, from_operational, AuthoringError,
    CompiledMissionProject, CompletedMissionSession, MissionPackage, MissionScenario,
    SessionRunEvidence,
};
use crate::phase11_operations::{gnss_loss_procedure_pack, ProcedureEngine, ProcedureState};
use crate::phase11_scenarios::{
    coast_config, coast_fast, commit_request, finish_evidence, ground_navigation_load,
    operational_snapshot, ActionTranscript, GNSS_LOSS_SCENARIO_ID,
};
use ksa64_flight::phase10::GlobalFlightEvidence;
use ksa64_flight::phase11::{
    ksa_g10r_reference_mission_plan, GlobalKlr10FlightPackage, KsaG10rReferenceOpsV1,
};
use ksa64_interface::phase10::GlobalFrameId;
use ksa64_interface::phase11::{
    GroundEstimate, PredictionSummary, UplinkCommandLoad, UplinkControlRecord, UplinkLoadType,
    UplinkState,
};
use ksa64_sim::phase11::{synthesize_ground_observation, GroundEstimator, GroundTruthSample};

pub const LIVE_RELEASE_PERIOD_MICROS: u32 = 31_250;
pub const LIVE_GNSS_LOSS_MAX_RELEASES: u32 = 322;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveMissionCapability {
    pub adapter_identity: u32,
    pub release_hz: u16,
    pub bounded_step: bool,
    pub operator_actions: bool,
    pub deterministic_finalization: bool,
}

pub const GNSS_LOSS_LIVE_CAPABILITY: LiveMissionCapability = LiveMissionCapability {
    adapter_identity: 0x1150_0001,
    release_hz: 32,
    bounded_step: true,
    operator_actions: true,
    deterministic_finalization: true,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionSessionLifecycle {
    Compiled,
    Ready,
    Running,
    Paused,
    Completed,
    Aborted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionSessionPace {
    Fast,
    Realtime,
    Paused,
    SingleStep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionSessionEventKind {
    Compiled,
    Prepared,
    Release,
    Paused,
    Resumed,
    PaceChanged,
    ActionStaged,
    ActionCommitted,
    ActionCancelled,
    ActionRejected,
    Completed,
    Aborted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionSessionEvent {
    pub sequence: u32,
    pub release_epoch: u32,
    pub kind: MissionSessionEventKind,
    pub detail_identity: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionSessionSnapshot {
    pub definition_identity: u32,
    pub lifecycle: MissionSessionLifecycle,
    pub pace: MissionSessionPace,
    pub release_epoch: u32,
    pub release_period_micros: u32,
    pub frame: Option<GlobalFrameId>,
    pub mission_time_q16: Option<u32>,
    pub navigation_position_q12: Option<[i32; 3]>,
    pub navigation_velocity_q24: Option<[i32; 3]>,
    pub flight_checksum: Option<u32>,
    pub navigation_checksum: Option<u32>,
    pub command_checksum: Option<u32>,
    pub prediction: Option<PredictionSummary>,
    pub evidence_identity: Option<u32>,
    pub procedure_chain: u32,
    pub journal_chain: u32,
    pub action_chain: u32,
    pub procedure_state: ProcedureState,
    pub procedure_step: u16,
    pub staged_load_identity: Option<u32>,
    pub action_count: usize,
    pub event_count: usize,
    pub rejected_loads: u16,
    pub safe: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionOperatorAction {
    Stage {
        load: UplinkCommandLoad,
        completed_event_mask: u32,
    },
    Commit(UplinkControlRecord),
    Cancel(UplinkControlRecord),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionActionReceipt {
    pub record: UplinkControlRecord,
    pub accepted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionSessionError {
    Unsupported,
    Lifecycle,
    ActionUnavailable,
    ActionRejected,
    Procedure,
    NotCompleted,
    Authoring,
}

impl From<AuthoringError> for MissionSessionError {
    fn from(_: AuthoringError) -> Self {
        Self::Authoring
    }
}

struct GnssLossRuntime {
    package: KsaG10rReferenceOpsV1,
    procedure: ProcedureEngine,
    ground: Option<GroundEstimate>,
    final_flight: Option<GlobalFlightEvidence>,
    transcript: ActionTranscript,
    releases: u32,
    rejected_loads: u16,
    staged_load: Option<UplinkCommandLoad>,
}

impl GnssLossRuntime {
    fn new() -> Result<Self, MissionSessionError> {
        let mut package =
            KsaG10rReferenceOpsV1::new(coast_config()).ok_or(MissionSessionError::Unsupported)?;
        let plan = ksa_g10r_reference_mission_plan();
        if !package.initialize_mission_plan(plan) {
            return Err(MissionSessionError::Unsupported);
        }
        let procedure = ProcedureEngine::new(gnss_loss_procedure_pack(plan.plan_identity), 0)
            .map_err(|_| MissionSessionError::Procedure)?;
        Ok(Self {
            package,
            procedure,
            ground: None,
            final_flight: None,
            transcript: ActionTranscript::new(),
            releases: 0,
            rejected_loads: 0,
            staged_load: None,
        })
    }

    fn advance_one_release(&mut self) -> Result<bool, MissionSessionError> {
        if self.releases >= LIVE_GNSS_LOSS_MAX_RELEASES {
            return Err(MissionSessionError::Lifecycle);
        }
        let epoch = u16::try_from(self.releases).map_err(|_| MissionSessionError::Lifecycle)?;
        let flight = self
            .package
            .process_release(Some(coast_fast(epoch)), None, None);
        if epoch == 0 {
            let observation = synthesize_ground_observation(
                GroundTruthSample {
                    epoch: 0,
                    frame: flight.navigation.frame,
                    position_q12_km: flight.navigation.position_q12,
                    velocity_q24_km_s: flight.navigation.velocity_q24,
                },
                0,
                0x4b53_4111,
            );
            let mut estimator = GroundEstimator::new();
            let ground = estimator
                .update(observation, 0)
                .ok_or(MissionSessionError::Procedure)?;
            let snapshot = operational_snapshot(0, &flight, ground.position_q12_km);
            for _ in 0..3 {
                self.procedure
                    .tick(&snapshot)
                    .map_err(|_| MissionSessionError::Procedure)?;
            }
            self.ground = Some(ground);
        }
        self.final_flight = Some(flight);
        self.releases += 1;

        if self.releases >= 9 {
            let ground = self.ground.ok_or(MissionSessionError::Procedure)?;
            let snapshot = operational_snapshot(
                self.releases,
                self.final_flight
                    .as_ref()
                    .ok_or(MissionSessionError::Procedure)?,
                ground.position_q12_km,
            );
            self.procedure
                .tick(&snapshot)
                .map_err(|_| MissionSessionError::Procedure)?;
        }
        Ok(matches!(
            self.procedure.state(),
            ProcedureState::Completed | ProcedureState::Failed | ProcedureState::ManuallyOverridden
        ))
    }

    fn recommended_load(&self) -> Option<UplinkCommandLoad> {
        if self.staged_load.is_some() {
            return None;
        }
        match self.procedure.current_step() {
            4 if self.releases == 1 => {
                Some(ground_navigation_load(1, 4, 0x11c0_2001, self.ground?))
            }
            5 if self.releases == 5 => Some(crate::phase11_scenarios::generic_load(
                5,
                8,
                0x11c0_2002,
                UplinkLoadType::ContingencyBranch,
                ksa64_interface::phase11::PACKAGE_CAP_BRANCH_SELECT,
                self.final_flight?.navigation.frame,
                [1, 0, 0, 0],
            )),
            _ => None,
        }
    }

    fn submit(
        &mut self,
        role: ksa64_interface::phase11::OperationalRole,
        action: MissionOperatorAction,
    ) -> Result<MissionActionReceipt, MissionSessionError> {
        let epoch = self.releases;
        let step = self.procedure.current_step();
        let record = match action {
            MissionOperatorAction::Stage {
                load,
                completed_event_mask,
            } => {
                let record = self
                    .package
                    .stage_uplink(load, completed_event_mask)
                    .ok_or(MissionSessionError::ActionRejected)?;
                self.transcript.record(epoch, step, record);
                if record.state == UplinkState::Staged {
                    self.staged_load = Some(load);
                } else {
                    self.rejected_loads = self.rejected_loads.saturating_add(1);
                }
                record
            }
            MissionOperatorAction::Commit(request) => {
                let load = self
                    .staged_load
                    .ok_or(MissionSessionError::ActionUnavailable)?;
                let record = self
                    .package
                    .commit_uplink(&request)
                    .ok_or(MissionSessionError::ActionRejected)?;
                self.transcript.record(epoch, step, record);
                if record.state == UplinkState::Committed {
                    self.procedure
                        .accept_action(role, epoch, load.load_type, true)
                        .map_err(|_| MissionSessionError::Procedure)?;
                    self.staged_load = None;
                } else {
                    self.rejected_loads = self.rejected_loads.saturating_add(1);
                }
                record
            }
            MissionOperatorAction::Cancel(request) => {
                let record = self
                    .package
                    .cancel_uplink(&request)
                    .ok_or(MissionSessionError::ActionRejected)?;
                self.transcript.record(epoch, step, record);
                if record.state == UplinkState::Cancelled {
                    self.staged_load = None;
                }
                record
            }
        };
        Ok(MissionActionReceipt {
            record,
            accepted: matches!(
                record.state,
                UplinkState::Staged | UplinkState::Committed | UplinkState::Cancelled
            ),
        })
    }

    fn commit_request_for_staged(&self) -> Option<UplinkControlRecord> {
        self.staged_load
            .map(|load| commit_request(load, self.releases))
    }

    fn finish_evidence(&self) -> Result<SessionRunEvidence, MissionSessionError> {
        if !matches!(
            self.procedure.state(),
            ProcedureState::Completed | ProcedureState::Failed | ProcedureState::ManuallyOverridden
        ) {
            return Err(MissionSessionError::NotCompleted);
        }
        let procedure_chain = self
            .procedure
            .evidence()
            .last()
            .map_or(0, |record| record.chain);
        Ok(from_operational(finish_evidence(
            GNSS_LOSS_SCENARIO_ID,
            self.releases,
            self.final_flight.ok_or(MissionSessionError::NotCompleted)?,
            &self.package,
            self.procedure.state(),
            procedure_chain,
            self.transcript.clone(),
            self.rejected_loads,
        )))
    }
}

fn current_journal_chain(runtime: &GnssLossRuntime) -> u32 {
    let mut journal = [ksa64_interface::phase11::EventJournalRecord::EMPTY;
        ksa64_flight::phase11::EVENT_JOURNAL_CAPACITY];
    let count = runtime.package.recover_journal_after(0, &mut journal);
    count.checked_sub(1).map_or(0, |index| journal[index].chain)
}

pub struct LiveMissionSession {
    project: CompiledMissionProject,
    lifecycle: MissionSessionLifecycle,
    pace: MissionSessionPace,
    resume_pace: MissionSessionPace,
    runtime: Option<GnssLossRuntime>,
    completed_evidence: Option<SessionRunEvidence>,
    events: Vec<MissionSessionEvent>,
    telemetry: Vec<MissionSessionSnapshot>,
}

impl LiveMissionSession {
    pub fn compiled(project: CompiledMissionProject) -> Result<Self, MissionSessionError> {
        if project.package != MissionPackage::ReferenceOps
            || project.scenario != MissionScenario::GnssLoss
        {
            return Err(MissionSessionError::Unsupported);
        }
        let mut session = Self {
            project,
            lifecycle: MissionSessionLifecycle::Compiled,
            pace: MissionSessionPace::Fast,
            resume_pace: MissionSessionPace::Fast,
            runtime: None,
            completed_evidence: None,
            events: Vec::new(),
            telemetry: Vec::new(),
        };
        session.record(
            MissionSessionEventKind::Compiled,
            session.project.definition_identity,
        );
        Ok(session)
    }

    pub fn prepare(&mut self) -> Result<(), MissionSessionError> {
        if self.lifecycle != MissionSessionLifecycle::Compiled {
            return Err(MissionSessionError::Lifecycle);
        }
        self.runtime = Some(GnssLossRuntime::new()?);
        self.lifecycle = MissionSessionLifecycle::Ready;
        self.record(
            MissionSessionEventKind::Prepared,
            self.project.definition_identity,
        );
        Ok(())
    }

    pub fn lifecycle(&self) -> MissionSessionLifecycle {
        self.lifecycle
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
            MissionSessionPace::Fast
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

    pub fn advance_one_release(&mut self) -> Result<MissionSessionSnapshot, MissionSessionError> {
        if self.lifecycle == MissionSessionLifecycle::Paused {
            return Err(MissionSessionError::Lifecycle);
        }
        self.advance_internal(false)
    }

    pub fn step_one_release(&mut self) -> Result<MissionSessionSnapshot, MissionSessionError> {
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
    ) -> Result<MissionSessionSnapshot, MissionSessionError> {
        if maximum_releases == 0 {
            return Ok(self.snapshot());
        }
        if self.pace == MissionSessionPace::Paused {
            return Ok(self.snapshot());
        }
        let budget = match self.pace {
            MissionSessionPace::Fast => maximum_releases,
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

    pub fn recommended_load(&self) -> Option<UplinkCommandLoad> {
        self.runtime.as_ref()?.recommended_load()
    }

    pub fn commit_request_for_staged(&self) -> Option<UplinkControlRecord> {
        self.runtime.as_ref()?.commit_request_for_staged()
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
        let receipt = self
            .runtime
            .as_mut()
            .ok_or(MissionSessionError::Lifecycle)?
            .submit(self.project.role, action)?;
        let kind = match receipt.record.state {
            UplinkState::Staged => MissionSessionEventKind::ActionStaged,
            UplinkState::Committed => MissionSessionEventKind::ActionCommitted,
            UplinkState::Cancelled => MissionSessionEventKind::ActionCancelled,
            _ => MissionSessionEventKind::ActionRejected,
        };
        self.record(kind, receipt.record.load_identity);
        Ok(receipt)
    }

    pub fn snapshot(&self) -> MissionSessionSnapshot {
        let runtime = self.runtime.as_ref();
        let flight = runtime.and_then(|value| value.final_flight.as_ref());
        MissionSessionSnapshot {
            definition_identity: self.project.definition_identity,
            lifecycle: self.lifecycle,
            pace: self.pace,
            release_epoch: runtime.map_or(0, |value| value.releases),
            release_period_micros: LIVE_RELEASE_PERIOD_MICROS,
            frame: flight.map(|value| value.navigation.frame),
            mission_time_q16: runtime
                .and_then(|value| value.releases.checked_sub(1))
                .map(|epoch| 4_000_000 + epoch * 2_048),
            navigation_position_q12: flight.map(|value| value.navigation.position_q12),
            navigation_velocity_q24: flight.map(|value| value.navigation.velocity_q24),
            flight_checksum: flight.map(|value| value.flight_checksum),
            navigation_checksum: flight.map(|value| value.navigation.checksum),
            command_checksum: flight.map(|value| value.command.command_checksum),
            prediction: runtime.and_then(|value| value.package.prediction_summary()),
            evidence_identity: self
                .completed_evidence
                .as_ref()
                .map(|value| value.evidence_identity),
            procedure_chain: runtime
                .and_then(|value| value.procedure.evidence().last())
                .map_or(0, |record| record.chain),
            journal_chain: runtime.map_or(0, current_journal_chain),
            action_chain: runtime.map_or(0x811c_9dc5, |value| value.transcript.chain),
            procedure_state: runtime
                .map_or(ProcedureState::Active, |value| value.procedure.state()),
            procedure_step: runtime.map_or(0, |value| value.procedure.current_step()),
            staged_load_identity: runtime
                .and_then(|value| value.staged_load)
                .map(|value| value.load_identity),
            action_count: runtime.map_or(0, |value| value.transcript.records.len()),
            event_count: self.events.len(),
            rejected_loads: runtime.map_or(0, |value| value.rejected_loads),
            safe: flight.map(|value| value.safe),
        }
    }

    pub fn telemetry_after(&self, release_count: u32) -> &[MissionSessionSnapshot] {
        let start = usize::try_from(release_count)
            .unwrap_or(usize::MAX)
            .min(self.telemetry.len());
        &self.telemetry[start..]
    }

    pub fn events_after(&self, sequence: u32) -> &[MissionSessionEvent] {
        let start = usize::try_from(sequence)
            .unwrap_or(usize::MAX)
            .min(self.events.len());
        &self.events[start..]
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

    pub fn finish(self) -> Result<CompletedMissionSession, MissionSessionError> {
        if self.lifecycle != MissionSessionLifecycle::Completed {
            return Err(MissionSessionError::NotCompleted);
        }
        complete_project_session_from_evidence(
            &self.project,
            self.completed_evidence
                .ok_or(MissionSessionError::NotCompleted)?,
        )
        .map_err(Into::into)
    }

    pub fn run_scripted_to_completion(
        mut self,
    ) -> Result<CompletedMissionSession, MissionSessionError> {
        if self.lifecycle == MissionSessionLifecycle::Compiled {
            self.prepare()?;
        }
        while self.lifecycle != MissionSessionLifecycle::Completed {
            if let Some(load) = self.recommended_load() {
                self.submit_operator_action(MissionOperatorAction::Stage {
                    load,
                    completed_event_mask: 0,
                })?;
                let commit = self
                    .commit_request_for_staged()
                    .ok_or(MissionSessionError::ActionUnavailable)?;
                self.submit_operator_action(MissionOperatorAction::Commit(commit))?;
            }
            self.advance_one_release()?;
        }
        self.finish()
    }

    fn advance_internal(
        &mut self,
        single_step: bool,
    ) -> Result<MissionSessionSnapshot, MissionSessionError> {
        if !matches!(
            self.lifecycle,
            MissionSessionLifecycle::Ready
                | MissionSessionLifecycle::Running
                | MissionSessionLifecycle::Paused
        ) {
            return Err(MissionSessionError::Lifecycle);
        }
        self.lifecycle = MissionSessionLifecycle::Running;
        let terminal = self
            .runtime
            .as_mut()
            .ok_or(MissionSessionError::Lifecycle)?
            .advance_one_release()?;
        let epoch = self.runtime.as_ref().map_or(0, |value| value.releases);
        self.record(MissionSessionEventKind::Release, epoch);
        if terminal {
            let evidence = self
                .runtime
                .as_ref()
                .ok_or(MissionSessionError::Lifecycle)?
                .finish_evidence()?;
            self.completed_evidence = Some(evidence);
            self.lifecycle = MissionSessionLifecycle::Completed;
            self.record(MissionSessionEventKind::Completed, epoch);
        } else if single_step {
            self.lifecycle = MissionSessionLifecycle::Paused;
        }
        let snapshot = self.snapshot();
        self.telemetry.push(snapshot.clone());
        Ok(snapshot)
    }

    fn record(&mut self, kind: MissionSessionEventKind, detail_identity: u32) {
        let sequence = self.events.len() as u32 + 1;
        let release_epoch = self.runtime.as_ref().map_or(0, |value| value.releases);
        self.events.push(MissionSessionEvent {
            sequence,
            release_epoch,
            kind,
            detail_identity,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_fixtures::GNSS_LOSS_SOURCE;
    use crate::phase11_authoring::{compile_project_source, complete_project_session};

    fn session() -> LiveMissionSession {
        let project = compile_project_source(GNSS_LOSS_SOURCE).unwrap();
        LiveMissionSession::compiled(project).unwrap()
    }

    #[test]
    fn lifecycle_pause_step_and_abort_are_explicit() {
        let mut live = session();
        assert_eq!(live.lifecycle(), MissionSessionLifecycle::Compiled);
        live.prepare().unwrap();
        assert_eq!(live.lifecycle(), MissionSessionLifecycle::Ready);
        live.pause().unwrap();
        assert_eq!(live.advance_bounded(10).unwrap().release_epoch, 0);
        live.step_one_release().unwrap();
        assert_eq!(live.snapshot().release_epoch, 1);
        assert_eq!(live.telemetry_after(0).len(), 1);
        assert_eq!(live.lifecycle(), MissionSessionLifecycle::Paused);
        live.resume().unwrap();
        live.abort(0xdead_beef).unwrap();
        assert_eq!(live.lifecycle(), MissionSessionLifecycle::Aborted);
        assert_eq!(
            live.events_after(0).last().unwrap().kind,
            MissionSessionEventKind::Aborted
        );
    }

    #[test]
    fn realtime_and_single_step_pacing_are_bounded() {
        let mut live = session();
        live.prepare().unwrap();
        live.set_pace(MissionSessionPace::Realtime).unwrap();
        assert_eq!(live.advance_bounded(100).unwrap().release_epoch, 1);
        live.set_pace(MissionSessionPace::SingleStep).unwrap();
        live.advance_bounded(100).unwrap();
        assert_eq!(live.snapshot().release_epoch, 2);
        assert_eq!(live.lifecycle(), MissionSessionLifecycle::Paused);
    }

    #[test]
    fn scripted_and_manually_submitted_transcripts_finalize_identically() {
        let project = compile_project_source(GNSS_LOSS_SOURCE).unwrap();
        let legacy = complete_project_session(&project, true).unwrap();
        let scripted = LiveMissionSession::compiled(project.clone())
            .unwrap()
            .run_scripted_to_completion()
            .unwrap();
        assert_eq!(scripted, legacy);

        let mut interactive = LiveMissionSession::compiled(project).unwrap();
        interactive.prepare().unwrap();
        while interactive.lifecycle() != MissionSessionLifecycle::Completed {
            if let Some(load) = interactive.recommended_load() {
                interactive
                    .submit_operator_action(MissionOperatorAction::Stage {
                        load,
                        completed_event_mask: 0,
                    })
                    .unwrap();
                let commit = interactive.commit_request_for_staged().unwrap();
                interactive
                    .submit_operator_action(MissionOperatorAction::Commit(commit))
                    .unwrap();
            }
            interactive.advance_one_release().unwrap();
        }
        let interactive = interactive.finish().unwrap();
        assert_eq!(interactive, legacy);
    }

    #[test]
    fn staged_actions_can_be_cancelled_without_execution() {
        let mut live = session();
        live.prepare().unwrap();
        live.advance_one_release().unwrap();
        let load = live.recommended_load().unwrap();
        live.submit_operator_action(MissionOperatorAction::Stage {
            load,
            completed_event_mask: 0,
        })
        .unwrap();
        let mut cancel = live.commit_request_for_staged().unwrap();
        cancel.kind = ksa64_interface::phase11::UplinkControlKind::Cancellation;
        let receipt = live
            .submit_operator_action(MissionOperatorAction::Cancel(cancel))
            .unwrap();
        assert_eq!(receipt.record.state, UplinkState::Cancelled);
        assert!(live.snapshot().staged_load_identity.is_none());
    }

    #[test]
    fn an_unanswered_procedure_finishes_as_failed_evidence() {
        let mut live = session();
        live.prepare().unwrap();
        while live.lifecycle() != MissionSessionLifecycle::Completed {
            live.advance_one_release().unwrap();
        }
        let completed = live.finish().unwrap();
        assert!(completed.evidence.actions.is_empty());
        assert!(completed.evidence.releases <= LIVE_GNSS_LOSS_MAX_RELEASES);
    }
}
