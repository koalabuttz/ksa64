use ksa64_core::evaluation::{EvaluationOutcome, MetricSlot, ModelProfileId};
use ksa64_core::phase2_mission::execute_phase2_mission;
use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_sim::evaluation::{evaluate, EvaluationRequest};
use ksa64_sim::phase5_mission::{run_phase5_mission, Phase5MissionCase};

const SCENARIO: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");

#[test]
fn phase2_facade_preserves_legacy_result() {
    let scenario = parse_phase2_scenario(SCENARIO).unwrap();
    let direct = execute_phase2_mission(&scenario).unwrap();
    let adapted = evaluate(EvaluationRequest::LegacyKsa2PlanarV1(&scenario)).unwrap();
    let truth = direct.truth();

    assert_eq!(adapted.profile, ModelProfileId::LegacyKsa2PlanarV1);
    assert_eq!(adapted.outcome, EvaluationOutcome::StableOrbit);
    assert_eq!(adapted.steps, truth.step());
    assert_eq!(
        adapted.terminal_state_a,
        [truth.radius().raw(), truth.downrange().raw(), 0]
    );
    assert_eq!(
        adapted.terminal_state_b,
        [
            truth.radial_velocity().raw(),
            truth.specific_angular_momentum().raw(),
            0
        ]
    );
    assert_eq!(
        adapted.metric(MetricSlot::MaxDynamicPressure),
        Some(direct.max_dynamic_pressure().raw())
    );
    assert_eq!(adapted.source_checksums[0], direct.state_checksum());
}

#[test]
fn phase5_facade_preserves_legacy_result() {
    let direct = run_phase5_mission(Phase5MissionCase::Nominal).unwrap();
    let adapted = evaluate(EvaluationRequest::LegacyKsa5SpatialV1(
        Phase5MissionCase::Nominal,
    ))
    .unwrap();

    assert_eq!(adapted.profile, ModelProfileId::LegacyKsa5SpatialV1);
    assert_eq!(adapted.outcome, EvaluationOutcome::StableOrbit);
    assert_eq!(adapted.steps, direct.steps);
    assert_eq!(adapted.terminal_state_a, direct.terminal_position_q12);
    assert_eq!(adapted.terminal_state_b, direct.terminal_velocity_q24);
    assert_eq!(
        adapted.metric(MetricSlot::MaxDynamicPressure),
        Some(direct.max_dynamic_pressure_q16)
    );
    assert_eq!(
        adapted.metric(MetricSlot::MaxNavigationError),
        Some(direct.max_nav_position_error_q12)
    );
    assert_eq!(
        adapted.source_checksums,
        [
            direct.sensor_checksum,
            direct.navigation_checksum,
            direct.flight_checksum,
            direct.summary_checksum,
            0
        ]
    );
}

#[test]
fn request_reports_profile_without_execution() {
    let request = EvaluationRequest::LegacyKsa5SpatialV1(Phase5MissionCase::Nominal);
    assert_eq!(request.profile(), ModelProfileId::LegacyKsa5SpatialV1);
}
