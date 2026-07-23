use ksa64_sim::{phase5_telemetry_codec_signature, run_phase5_telemetry_self_tests};
#[test]
fn codec_probe_is_frozen() {
    assert_eq!(phase5_telemetry_codec_signature(), 0x07bc_3e16);
    assert_eq!(run_phase5_telemetry_self_tests(), 0);
}
