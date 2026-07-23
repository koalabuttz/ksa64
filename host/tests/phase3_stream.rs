use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_host::phase3::*;
use ksa64_interface::crc32_ieee;
use ksa64_sim::config::PHASE3_CONFIG_LENGTH;
use ksa64_sim::mission::MissionCase;
use ksa64_sim::telemetry::{PHASE3_TELEMETRY_FRAME_LENGTH, PHASE3_TELEMETRY_HEADER_LENGTH};

const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const CONFIG: &[u8; PHASE3_CONFIG_LENGTH] =
    include_bytes!("../../phase3/examples/ksa3-nominal.ksc3");

fn capture() -> Vec<u8> {
    let scenario = parse_phase2_scenario(BASE).unwrap();
    let mut bytes = Vec::new();
    capture_phase3_mission(
        &scenario,
        crc32_ieee(&BASE[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
        crc32_ieee(&CONFIG[..PHASE3_CONFIG_LENGTH - 4]),
        MissionCase::Nominal,
        &mut bytes,
    )
    .unwrap();
    bytes
}

#[test]
fn strict_inspector_accepts_capture_and_replay_is_deterministic() {
    let stream = capture();
    let inspection = inspect_phase3_stream(&stream, BASE, CONFIG).unwrap();
    assert_eq!(inspection.first_frame.step, 0);
    assert_eq!(inspection.final_frame.step, 7200);
    assert_eq!(inspection.terminal_frames, 1);
    assert!(inspection.event_frames >= 5);
    let a = derive_validated_phase3_replay(&stream, BASE, CONFIG).unwrap();
    let b = derive_validated_phase3_replay(&stream, BASE, CONFIG).unwrap();
    assert_eq!(a, b);
    assert_eq!(&a[..4], b"KRP3");
    assert_eq!(
        a.len(),
        PHASE3_REPLAY_HEADER_LENGTH + inspection.frame_count * PHASE3_REPLAY_FRAME_LENGTH
    );
}

#[test]
fn first_bad_frame_is_reported_and_replay_refuses_unvalidated_input() {
    let mut stream = capture();
    let bad_index = 7usize;
    let at = PHASE3_TELEMETRY_HEADER_LENGTH + bad_index * PHASE3_TELEMETRY_FRAME_LENGTH + 84;
    stream[at] ^= 1;
    assert!(matches!(
        inspect_phase3_stream(&stream, BASE, CONFIG),
        Err(Phase3StreamInspectionError::Frame { index: 7, .. })
    ));
    assert!(derive_validated_phase3_replay(&stream, BASE, CONFIG).is_err());
}

#[test]
fn framing_and_exact_config_identity_fail_closed() {
    let stream = capture();
    assert_eq!(
        inspect_phase3_stream(&stream[..stream.len() - 1], BASE, CONFIG),
        Err(Phase3StreamInspectionError::Framing)
    );
    let mut wrong = *CONFIG;
    wrong[20] ^= 1;
    assert!(matches!(
        inspect_phase3_stream(&stream, BASE, &wrong),
        Err(Phase3StreamInspectionError::Config(_))
    ));
}

#[test]
fn frozen_kst3_streams_validate_and_are_the_only_source_of_frozen_krp3() {
    let artifacts: [(&[u8], &[u8; PHASE3_CONFIG_LENGTH], &[u8]); 4] = [
        (
            include_bytes!("../../phase3/examples/ksa3-nominal.kst3"),
            include_bytes!("../../phase3/examples/ksa3-nominal.ksc3"),
            include_bytes!("../../phase3/examples/ksa3-nominal.krp3"),
        ),
        (
            include_bytes!("../../phase3/examples/ksa3-altimeter-dropout.kst3"),
            include_bytes!("../../phase3/examples/ksa3-altimeter-dropout.ksc3"),
            include_bytes!("../../phase3/examples/ksa3-altimeter-dropout.krp3"),
        ),
        (
            include_bytes!("../../phase3/examples/ksa3-gps-outage.kst3"),
            include_bytes!("../../phase3/examples/ksa3-gps-outage.ksc3"),
            include_bytes!("../../phase3/examples/ksa3-gps-outage.krp3"),
        ),
        (
            include_bytes!("../../phase3/examples/ksa3-steering-stuck.kst3"),
            include_bytes!("../../phase3/examples/ksa3-steering-stuck.ksc3"),
            include_bytes!("../../phase3/examples/ksa3-steering-stuck.krp3"),
        ),
    ];
    for (stream, config, replay) in artifacts {
        inspect_phase3_stream(stream, BASE, config).unwrap();
        assert_eq!(
            derive_validated_phase3_replay(stream, BASE, config).unwrap(),
            replay
        );
    }
}
