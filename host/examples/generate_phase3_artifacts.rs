use std::fs;
use std::path::PathBuf;

use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_host::phase3::{
    capture_phase3_mission, derive_validated_phase3_replay, inspect_phase3_stream,
};
use ksa64_interface::crc32_ieee;
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::mission::MissionCase;

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const CASES: [(&str, MissionCase, &[u8; PHASE3_CONFIG_LENGTH]); 4] = [
    (
        "nominal",
        MissionCase::Nominal,
        include_bytes!("../../phase3/examples/ksa3-nominal.ksc3"),
    ),
    (
        "altimeter-dropout",
        MissionCase::AltimeterDropout,
        include_bytes!("../../phase3/examples/ksa3-altimeter-dropout.ksc3"),
    ),
    (
        "gps-outage",
        MissionCase::GpsOutage,
        include_bytes!("../../phase3/examples/ksa3-gps-outage.ksc3"),
    ),
    (
        "steering-stuck",
        MissionCase::SteeringStuck,
        include_bytes!("../../phase3/examples/ksa3-steering-stuck.ksc3"),
    ),
];

fn main() {
    let scenario = parse_phase2_scenario(BASE).expect("frozen KSA-2A scenario");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../phase3/examples");
    for (name, case, config) in CASES {
        let mut stream = Vec::new();
        let (result, _) = capture_phase3_mission(
            &scenario,
            crc32_ieee(&BASE[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
            crc32_ieee(&config[..PHASE3_CONFIG_LENGTH - 4]),
            case,
            &mut stream,
        )
        .expect("capture Phase 3 mission");
        let inspection =
            inspect_phase3_stream(&stream, BASE, config).expect("strict KST3 validation");
        let replay = derive_validated_phase3_replay(&stream, BASE, config)
            .expect("validated KRP3 derivation");
        fs::write(root.join(format!("ksa3-{name}.kst3")), &stream).expect("write KST3");
        fs::write(root.join(format!("ksa3-{name}.krp3")), &replay).expect("write KRP3");
        println!(
            "{name}: {} KST3 frames, {} bytes; {} KRP3 bytes; terminal step {}; checksums {:08x}/{:08x}/{:08x}/{:08x}",
            inspection.frame_count,
            stream.len(),
            replay.len(),
            inspection.final_frame.step,
            result.truth_checksum,
            result.sensor_checksum,
            result.nav_checksum,
            result.flight_checksum,
        );
    }
}
