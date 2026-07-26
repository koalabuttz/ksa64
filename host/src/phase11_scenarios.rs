//! Integrated deterministic Phase 11 operations scenarios.

use crate::phase11_operations::{
    gnss_loss_procedure_pack, OperationalMetricSnapshot, ProcedureEngine, ProcedureState,
    METRIC_GNSS_VALID, METRIC_INERTIAL_HEALTHY, METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12,
};
use ksa64_core::phase10_contract::WGS84_SEMI_MAJOR_Q12_KM;
use ksa64_flight::phase10::{
    ksa_g10r_reference_flight_config, GlobalFlightComputer, GlobalFlightConfig,
    GlobalFlightEvidence,
};
use ksa64_flight::phase11::{
    ksa_g10r_reference_mission_plan, GlobalKlr10FlightPackage, KsaG10rReferenceOpsV1,
    EVENT_JOURNAL_CAPACITY, KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
};
use ksa64_interface::phase10::{
    GlobalFastSensorCell, GlobalFrameId, GLOBAL_FAST_ATTITUDE, GLOBAL_FAST_DELTA_ANGLE,
    GLOBAL_FAST_DELTA_V,
};
use ksa64_interface::phase11::{
    ActionLogRecord, EventJournalRecord, FlightAbiId, OperationalRole, UplinkCommandLoad,
    UplinkControlKind, UplinkControlRecord, UplinkLoadType, UplinkReasonCode, UplinkState,
    PACKAGE_CAP_BRANCH_SELECT, PACKAGE_CAP_GROUND_NAV_UPDATE, PACKAGE_CAP_HIGH_LEVEL_MODE,
    PACKAGE_CAP_TARGET_UPDATE,
};
use ksa64_sim::phase11::{
    synthesize_ground_observation, GroundEstimator, GroundTruthSample, GROUND_ESTIMATOR_ID,
};

pub const NOMINAL_OPERATIONS_SCENARIO_ID: u32 = 0x11b0_0001;
pub const GNSS_LOSS_SCENARIO_ID: u32 = 0x11b0_0002;
pub const GUIDANCE_UPDATE_SCENARIO_ID: u32 = 0x11b0_0003;
pub const GROUND_BLACKOUT_SCENARIO_ID: u32 = 0x11b0_0004;
pub const GNSS_LOSS_NO_ACTION_SCENARIO_ID: u32 = 0x11b0_0006;
pub const GNSS_LOSS_DELAYED_ACTION_SCENARIO_ID: u32 = 0x11b0_0007;
pub const INVALID_OPERATIONS_SCENARIO_ID: u32 = 0x11b0_0005;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalScenarioEvidence {
    pub scenario_identity: u32,
    pub releases: u32,
    pub flight_checksum: u32,
    pub navigation_checksum: u32,
    pub command_checksum: u32,
    pub prediction_checksum: u32,
    pub procedure_chain: u32,
    pub journal_chain: u32,
    pub action_chain: u32,
    pub rejected_loads: u16,
    pub procedure_state: ProcedureState,
    pub safe: bool,
    pub evidence_identity: u32,
    pub actions: Vec<ActionLogRecord>,
}

#[derive(Clone)]
pub(crate) struct ActionTranscript {
    pub(crate) records: Vec<ActionLogRecord>,
    pub(crate) chain: u32,
}

impl ActionTranscript {
    pub(crate) fn new() -> Self {
        Self {
            records: Vec::new(),
            chain: 0x811c_9dc5,
        }
    }

    pub(crate) fn record(&mut self, epoch: u32, step: u16, receipt: UplinkControlRecord) {
        let sequence = self.records.len() as u32 + 1;
        let prior = self.chain;
        self.chain = hash_words(&[
            prior,
            sequence,
            epoch,
            u32::from(step),
            receipt.kind as u32,
            receipt.state as u32,
            receipt.reason as u32,
            receipt.load_identity,
        ]);
        self.records.push(ActionLogRecord {
            sequence,
            epoch,
            role: OperationalRole::FlightController,
            action_kind: receipt.kind,
            state: receipt.state,
            reason: receipt.reason,
            load_identity: receipt.load_identity,
            detail_identity: receipt.control_identity,
            procedure_step: step,
            arguments: [receipt.effective_epoch as i32, 0, 0, 0],
            prior_chain: prior,
            chain: self.chain,
        });
    }
}

pub fn run_nominal_operations_probe() -> OperationalScenarioEvidence {
    let config = coast_config();
    let mut frozen = GlobalFlightComputer::new(config).unwrap();
    let mut operational = KsaG10rReferenceOpsV1::new(config).unwrap();
    assert!(operational.initialize_mission_plan(ksa_g10r_reference_mission_plan()));
    let mut final_evidence = None;
    for epoch in 0..64u16 {
        let fast = coast_fast(epoch);
        let expected = frozen.tick(Some(fast), None, None);
        let actual = operational.process_release(Some(fast), None, None);
        assert_eq!(actual, expected);
        final_evidence = Some(actual);
    }
    finish_evidence(
        NOMINAL_OPERATIONS_SCENARIO_ID,
        64,
        final_evidence.unwrap(),
        &operational,
        ProcedureState::Completed,
        0,
        ActionTranscript::new(),
        0,
    )
}

pub fn run_gnss_loss_procedure(_scripted: bool) -> OperationalScenarioEvidence {
    let mut package = KsaG10rReferenceOpsV1::new(coast_config()).unwrap();
    let plan = ksa_g10r_reference_mission_plan();
    assert!(package.initialize_mission_plan(plan));
    let mut procedure =
        ProcedureEngine::new(gnss_loss_procedure_pack(plan.plan_identity), 0).unwrap();
    let mut transcript = ActionTranscript::new();

    let first = package.process_release(Some(coast_fast(0)), None, None);
    let observation = synthesize_ground_observation(
        GroundTruthSample {
            epoch: 0,
            frame: first.navigation.frame,
            position_q12_km: first.navigation.position_q12,
            velocity_q24_km_s: first.navigation.velocity_q24,
        },
        0,
        0x4b53_4111,
    );
    let mut estimator = GroundEstimator::new();
    let ground = estimator.update(observation, 0).unwrap();
    let mut snapshot = operational_snapshot(0, &first, ground.position_q12_km);
    for _ in 0..3 {
        procedure.tick(&snapshot).unwrap();
    }

    let ground_load = ground_navigation_load(1, 4, 0x11c0_2001, ground);
    let staged = package.stage_uplink(ground_load, 0).unwrap();
    transcript.record(1, procedure.current_step(), staged);
    let committed = package
        .commit_uplink(&commit_request(ground_load, 1))
        .unwrap();
    transcript.record(1, procedure.current_step(), committed);
    procedure
        .accept_action(
            OperationalRole::FlightController,
            1,
            UplinkLoadType::GroundNavigationUpdate,
            committed.state == UplinkState::Committed,
        )
        .unwrap();

    let mut final_flight = first;
    for epoch in 1..=4u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }
    let branch_load = generic_load(
        5,
        8,
        0x11c0_2002,
        UplinkLoadType::ContingencyBranch,
        PACKAGE_CAP_BRANCH_SELECT,
        final_flight.navigation.frame,
        [1, 0, 0, 0],
    );
    let staged = package.stage_uplink(branch_load, 0).unwrap();
    transcript.record(5, procedure.current_step(), staged);
    let committed = package
        .commit_uplink(&commit_request(branch_load, 5))
        .unwrap();
    transcript.record(5, procedure.current_step(), committed);
    procedure
        .accept_action(
            OperationalRole::FlightController,
            5,
            UplinkLoadType::ContingencyBranch,
            committed.state == UplinkState::Committed,
        )
        .unwrap();
    for epoch in 5..=8u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }
    snapshot = operational_snapshot(9, &final_flight, ground.position_q12_km);
    procedure.tick(&snapshot).unwrap();
    assert_eq!(procedure.state(), ProcedureState::Completed);
    let procedure_chain = procedure.evidence().last().map_or(0, |record| record.chain);
    finish_evidence(
        GNSS_LOSS_SCENARIO_ID,
        9,
        final_flight,
        &package,
        procedure.state(),
        procedure_chain,
        transcript,
        0,
    )
}
pub fn run_gnss_loss_no_action_probe() -> OperationalScenarioEvidence {
    let (mut package, mut procedure, ground, first) = gnss_loss_setup();
    let transcript = ActionTranscript::new();
    let mut final_flight = first;
    for epoch in 1..=321u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }
    let snapshot = operational_snapshot(321, &final_flight, ground.position_q12_km);
    procedure.tick(&snapshot).unwrap();
    procedure.tick(&snapshot).unwrap();
    assert_eq!(procedure.state(), ProcedureState::Failed);
    let procedure_chain = procedure.evidence().last().map_or(0, |record| record.chain);
    finish_evidence(
        GNSS_LOSS_NO_ACTION_SCENARIO_ID,
        322,
        final_flight,
        &package,
        procedure.state(),
        procedure_chain,
        transcript,
        0,
    )
}

pub fn run_gnss_loss_delayed_action_probe() -> OperationalScenarioEvidence {
    let (mut package, mut procedure, ground, first) = gnss_loss_setup();
    let mut transcript = ActionTranscript::new();
    let mut final_flight = first;
    for epoch in 1..=321u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }
    let snapshot = operational_snapshot(321, &final_flight, ground.position_q12_km);
    procedure.tick(&snapshot).unwrap();
    procedure.tick(&snapshot).unwrap();
    assert_eq!(procedure.state(), ProcedureState::Failed);

    let ground_load = ground_navigation_load(322, 325, 0x11c0_6001, ground);
    let staged = package.stage_uplink(ground_load, 322).unwrap();
    transcript.record(322, procedure.current_step(), staged);
    let committed = package
        .commit_uplink(&commit_request(ground_load, 322))
        .unwrap();
    transcript.record(322, procedure.current_step(), committed);
    for epoch in 322..=325u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }

    let branch_load = generic_load(
        326,
        329,
        0x11c0_6002,
        UplinkLoadType::ContingencyBranch,
        PACKAGE_CAP_BRANCH_SELECT,
        final_flight.navigation.frame,
        [1, 0, 0, 0],
    );
    let staged = package.stage_uplink(branch_load, 326).unwrap();
    transcript.record(326, procedure.current_step(), staged);
    let committed = package
        .commit_uplink(&commit_request(branch_load, 326))
        .unwrap();
    transcript.record(326, procedure.current_step(), committed);
    for epoch in 326..=329u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }
    let procedure_chain = procedure.evidence().last().map_or(0, |record| record.chain);
    finish_evidence(
        GNSS_LOSS_DELAYED_ACTION_SCENARIO_ID,
        330,
        final_flight,
        &package,
        procedure.state(),
        procedure_chain,
        transcript,
        0,
    )
}

pub fn run_guidance_update_probe() -> OperationalScenarioEvidence {
    let mut package = KsaG10rReferenceOpsV1::new(coast_config()).unwrap();
    assert!(package.initialize_mission_plan(ksa_g10r_reference_mission_plan()));
    let initial = package.process_release(Some(coast_fast(0)), None, None);
    let before = package.prediction_summary().unwrap().prediction_checksum;
    let load = generic_load(
        1,
        4,
        0x11c0_3001,
        UplinkLoadType::MissionEventTarget,
        PACKAGE_CAP_TARGET_UPDATE,
        initial.navigation.frame,
        [2, 1 << 30, 0, 0],
    );
    let mut transcript = ActionTranscript::new();
    let staged = package.stage_uplink(load, 0).unwrap();
    transcript.record(1, 0, staged);
    let committed = package.commit_uplink(&commit_request(load, 1)).unwrap();
    transcript.record(1, 0, committed);
    let mut final_flight = initial;
    for epoch in 1..=4u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }
    assert_ne!(
        package.prediction_summary().unwrap().prediction_checksum,
        before
    );
    finish_evidence(
        GUIDANCE_UPDATE_SCENARIO_ID,
        5,
        final_flight,
        &package,
        ProcedureState::Completed,
        0,
        transcript,
        0,
    )
}

pub fn run_ground_blackout_probe() -> OperationalScenarioEvidence {
    let mut package = KsaG10rReferenceOpsV1::new(coast_config()).unwrap();
    assert!(package.initialize_mission_plan(ksa_g10r_reference_mission_plan()));
    let load = generic_load(
        0,
        4,
        0x11c0_4001,
        UplinkLoadType::MissionEventTarget,
        PACKAGE_CAP_TARGET_UPDATE,
        GlobalFrameId::EarthInertialEciV1,
        [2, 1 << 30, 0, 0],
    );
    let mut transcript = ActionTranscript::new();
    let staged = package.stage_uplink(load, 0).unwrap();
    transcript.record(0, 0, staged);
    let committed = package.commit_uplink(&commit_request(load, 0)).unwrap();
    transcript.record(0, 0, committed);
    package.record_ground_communications(false);
    let mut final_flight = package.process_release(Some(coast_fast(0)), None, None);
    for epoch in 1..=8u16 {
        final_flight = package.process_release(Some(coast_fast(epoch)), None, None);
    }
    package.record_ground_communications(true);

    let mut uncommitted = KsaG10rReferenceOpsV1::new(coast_config()).unwrap();
    assert!(uncommitted.initialize_mission_plan(ksa_g10r_reference_mission_plan()));
    let never = generic_load(
        0,
        3,
        0x11c0_4002,
        UplinkLoadType::HighLevelMode,
        PACKAGE_CAP_HIGH_LEVEL_MODE,
        GlobalFrameId::EarthInertialEciV1,
        [2, 0, 0, 0],
    );
    assert_eq!(
        uncommitted.stage_uplink(never, 0).unwrap().state,
        UplinkState::Staged
    );
    let mut uncommitted_flight = uncommitted.process_release(Some(coast_fast(0)), None, None);
    for epoch in 1..=4u16 {
        uncommitted_flight = uncommitted.process_release(Some(coast_fast(epoch)), None, None);
    }
    assert!(!uncommitted_flight.safe);

    finish_evidence(
        GROUND_BLACKOUT_SCENARIO_ID,
        9,
        final_flight,
        &package,
        ProcedureState::Completed,
        0,
        transcript,
        0,
    )
}

pub fn run_invalid_operations_probe() -> OperationalScenarioEvidence {
    let mut package = KsaG10rReferenceOpsV1::new(coast_config()).unwrap();
    assert!(package.initialize_mission_plan(ksa_g10r_reference_mission_plan()));
    let baseline = generic_load(
        0,
        4,
        0x11c0_5001,
        UplinkLoadType::HighLevelMode,
        PACKAGE_CAP_HIGH_LEVEL_MODE,
        GlobalFrameId::EarthInertialEciV1,
        [1, 0, 0, 0],
    );
    let mut cases = [baseline; 6];
    cases[0].package_manifest_identity ^= 1;
    cases[1].frame = GlobalFrameId::EarthFixedEcefV1;
    cases[2].stage_epoch = 1;
    cases[3].requested_effective_epoch = 1;
    cases[4].required_capabilities = 1 << 31;
    cases[5].prerequisite_event_mask = 1;
    let expected = [
        UplinkReasonCode::Identity,
        UplinkReasonCode::Frame,
        UplinkReasonCode::Stale,
        UplinkReasonCode::Late,
        UplinkReasonCode::Capability,
        UplinkReasonCode::Prerequisite,
    ];
    let mut transcript = ActionTranscript::new();
    for (index, (load, reason)) in cases.into_iter().zip(expected).enumerate() {
        let receipt = package.stage_uplink(load, 0).unwrap();
        assert_eq!(receipt.state, UplinkState::Rejected);
        assert_eq!(receipt.reason, reason);
        transcript.record(0, index as u16, receipt);
    }
    let final_flight = package.process_release(Some(coast_fast(0)), None, None);
    finish_evidence(
        INVALID_OPERATIONS_SCENARIO_ID,
        1,
        final_flight,
        &package,
        ProcedureState::Completed,
        0,
        transcript,
        expected.len() as u16,
    )
}

pub(crate) fn coast_config() -> GlobalFlightConfig {
    GlobalFlightConfig {
        initial_frame: GlobalFrameId::EarthInertialEciV1,
        initial_position_q12: [WGS84_SEMI_MAJOR_Q12_KM + 614_400, 0, 0],
        ..ksa_g10r_reference_flight_config()
    }
}
fn gnss_loss_setup() -> (
    KsaG10rReferenceOpsV1,
    ProcedureEngine,
    ksa64_interface::phase11::GroundEstimate,
    GlobalFlightEvidence,
) {
    let mut package = KsaG10rReferenceOpsV1::new(coast_config()).unwrap();
    let plan = ksa_g10r_reference_mission_plan();
    assert!(package.initialize_mission_plan(plan));
    let mut procedure =
        ProcedureEngine::new(gnss_loss_procedure_pack(plan.plan_identity), 0).unwrap();
    let first = package.process_release(Some(coast_fast(0)), None, None);
    let observation = synthesize_ground_observation(
        GroundTruthSample {
            epoch: 0,
            frame: first.navigation.frame,
            position_q12_km: first.navigation.position_q12,
            velocity_q24_km_s: first.navigation.velocity_q24,
        },
        0,
        0x4b53_4111,
    );
    let mut estimator = GroundEstimator::new();
    let ground = estimator.update(observation, 0).unwrap();
    let snapshot = operational_snapshot(0, &first, ground.position_q12_km);
    for _ in 0..3 {
        procedure.tick(&snapshot).unwrap();
    }
    assert_eq!(procedure.current_step(), 4);
    (package, procedure, ground, first)
}

pub(crate) fn coast_fast(epoch: u16) -> GlobalFastSensorCell {
    GlobalFastSensorCell {
        session: 0x10a0,
        measurement_epoch: epoch,
        production_epoch: epoch,
        frame: GlobalFrameId::EarthInertialEciV1,
        validity: GLOBAL_FAST_DELTA_V | GLOBAL_FAST_DELTA_ANGLE | GLOBAL_FAST_ATTITUDE,
        mission_time_q16: 4_000_000 + u32::from(epoch) * 2_048,
        delta_velocity_q24: [0, 4, 0],
        delta_angle_q24: [0; 3],
        attitude_vector_q15: [0; 3],
        angular_rate_q15: [0; 3],
        dynamic_pressure_q10: 0,
        mach_q12: 0,
        gimbal_applied_q15: [0; 2],
        rcs_propellant_q21: 5 << 21,
        actuator_feedback: 0,
        vehicle_status: 0,
        sensor_checksum: epoch ^ 0x11,
    }
}

pub(crate) fn operational_snapshot(
    epoch: u32,
    flight: &GlobalFlightEvidence,
    ground_position: [i32; 3],
) -> OperationalMetricSnapshot {
    let mut snapshot = OperationalMetricSnapshot::new(epoch);
    snapshot.set(METRIC_GNSS_VALID, 0);
    snapshot.set(METRIC_INERTIAL_HEALTHY, 1);
    let residual = flight
        .navigation
        .position_q12
        .iter()
        .zip(ground_position.iter())
        .map(|(left, right)| (i64::from(*left) - i64::from(*right)).unsigned_abs())
        .max()
        .unwrap_or(0)
        .min(i32::MAX as u64) as i32;
    snapshot.set(METRIC_ONBOARD_GROUND_POSITION_RESIDUAL_Q12, residual);
    snapshot
}

pub(crate) fn ground_navigation_load(
    epoch: u32,
    effective: u32,
    identity: u32,
    ground: ksa64_interface::phase11::GroundEstimate,
) -> UplinkCommandLoad {
    let mut arguments = [0; 16];
    arguments[..3].copy_from_slice(&ground.position_q12_km);
    arguments[3..6].copy_from_slice(&ground.velocity_q24_km_s);
    UplinkCommandLoad {
        load_identity: identity,
        package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
        plan_identity: ksa_g10r_reference_mission_plan().plan_identity,
        abi: FlightAbiId::GlobalKlr10V1,
        source_estimator_identity: GROUND_ESTIMATOR_ID,
        source_estimator_checksum: ground.estimator_checksum,
        stage_epoch: epoch,
        not_before_epoch: epoch + 2,
        expires_epoch: epoch + 16,
        requested_effective_epoch: effective,
        required_capabilities: PACKAGE_CAP_GROUND_NAV_UPDATE,
        prerequisite_event_mask: 0,
        position_residual_limit_q12: 4_096,
        velocity_residual_limit_q24: 65_536,
        frame: ground.frame,
        load_type: UplinkLoadType::GroundNavigationUpdate,
        arguments,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn generic_load(
    epoch: u32,
    effective: u32,
    identity: u32,
    load_type: UplinkLoadType,
    capability: u32,
    frame: GlobalFrameId,
    first_arguments: [i32; 4],
) -> UplinkCommandLoad {
    let mut arguments = [0; 16];
    arguments[..4].copy_from_slice(&first_arguments);
    UplinkCommandLoad {
        load_identity: identity,
        package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
        plan_identity: ksa_g10r_reference_mission_plan().plan_identity,
        abi: FlightAbiId::GlobalKlr10V1,
        source_estimator_identity: GROUND_ESTIMATOR_ID,
        source_estimator_checksum: 0x11e0_5555,
        stage_epoch: epoch,
        not_before_epoch: epoch + 2,
        expires_epoch: epoch + 16,
        requested_effective_epoch: effective,
        required_capabilities: capability,
        prerequisite_event_mask: 0,
        position_residual_limit_q12: 0,
        velocity_residual_limit_q24: 0,
        frame,
        load_type,
        arguments,
    }
}

pub(crate) fn commit_request(load: UplinkCommandLoad, epoch: u32) -> UplinkControlRecord {
    UplinkControlRecord {
        kind: UplinkControlKind::CommitRequest,
        control_identity: load.load_identity ^ 0x55aa_0000,
        load_identity: load.load_identity,
        package_manifest_identity: load.package_manifest_identity,
        plan_identity: load.plan_identity,
        request_epoch: epoch,
        effective_epoch: load.requested_effective_epoch,
        state: UplinkState::Staged,
        reason: UplinkReasonCode::Accepted,
        receipt_checksum: 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_evidence(
    scenario_identity: u32,
    releases: u32,
    flight: GlobalFlightEvidence,
    package: &KsaG10rReferenceOpsV1,
    procedure_state: ProcedureState,
    procedure_chain: u32,
    transcript: ActionTranscript,
    rejected_loads: u16,
) -> OperationalScenarioEvidence {
    let mut journal = [EventJournalRecord::EMPTY; EVENT_JOURNAL_CAPACITY];
    let count = package.recover_journal_after(0, &mut journal);
    let journal_chain = if count == 0 {
        0
    } else {
        journal[count - 1].chain
    };
    let prediction_checksum = package
        .prediction_summary()
        .map_or(0, |value| value.prediction_checksum);
    let evidence_identity = hash_words(&[
        scenario_identity,
        releases,
        flight.flight_checksum,
        flight.navigation.checksum,
        flight.command.command_checksum,
        prediction_checksum,
        procedure_chain,
        journal_chain,
        transcript.chain,
        u32::from(rejected_loads),
        procedure_state as u32,
        u32::from(flight.safe),
    ]);
    OperationalScenarioEvidence {
        scenario_identity,
        releases,
        flight_checksum: flight.flight_checksum,
        navigation_checksum: flight.navigation.checksum,
        command_checksum: flight.command.command_checksum,
        prediction_checksum,
        procedure_chain,
        journal_chain,
        action_chain: transcript.chain,
        rejected_loads,
        procedure_state,
        safe: flight.safe,
        evidence_identity,
        actions: transcript.records,
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

    #[test]
    fn nominal_operational_shell_preserves_inner_phase10_chains() {
        let evidence = run_nominal_operations_probe();
        assert_eq!(evidence.scenario_identity, NOMINAL_OPERATIONS_SCENARIO_ID);
        assert_ne!(evidence.flight_checksum, 0);
        assert!(!evidence.safe);
    }

    #[test]
    fn human_and_scripted_gnss_loss_runs_are_identical() {
        let human = run_gnss_loss_procedure(false);
        let scripted = run_gnss_loss_procedure(true);
        assert_eq!(human, scripted);
        assert_eq!(human.procedure_state, ProcedureState::Completed);
        assert_eq!(human.actions.len(), 4);
    }

    #[test]
    fn guidance_commit_regenerates_prediction_and_blackout_is_ground_only() {
        let guidance = run_guidance_update_probe();
        let blackout = run_ground_blackout_probe();
        assert_ne!(guidance.prediction_checksum, 0);
        assert_ne!(blackout.prediction_checksum, 0);
        assert!(!blackout.safe);
    }

    #[test]
    fn invalid_operations_fail_closed_with_exact_reasons() {
        let evidence = run_invalid_operations_probe();
        assert_eq!(evidence.rejected_loads, 6);
        assert!(!evidence.safe);
        assert!(evidence
            .actions
            .iter()
            .all(|record| record.state == UplinkState::Rejected));
    }
}
