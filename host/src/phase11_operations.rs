//! Deterministic Phase 11 procedures, roles, and role-filtered operations data.

use ksa64_interface::phase11::{
    OperationalRole, ProcedureComparison, ProcedurePack, ProcedurePredicate, ProcedureStep,
    ProcedureStepKind, UplinkLoadType, KPC11_MAX_STEPS,
};

pub const PHASE11_OPERATIONAL_METRIC_CATALOG_ID: u32 = 0x11c2_2001;
pub const METRIC_GNSS_VALID: u16 = 1;
pub const METRIC_INERTIAL_HEALTHY: u16 = 2;
pub const METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12: u16 = 3;
pub const METRIC_GROUND_COMMUNICATIONS_AVAILABLE: u16 = 4;
pub const METRIC_UPLINK_STAGE_ACCEPTED: u16 = 5;
pub const METRIC_UPLINK_COMMIT_ACCEPTED: u16 = 6;
pub const METRIC_SAFE_STATE: u16 = 7;
pub const OPERATIONAL_METRIC_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationalMetricSnapshot {
    pub epoch: u32,
    values: [i32; OPERATIONAL_METRIC_CAPACITY],
    valid: u32,
}

impl OperationalMetricSnapshot {
    pub const fn new(epoch: u32) -> Self {
        Self {
            epoch,
            values: [0; OPERATIONAL_METRIC_CAPACITY],
            valid: 0,
        }
    }

    pub fn set(&mut self, metric_id: u16, value: i32) -> bool {
        let Some(index) = metric_index(metric_id) else {
            return false;
        };
        self.values[index] = value;
        self.valid |= 1 << index;
        true
    }

    pub fn get(&self, metric_id: u16) -> Option<i32> {
        let index = metric_index(metric_id)?;
        (self.valid & (1 << index) != 0).then_some(self.values[index])
    }
}

fn metric_index(metric_id: u16) -> Option<usize> {
    let index = usize::from(metric_id.checked_sub(1)?);
    (index < OPERATIONAL_METRIC_CAPACITY).then_some(index)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcedureState {
    Active,
    Completed,
    Skipped,
    Failed,
    Mistimed,
    ManuallyOverridden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcedureEvidenceKind {
    Enter,
    Complete,
    Timeout,
    Acknowledge,
    Branch,
    Action,
    Hint,
    ManualOverride,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedureEvidence {
    pub sequence: u32,
    pub epoch: u32,
    pub step_id: u16,
    pub kind: ProcedureEvidenceKind,
    pub state: ProcedureState,
    pub detail: u32,
    pub chain: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedureActionRequest {
    pub step_id: u16,
    pub action: UplinkLoadType,
    pub arguments: [i32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcedureError {
    Identity,
    Graph,
    Permission,
    State,
    Step,
}

pub struct ProcedureEngine {
    pack: ProcedurePack,
    state: ProcedureState,
    current_step: u16,
    entered_epoch: u32,
    evidence: Vec<ProcedureEvidence>,
    chain: u32,
}

impl ProcedureEngine {
    pub fn new(pack: ProcedurePack, epoch: u32) -> Result<Self, ProcedureError> {
        validate_pack(&pack)?;
        let current_step = pack.entry_step;
        let mut engine = Self {
            pack,
            state: ProcedureState::Active,
            current_step,
            entered_epoch: epoch,
            evidence: Vec::new(),
            chain: 0x811c_9dc5,
        };
        engine.record(epoch, ProcedureEvidenceKind::Enter, current_step, 0);
        Ok(engine)
    }

    pub const fn state(&self) -> ProcedureState {
        self.state
    }

    pub const fn current_step(&self) -> u16 {
        self.current_step
    }

    pub const fn entered_epoch(&self) -> u32 {
        self.entered_epoch
    }

    pub fn current(&self) -> Option<&ProcedureStep> {
        find_step(&self.pack, self.current_step)
    }

    pub fn evidence(&self) -> &[ProcedureEvidence] {
        &self.evidence
    }

    pub fn requested_action(&self) -> Option<ProcedureActionRequest> {
        let step = self.current()?;
        if self.state != ProcedureState::Active || step.kind != ProcedureStepKind::RequestAction {
            return None;
        }
        Some(ProcedureActionRequest {
            step_id: step.step_id,
            action: step.action?,
            arguments: step.action_arguments,
        })
    }

    pub fn tick(&mut self, snapshot: &OperationalMetricSnapshot) -> Result<(), ProcedureError> {
        if self.state != ProcedureState::Active {
            return Ok(());
        }
        let step = *self.current().ok_or(ProcedureError::Step)?;
        if step.timeout_epochs != 0
            && snapshot.epoch > self.entered_epoch.saturating_add(step.timeout_epochs)
        {
            self.record(
                snapshot.epoch,
                ProcedureEvidenceKind::Timeout,
                step.step_id,
                step.timeout_epochs,
            );
            if step.next_failure == 0 {
                self.state = ProcedureState::Mistimed;
            } else {
                self.transition(
                    snapshot.epoch,
                    step.next_failure,
                    ProcedureEvidenceKind::Branch,
                )?;
            }
            return Ok(());
        }
        match step.kind {
            ProcedureStepKind::Observe | ProcedureStepKind::Verify | ProcedureStepKind::Wait => {
                if predicates_satisfied(&step, snapshot) {
                    self.complete_step(snapshot.epoch, step)?;
                }
            }
            ProcedureStepKind::Branch => {
                let destination = if predicates_satisfied(&step, snapshot) {
                    step.next_complete
                } else {
                    step.next_failure
                };
                self.transition(snapshot.epoch, destination, ProcedureEvidenceKind::Branch)?;
            }
            ProcedureStepKind::Complete => {
                self.state = ProcedureState::Completed;
                self.record(
                    snapshot.epoch,
                    ProcedureEvidenceKind::Complete,
                    step.step_id,
                    0,
                );
            }
            ProcedureStepKind::Skip => {
                self.state = ProcedureState::Skipped;
                self.record(
                    snapshot.epoch,
                    ProcedureEvidenceKind::Complete,
                    step.step_id,
                    0,
                );
            }
            ProcedureStepKind::Fail => {
                self.state = ProcedureState::Failed;
                self.record(
                    snapshot.epoch,
                    ProcedureEvidenceKind::Complete,
                    step.step_id,
                    0,
                );
            }
            ProcedureStepKind::Acknowledge
            | ProcedureStepKind::RequestAction
            | ProcedureStepKind::Caution
            | ProcedureStepKind::Warning => {}
        }
        Ok(())
    }

    /// Force the active step through its declared timeout edge at an external
    /// absolute mission deadline. This preserves the procedure's normal failure
    /// branch while preventing per-step relative deadlines from drifting.
    pub fn expire_at(&mut self, epoch: u32) -> Result<(), ProcedureError> {
        if self.state != ProcedureState::Active {
            return Ok(());
        }
        let step = *self.current().ok_or(ProcedureError::Step)?;
        self.record(epoch, ProcedureEvidenceKind::Timeout, step.step_id, 0);
        if step.next_failure == 0 {
            self.state = ProcedureState::Mistimed;
        } else {
            self.transition(epoch, step.next_failure, ProcedureEvidenceKind::Branch)?;
        }
        Ok(())
    }

    pub fn acknowledge(&mut self, role: OperationalRole, epoch: u32) -> Result<(), ProcedureError> {
        if !role_can_act(role) || self.state != ProcedureState::Active {
            return Err(ProcedureError::Permission);
        }
        let step = *self.current().ok_or(ProcedureError::Step)?;
        if !matches!(
            step.kind,
            ProcedureStepKind::Acknowledge
                | ProcedureStepKind::Caution
                | ProcedureStepKind::Warning
        ) {
            return Err(ProcedureError::State);
        }
        self.record(epoch, ProcedureEvidenceKind::Acknowledge, step.step_id, 0);
        self.complete_step(epoch, step)
    }

    pub fn accept_action(
        &mut self,
        role: OperationalRole,
        epoch: u32,
        action: UplinkLoadType,
        accepted: bool,
    ) -> Result<(), ProcedureError> {
        if !role_can_act(role) || self.state != ProcedureState::Active {
            return Err(ProcedureError::Permission);
        }
        let step = *self.current().ok_or(ProcedureError::Step)?;
        if step.kind != ProcedureStepKind::RequestAction || step.action != Some(action) {
            return Err(ProcedureError::State);
        }
        self.record(
            epoch,
            ProcedureEvidenceKind::Action,
            step.step_id,
            u32::from(accepted),
        );
        let destination = if accepted {
            step.next_complete
        } else {
            step.next_failure
        };
        self.transition(epoch, destination, ProcedureEvidenceKind::Branch)
    }

    pub fn use_hint(&mut self, role: OperationalRole, epoch: u32) -> Result<u16, ProcedureError> {
        if role != OperationalRole::GuidedOperator || self.state != ProcedureState::Active {
            return Err(ProcedureError::Permission);
        }
        let step = *self.current().ok_or(ProcedureError::Step)?;
        if step.hint_identity == 0 {
            return Err(ProcedureError::State);
        }
        self.record(
            epoch,
            ProcedureEvidenceKind::Hint,
            step.step_id,
            u32::from(step.hint_identity),
        );
        Ok(step.hint_identity)
    }

    pub fn override_step(
        &mut self,
        role: OperationalRole,
        epoch: u32,
        complete: bool,
    ) -> Result<(), ProcedureError> {
        if !matches!(
            role,
            OperationalRole::FlightController | OperationalRole::SimDirector
        ) || self.state != ProcedureState::Active
        {
            return Err(ProcedureError::Permission);
        }
        let step = *self.current().ok_or(ProcedureError::Step)?;
        self.record(
            epoch,
            ProcedureEvidenceKind::ManualOverride,
            step.step_id,
            u32::from(complete),
        );
        if complete {
            if step.next_complete == 0 {
                self.state = ProcedureState::ManuallyOverridden;
            } else {
                self.transition(epoch, step.next_complete, ProcedureEvidenceKind::Branch)?;
            }
        } else {
            self.state = ProcedureState::ManuallyOverridden;
        }
        Ok(())
    }

    fn complete_step(&mut self, epoch: u32, step: ProcedureStep) -> Result<(), ProcedureError> {
        self.record(epoch, ProcedureEvidenceKind::Complete, step.step_id, 0);
        self.transition(epoch, step.next_complete, ProcedureEvidenceKind::Enter)
    }

    fn transition(
        &mut self,
        epoch: u32,
        destination: u16,
        kind: ProcedureEvidenceKind,
    ) -> Result<(), ProcedureError> {
        if destination == 0 || find_step(&self.pack, destination).is_none() {
            self.state = ProcedureState::Failed;
            return Err(ProcedureError::Graph);
        }
        self.current_step = destination;
        self.entered_epoch = epoch;
        self.record(epoch, kind, destination, 0);
        Ok(())
    }

    fn record(&mut self, epoch: u32, kind: ProcedureEvidenceKind, step_id: u16, detail: u32) {
        let sequence = self.evidence.len() as u32 + 1;
        self.chain = hash_words(&[
            self.chain,
            sequence,
            epoch,
            u32::from(step_id),
            kind as u32,
            self.state as u32,
            detail,
        ]);
        self.evidence.push(ProcedureEvidence {
            sequence,
            epoch,
            step_id,
            kind,
            state: self.state,
            detail,
            chain: self.chain,
        });
    }
}

fn validate_pack(pack: &ProcedurePack) -> Result<(), ProcedureError> {
    if pack.metric_catalog_identity != PHASE11_OPERATIONAL_METRIC_CATALOG_ID
        || pack.step_count == 0
        || usize::from(pack.step_count) > KPC11_MAX_STEPS
        || find_step(pack, pack.entry_step).is_none()
    {
        return Err(ProcedureError::Identity);
    }
    for (index, step) in pack.steps[..usize::from(pack.step_count)]
        .iter()
        .enumerate()
    {
        if step.step_id == 0
            || pack.steps[..index]
                .iter()
                .any(|prior| prior.step_id == step.step_id)
            || step.next_complete != 0 && find_step(pack, step.next_complete).is_none()
            || step.next_failure != 0 && find_step(pack, step.next_failure).is_none()
            || step.kind == ProcedureStepKind::RequestAction && step.action.is_none()
        {
            return Err(ProcedureError::Graph);
        }
    }
    Ok(())
}

fn find_step(pack: &ProcedurePack, id: u16) -> Option<&ProcedureStep> {
    pack.steps[..usize::from(pack.step_count)]
        .iter()
        .find(|step| step.step_id == id)
}

fn predicates_satisfied(step: &ProcedureStep, snapshot: &OperationalMetricSnapshot) -> bool {
    step.predicates[..usize::from(step.predicate_count)]
        .iter()
        .all(|predicate| predicate_satisfied(*predicate, snapshot))
}

fn predicate_satisfied(
    predicate: ProcedurePredicate,
    snapshot: &OperationalMetricSnapshot,
) -> bool {
    let Some(value) = snapshot.get(predicate.metric_id) else {
        return predicate.flags & 1 != 0;
    };
    match predicate.comparison {
        ProcedureComparison::Equal => value == predicate.threshold,
        ProcedureComparison::NotEqual => value != predicate.threshold,
        ProcedureComparison::Less => value < predicate.threshold,
        ProcedureComparison::LessOrEqual => value <= predicate.threshold,
        ProcedureComparison::Greater => value > predicate.threshold,
        ProcedureComparison::GreaterOrEqual => value >= predicate.threshold,
        ProcedureComparison::BitSet => value & predicate.threshold == predicate.threshold,
        ProcedureComparison::BitClear => value & predicate.threshold == 0,
    }
}

pub const fn role_can_view_truth(role: OperationalRole) -> bool {
    matches!(role, OperationalRole::SimDirector)
}

pub const fn role_can_inject_fault(role: OperationalRole) -> bool {
    matches!(role, OperationalRole::SimDirector)
}

pub const fn role_can_act(role: OperationalRole) -> bool {
    matches!(
        role,
        OperationalRole::GuidedOperator
            | OperationalRole::FlightController
            | OperationalRole::SimDirector
            | OperationalRole::ScriptedOperator
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalTelemetryView {
    pub epoch: u32,
    pub package_identity: u32,
    pub plan_identity: u32,
    pub procedure_step: u16,
    pub metrics: OperationalMetricSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimDirectorTelemetryView<T> {
    pub operational: OperationalTelemetryView,
    pub truth: T,
}

pub fn role_filtered_truth<T: Clone>(
    role: OperationalRole,
    view: &SimDirectorTelemetryView<T>,
) -> Option<T> {
    role_can_view_truth(role).then(|| view.truth.clone())
}

pub fn gnss_loss_procedure_pack(plan_identity: u32) -> ProcedurePack {
    let mut steps = [ProcedureStep::EMPTY; KPC11_MAX_STEPS];
    steps[0] = procedure_step(
        1,
        ProcedureStepKind::Verify,
        2,
        7,
        METRIC_GNSS_VALID,
        ProcedureComparison::Equal,
        0,
    );
    steps[1] = procedure_step(
        2,
        ProcedureStepKind::Verify,
        3,
        7,
        METRIC_INERTIAL_HEALTHY,
        ProcedureComparison::Equal,
        1,
    );
    steps[2] = procedure_step(
        3,
        ProcedureStepKind::Verify,
        4,
        7,
        METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12,
        ProcedureComparison::LessOrEqual,
        2_048,
    );
    steps[3] = action_step(4, UplinkLoadType::GroundNavigationUpdate, 5, 7, [0, 0]);
    steps[4] = action_step(5, UplinkLoadType::ContingencyBranch, 6, 7, [1, 0]);
    steps[5] = terminal_step(6, ProcedureStepKind::Complete);
    steps[6] = terminal_step(7, ProcedureStepKind::Fail);
    ProcedurePack {
        pack_identity: 0x11c2_0001,
        plan_identity,
        procedure_identity: 0x11c2_1001,
        metric_catalog_identity: PHASE11_OPERATIONAL_METRIC_CATALOG_ID,
        name_identity: 0x11c2_3001,
        step_count: 7,
        entry_step: 1,
        flags: 0,
        steps,
    }
}

fn procedure_step(
    id: u16,
    kind: ProcedureStepKind,
    next_complete: u16,
    next_failure: u16,
    metric_id: u16,
    comparison: ProcedureComparison,
    threshold: i32,
) -> ProcedureStep {
    ProcedureStep {
        step_id: id,
        kind,
        next_complete,
        next_failure,
        timeout_epochs: 320,
        predicate_count: 1,
        hint_identity: id,
        message_identity: 0x1100_0000 | u32::from(id),
        predicates: [
            ProcedurePredicate {
                metric_id,
                comparison,
                flags: 0,
                threshold,
            },
            ProcedurePredicate::EMPTY,
            ProcedurePredicate::EMPTY,
            ProcedurePredicate::EMPTY,
        ],
        ..ProcedureStep::EMPTY
    }
}

fn action_step(
    id: u16,
    action: UplinkLoadType,
    next_complete: u16,
    next_failure: u16,
    arguments: [i32; 2],
) -> ProcedureStep {
    ProcedureStep {
        step_id: id,
        kind: ProcedureStepKind::RequestAction,
        next_complete,
        next_failure,
        timeout_epochs: 320,
        action: Some(action),
        hint_identity: id,
        message_identity: 0x1100_0000 | u32::from(id),
        action_arguments: arguments,
        ..ProcedureStep::EMPTY
    }
}

fn terminal_step(id: u16, kind: ProcedureStepKind) -> ProcedureStep {
    ProcedureStep {
        step_id: id,
        kind,
        message_identity: 0x1100_0000 | u32::from(id),
        ..ProcedureStep::EMPTY
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_loss_snapshot(epoch: u32) -> OperationalMetricSnapshot {
        let mut snapshot = OperationalMetricSnapshot::new(epoch);
        assert!(snapshot.set(METRIC_GNSS_VALID, 0));
        assert!(snapshot.set(METRIC_INERTIAL_HEALTHY, 1));
        assert!(snapshot.set(METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12, 1_000));
        snapshot
    }

    #[test]
    fn gnss_loss_procedure_branches_and_actions_deterministically() {
        let pack = gnss_loss_procedure_pack(0x11a0_0001);
        let mut engine = ProcedureEngine::new(pack, 100).unwrap();
        let snapshot = healthy_loss_snapshot(100);
        for expected in [2, 3, 4] {
            engine.tick(&snapshot).unwrap();
            assert_eq!(engine.current_step(), expected);
        }
        assert_eq!(
            engine.requested_action().unwrap().action,
            UplinkLoadType::GroundNavigationUpdate
        );
        engine
            .accept_action(
                OperationalRole::FlightController,
                101,
                UplinkLoadType::GroundNavigationUpdate,
                true,
            )
            .unwrap();
        engine
            .accept_action(
                OperationalRole::FlightController,
                102,
                UplinkLoadType::ContingencyBranch,
                true,
            )
            .unwrap();
        engine.tick(&healthy_loss_snapshot(103)).unwrap();
        assert_eq!(engine.state(), ProcedureState::Completed);
    }

    #[test]
    fn observer_cannot_act_and_only_sim_director_can_receive_truth() {
        let pack = gnss_loss_procedure_pack(0x11a0_0001);
        let mut engine = ProcedureEngine::new(pack, 100).unwrap();
        let snapshot = healthy_loss_snapshot(100);
        for _ in 0..3 {
            engine.tick(&snapshot).unwrap();
        }
        assert_eq!(
            engine.accept_action(
                OperationalRole::Observer,
                101,
                UplinkLoadType::GroundNavigationUpdate,
                true,
            ),
            Err(ProcedureError::Permission)
        );
        let view = SimDirectorTelemetryView {
            operational: OperationalTelemetryView {
                epoch: 100,
                package_identity: 1,
                plan_identity: 2,
                procedure_step: 4,
                metrics: snapshot,
            },
            truth: [1, 2, 3],
        };
        assert_eq!(role_filtered_truth(OperationalRole::Observer, &view), None);
        assert_eq!(
            role_filtered_truth(OperationalRole::SimDirector, &view),
            Some([1, 2, 3])
        );
    }
}
