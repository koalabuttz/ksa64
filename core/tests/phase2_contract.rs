use ksa64_core::phase2_numeric::{
    sin_cos_pitch_q15, sqrt_floor_u32, EARTH_MU_Q12, EARTH_RADIUS_Q12, PHASE2_NUMERIC_CONTRACT,
};
use ksa64_core::phase2_quantities::{DownrangeAngle, PitchAngle};

#[test]
fn generated_contract_identity_and_earth_constants_are_frozen() {
    assert_eq!(PHASE2_NUMERIC_CONTRACT, "ksa64.numeric.phase2-v1");
    assert_eq!(EARTH_RADIUS_Q12, 26_124_849);
    assert_eq!(EARTH_MU_Q12, 1_632_667_410);
}

#[test]
fn integer_square_root_matches_boundaries() {
    for value in [
        0,
        1,
        2,
        3,
        4,
        15,
        16,
        17,
        65_535,
        65_536,
        0x7fff_ffff,
        0xffff_ffff,
    ] {
        let root = sqrt_floor_u32(value);
        assert!(root <= value / root.max(1));
        let next = root + 1;
        assert!(next > value / next);
    }
}

#[test]
fn pitch_trig_has_exact_endpoints() {
    assert_eq!(sin_cos_pitch_q15(PitchAngle::RADIAL), Some((0, 32767)));
    assert_eq!(sin_cos_pitch_q15(PitchAngle::PROGRADE), Some((32767, 0)));
    assert!(sin_cos_pitch_q15(PitchAngle::from_raw(16_385)).is_none());
    let (sine, cosine) = sin_cos_pitch_q15(PitchAngle::from_raw(8192)).unwrap();
    assert!((sine as i32 - 23_170).abs() <= 1);
    assert!((cosine as i32 - 23_170).abs() <= 1);
}

#[test]
fn downrange_angle_wraps_by_contract() {
    assert_eq!(
        DownrangeAngle::from_raw(i32::MAX).wrapping_add_raw(1).raw(),
        i32::MIN
    );
}
