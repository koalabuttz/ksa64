use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_sim::probe::{
    run_actuator_probe, run_coast_probe, run_composed_probe, run_guidance_probe,
};

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");

fn main() {
    let scenario = parse_phase2_scenario(BASE).expect("frozen scenario");
    let composed = run_composed_probe(&scenario).expect("composed probe");
    let guidance = run_guidance_probe(false);
    let fault = run_guidance_probe(true);
    let coast = run_coast_probe().expect("coast probe");
    println!(
        concat!(
            "{{\"truth_checksum\":{},\"sensor_checksum\":{},\"nav_checksum\":{},",
            "\"flight_checksum\":{},\"radius_q12\":{},\"guidance_nav_checksum\":{},",
            "\"guidance_flight_checksum\":{},\"fault_nav_checksum\":{},",
            "\"fault_flight_checksum\":{},\"coast_radius_q12\":{},",
            "\"coast_radial_velocity_q24\":{},\"actuator_hash\":{},",
            "\"guidance_mode\":{},\"fault_mode\":{}}}"
        ),
        composed.truth_checksum,
        composed.sensor_checksum,
        composed.nav_checksum,
        composed.flight_checksum,
        composed.radius_q12,
        guidance.nav_checksum,
        guidance.flight_checksum,
        fault.nav_checksum,
        fault.flight_checksum,
        coast.radius_q12,
        coast.radial_velocity_q24,
        run_actuator_probe(),
        guidance.mode as u8,
        fault.mode as u8,
    );
}
