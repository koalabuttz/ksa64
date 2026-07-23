use ksa64_core::phase2_scenario::PHASE2_SCENARIO_IMAGE_LENGTH;
use ksa64_sim::config::{write_phase3_config, PHASE3_CONFIG_LENGTH};
use ksa64_sim::mission::MissionCase;
use std::{fs, path::PathBuf};
const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../phase3/examples");
    for (name, case) in [
        ("ksa3-nominal.ksc3", MissionCase::Nominal),
        ("ksa3-altimeter-dropout.ksc3", MissionCase::AltimeterDropout),
        ("ksa3-gps-outage.ksc3", MissionCase::GpsOutage),
        ("ksa3-steering-stuck.ksc3", MissionCase::SteeringStuck),
    ] {
        let mut bytes = [0u8; PHASE3_CONFIG_LENGTH];
        write_phase3_config(BASE, case, &mut bytes).unwrap();
        fs::write(root.join(name), bytes).unwrap();
    }
}
