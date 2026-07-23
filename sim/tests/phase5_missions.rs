use ksa64_sim::phase5_mission::{
    rcs_depletion_event_seen, run_phase5_mission, Phase5MissionCase, Phase5MissionOutcome,
};

const EXPECTED_OUTCOMES: [Phase5MissionOutcome; 6] = [
    Phase5MissionOutcome::StableOrbit,
    Phase5MissionOutcome::StableOrbit,
    Phase5MissionOutcome::StableOrbit,
    Phase5MissionOutcome::Aborted,
    Phase5MissionOutcome::Aborted,
    Phase5MissionOutcome::StableOrbit,
];
const EXPECTED_STEPS: [u32; 6] = [3133, 3133, 3133, 1103, 958, 3133];
const EXPECTED_EVENTS: [u16; 6] = [7, 7, 7, 515, 3, 263];
const EXPECTED_SUMMARY_CHECKSUMS: [u32; 6] = [
    557_491_580,
    977_608_682,
    1_801_330_793,
    3_230_120_338,
    3_522_370_491,
    2_173_775_322,
];

#[test]
fn reviewed_phase5_missions_match_frozen_evidence() {
    for (index, case) in Phase5MissionCase::ALL.into_iter().enumerate() {
        let summary = run_phase5_mission(case).unwrap();
        assert_eq!(summary.outcome, EXPECTED_OUTCOMES[index], "{case:?}");
        assert_eq!(summary.steps, EXPECTED_STEPS[index], "{case:?}");
        assert_eq!(summary.events, EXPECTED_EVENTS[index], "{case:?}");
        assert_eq!(
            summary.summary_checksum, EXPECTED_SUMMARY_CHECKSUMS[index],
            "{case:?}"
        );
    }
}

#[test]
fn nominal_mission_is_repeatable_and_guidance_probe_is_frozen() {
    let first = run_phase5_mission(Phase5MissionCase::Nominal).unwrap();
    let second = run_phase5_mission(Phase5MissionCase::Nominal).unwrap();
    assert_eq!(first, second);
    #[cfg(feature = "fixtures")]
    assert_eq!(ksa64_sim::run_phase5_guidance_self_tests(), 0);
}

#[test]
fn rcs_leak_case_depletes_without_preventing_insertion() {
    let summary = run_phase5_mission(Phase5MissionCase::RcsLeakAndDepletion).unwrap();
    assert_eq!(summary.outcome, Phase5MissionOutcome::StableOrbit);
    assert!(rcs_depletion_event_seen(summary));
}
