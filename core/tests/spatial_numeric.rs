use core::mem::size_of;

use ksa64_core::numeric::{sqrt_floor_scaled_u32, NumericFault, NumericStatus};
use ksa64_core::spatial_numeric::{FixedVec3, QuaternionQ30};

mod vectors {
    include!("../../phase5/generated/spatial_vectors_v1.rs");
}

fn vector<const F: u8>(raw: [i32; 3]) -> FixedVec3<F> {
    FixedVec3::new(raw[0], raw[1], raw[2])
}

fn quaternion(raw: [i32; 4]) -> QuaternionQ30 {
    QuaternionQ30::new(raw[0], raw[1], raw[2], raw[3])
}

fn quaternion_raw(value: QuaternionQ30) -> [i32; 4] {
    [value.w(), value.x(), value.y(), value.z()]
}

fn vector_raw<const F: u8>(value: FixedVec3<F>) -> [i32; 3] {
    [value.x(), value.y(), value.z()]
}

#[test]
fn spatial_values_have_only_the_declared_words() {
    assert_eq!(size_of::<FixedVec3<12>>(), 12);
    assert_eq!(size_of::<QuaternionQ30>(), 16);
}

#[test]
fn explicit_two_word_square_root_matches_boundaries() {
    let cases = [
        (0, 0, 0),
        (1, 0, 1),
        (2, 0, 1),
        (4, 0, 2),
        (1 << 30, 30, 1 << 30),
        (u32::MAX, 0, 65_535),
    ];
    for (value, shift, expected) in cases {
        let mut status = NumericStatus::CLEAR;
        assert_eq!(sqrt_floor_scaled_u32(value, shift, &mut status), expected);
        assert!(status.is_clear());
    }
    let mut status = NumericStatus::CLEAR;
    assert_eq!(sqrt_floor_scaled_u32(1, 32, &mut status), 0);
    assert!(status.contains(NumericFault::InvalidShift));
}

#[test]
fn vector_dot_and_cross_match_independent_integer_vectors() {
    let a = vector::<16>(vectors::VECTOR_A_Q16);
    let b = vector::<16>(vectors::VECTOR_B_Q16);
    let mut status = NumericStatus::CLEAR;
    assert_eq!(a.dot_scaled::<16>(b, &mut status), vectors::DOT_Q16);
    assert_eq!(
        vector_raw(a.cross_scaled::<16>(b, &mut status)),
        vectors::CROSS_Q16
    );
    assert!(status.is_clear());
}

#[test]
fn hamilton_product_and_rotation_match_independent_vectors() {
    let qz = quaternion(vectors::QZ90_Q30);
    let qx = quaternion(vectors::QX90_Q30);
    let mut status = NumericStatus::CLEAR;
    assert_eq!(
        quaternion_raw(qz.hamilton(qx, &mut status)),
        vectors::HAMILTON_Q30
    );
    assert_eq!(
        vector_raw(qz.rotate(vector::<16>([1 << 16, 0, 0]), &mut status)),
        vectors::ROTATED_X_Q16
    );
    assert!(status.is_clear());
}

#[test]
fn normalization_matches_independent_isqrt_reference() {
    let mut status = NumericStatus::CLEAR;
    let normalized = quaternion(vectors::UNNORMALIZED_Q30).normalized(&mut status);
    assert_eq!(quaternion_raw(normalized), vectors::NORMALIZED_Q30);
    let norm_error = (normalized.norm_squared_q30(&mut status) - QuaternionQ30::ONE).abs();
    assert!(norm_error <= 2);
    assert!(status.is_clear());
}

#[test]
fn invalid_scales_and_zero_quaternion_fail_closed() {
    let a = FixedVec3::<31>::new(1, 2, 3);
    let mut status = NumericStatus::CLEAR;
    assert_eq!(a.dot_scaled::<0>(a, &mut status), 0);
    assert!(status.contains(NumericFault::InvalidShift));

    let mut status = NumericStatus::CLEAR;
    assert_eq!(
        QuaternionQ30::new(0, 0, 0, 0).normalized(&mut status),
        QuaternionQ30::IDENTITY
    );
    assert!(status.contains(NumericFault::InvalidInput));
}
