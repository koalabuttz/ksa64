use ksa64_phase0_rust::run_manual_arithmetic_vectors;

#[test]
fn manual_two_word_vectors_pass_natively() {
    assert_eq!(run_manual_arithmetic_vectors(), 0);
}
