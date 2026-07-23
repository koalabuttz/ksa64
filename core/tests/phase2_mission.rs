use ksa64_core::phase2_mission::{
    execute_phase2_mission, Phase2MissionOutcome, EVENT_CUTOFF, EVENT_END, EVENT_IGNITION,
    EVENT_SEPARATION,
};
use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_core::planar::OrbitClass;

const NOMINAL: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const FAILURE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-early-cutoff.ksc2");

#[test]
fn nominal_mission_reaches_the_declared_orbit_envelope() {
    let scenario = parse_phase2_scenario(NOMINAL).unwrap();
    let result = execute_phase2_mission(&scenario).unwrap();
    eprintln!(
        "nominal terminal: outcome={:?} step={} cutoff={} altitude_raw={} vr_raw={} h_raw={}",
        result.outcome(),
        result.truth().step(),
        result.cutoff_step(),
        result.truth().radius().raw() - 26_124_849,
        result.truth().radial_velocity().raw(),
        result.truth().specific_angular_momentum().raw()
    );
    let orbit = result.cutoff_orbit().unwrap();
    let earth_q12 = 26_124_849;
    let perigee_km = (orbit.perigee().raw() - earth_q12) as f64 / 4096.0;
    let apogee_km = (orbit.apogee().raw() - earth_q12) as f64 / 4096.0;
    eprintln!(
        "fixed nominal: cutoff={} perigee={perigee_km:.6} apogee={apogee_km:.6} per_raw={} apo_raw={} e={} maxq={} accel={}",
        result.cutoff_step(),
        orbit.perigee().raw(),
        orbit.apogee().raw(),
        orbit.eccentricity().raw(),
        result.max_dynamic_pressure().raw(),
        result.max_proper_acceleration().raw(),
    );
    assert_eq!(result.outcome(), Phase2MissionOutcome::DurationComplete);
    assert_eq!(result.truth().step(), scenario.steps());
    assert_eq!(orbit.class(), OrbitClass::StableOrbit);
    assert!((180.0..=220.0).contains(&perigee_km));
    assert!((180.0..=220.0).contains(&apogee_km));
    assert!(orbit.eccentricity().raw() <= 655);
    assert!(result.max_dynamic_pressure().raw() <= 60 * 65_536);
    assert!(result.max_proper_acceleration().raw() <= 16_106_128);
    assert_eq!(result.cutoff_step(), 3_172);
    assert_eq!(
        result.event_history() & (EVENT_IGNITION | EVENT_CUTOFF | EVENT_SEPARATION | EVENT_END),
        EVENT_IGNITION | EVENT_CUTOFF | EVENT_SEPARATION | EVENT_END
    );
}

#[test]
fn early_cutoff_variant_is_deterministically_not_orbital() {
    let scenario = parse_phase2_scenario(FAILURE).unwrap();
    let result = execute_phase2_mission(&scenario).unwrap();
    eprintln!(
        "failure terminal: outcome={:?} step={} cutoff={} altitude_raw={} vr_raw={} h_raw={}",
        result.outcome(),
        result.truth().step(),
        result.cutoff_step(),
        result.truth().radius().raw() - 26_124_849,
        result.truth().radial_velocity().raw(),
        result.truth().specific_angular_momentum().raw()
    );
    let cutoff = result.cutoff_orbit().unwrap();
    eprintln!(
        "fixed failure: outcome={:?} step={} cutoff={} class={:?}",
        result.outcome(),
        result.truth().step(),
        result.cutoff_step(),
        cutoff.class(),
    );
    assert_eq!(result.outcome(), Phase2MissionOutcome::DurationComplete);
    assert_ne!(cutoff.class(), OrbitClass::StableOrbit);
    assert_ne!(
        result.terminal_orbit().unwrap().class(),
        OrbitClass::StableOrbit
    );
    assert!(result.cutoff_step() < 3_172);
}
#[test]
fn target_fixture_self_tests_match_native_execution() {
    assert_eq!(ksa64_core::run_phase2_nominal_mission_self_tests(), 0);
    assert_eq!(ksa64_core::run_phase2_failure_mission_self_tests(), 0);
}
