use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_interface::FlightMode;
use ksa64_sim::probe::*;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");

#[test]
fn finite_probe_results_are_deterministic_and_fault_path_safes() {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let a = run_composed_probe(&scenario).unwrap();
    let b = run_composed_probe(&scenario).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.step, PROBE_STEPS);
    let gps_a = run_guidance_probe(false);
    let gps_b = run_guidance_probe(false);
    assert_eq!(gps_a, gps_b);
    assert_eq!(gps_a.mode, FlightMode::Insertion);
    let fault = run_guidance_probe(true);
    assert!(command_is_safe_after_fault(fault));
    assert_eq!(run_coast_probe().unwrap().step, PROBE_STEPS);
    assert_eq!(run_actuator_probe(), run_actuator_probe());
}
