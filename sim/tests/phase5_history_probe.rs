#[test]
fn probe_signature_is_frozen() {
    assert_eq!(ksa64_sim::phase5_history_probe_signature(), 0xb578_3bf2);
}
