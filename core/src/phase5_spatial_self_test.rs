use crate::numeric::{sqrt_floor_scaled_u32, NumericStatus};
use crate::spatial_numeric::{FixedVec3, QuaternionQ30};

mod vectors {
    include!("../../phase5/generated/spatial_vectors_v1.rs");
}

#[inline]
fn failure(value: bool) -> u32 {
    if value {
        0
    } else {
        1
    }
}

pub fn run_phase5_spatial_self_tests() -> u32 {
    let mut failures = 0u32;
    let mut status = NumericStatus::CLEAR;
    failures += failure(sqrt_floor_scaled_u32(1 << 30, 30, &mut status) == 1 << 30);

    let a = FixedVec3::<16>::new(
        vectors::VECTOR_A_Q16[0],
        vectors::VECTOR_A_Q16[1],
        vectors::VECTOR_A_Q16[2],
    );
    let b = FixedVec3::<16>::new(
        vectors::VECTOR_B_Q16[0],
        vectors::VECTOR_B_Q16[1],
        vectors::VECTOR_B_Q16[2],
    );
    failures += failure(a.dot_scaled::<16>(b, &mut status) == vectors::DOT_Q16);
    let cross = a.cross_scaled::<16>(b, &mut status);
    failures += failure([cross.x(), cross.y(), cross.z()] == vectors::CROSS_Q16);

    let qz = QuaternionQ30::new(
        vectors::QZ90_Q30[0],
        vectors::QZ90_Q30[1],
        vectors::QZ90_Q30[2],
        vectors::QZ90_Q30[3],
    );
    let qx = QuaternionQ30::new(
        vectors::QX90_Q30[0],
        vectors::QX90_Q30[1],
        vectors::QX90_Q30[2],
        vectors::QX90_Q30[3],
    );
    let product = qz.hamilton(qx, &mut status);
    failures +=
        failure([product.w(), product.x(), product.y(), product.z()] == vectors::HAMILTON_Q30);
    let rotated = qz.rotate(FixedVec3::<16>::new(1 << 16, 0, 0), &mut status);
    failures += failure([rotated.x(), rotated.y(), rotated.z()] == vectors::ROTATED_X_Q16);

    let source = QuaternionQ30::new(
        vectors::UNNORMALIZED_Q30[0],
        vectors::UNNORMALIZED_Q30[1],
        vectors::UNNORMALIZED_Q30[2],
        vectors::UNNORMALIZED_Q30[3],
    );
    let normalized = source.normalized(&mut status);
    failures += failure(
        [
            normalized.w(),
            normalized.x(),
            normalized.y(),
            normalized.z(),
        ] == vectors::NORMALIZED_Q30,
    );
    failures + failure(status.is_clear())
}
