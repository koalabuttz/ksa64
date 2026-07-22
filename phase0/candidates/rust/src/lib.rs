#![no_std]

mod manual;
mod optimized;
mod vertical;
pub use manual::{
    divide_scaled_manual, interpolate_fixed_manual, multiply_scaled_manual,
    run_manual_arithmetic_vectors,
};
pub use vertical::{
    hash_vertical_state, run_vertical_kernel_manual, run_vertical_kernel_optimized,
    run_vertical_manual, run_vertical_optimized, vertical_state_matches_checkpoint,
    vertical_step_manual, vertical_step_optimized, VerticalRun, VerticalState,
};

pub mod vertical_vectors {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../generated/phase0_vertical.rs"
    ));
}

pub mod vectors {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../generated/phase0_vectors.rs"
    ));
}

const I32_NEGATIVE_LIMIT: u64 = 1u64 << 31;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticError {
    DivisionByZero,
    ShiftOutOfRange,
}

fn magnitude_i32(value: i32) -> u64 {
    if value < 0 {
        (-(value as i64)) as u64
    } else {
        value as u64
    }
}

fn saturate_signed_magnitude(magnitude: u64, negative: bool) -> i32 {
    if negative {
        if magnitude >= I32_NEGATIVE_LIMIT {
            i32::MIN
        } else {
            -(magnitude as i32)
        }
    } else if magnitude > i32::MAX as u64 {
        i32::MAX
    } else {
        magnitude as i32
    }
}

fn saturate_i64(value: i64) -> i32 {
    if value > i32::MAX as i64 {
        i32::MAX
    } else if value < i32::MIN as i64 {
        i32::MIN
    } else {
        value as i32
    }
}

fn rounded_unsigned_ratio(numerator: u64, denominator: u64) -> u64 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder >= denominator - remainder {
        quotient + 1
    } else {
        quotient
    }
}

pub fn multiply_scaled(a: i32, b: i32, shift: u8) -> Result<i32, ArithmeticError> {
    if shift > 31 {
        return Err(ArithmeticError::ShiftOutOfRange);
    }

    let negative = (a < 0) ^ (b < 0);
    let product = magnitude_i32(a) * magnitude_i32(b);
    let rounded = rounded_unsigned_ratio(product, 1u64 << shift);
    Ok(saturate_signed_magnitude(rounded, negative))
}

pub fn divide_scaled(numerator: i32, denominator: i32, shift: u8) -> Result<i32, ArithmeticError> {
    if denominator == 0 {
        return Err(ArithmeticError::DivisionByZero);
    }
    if shift > 31 {
        return Err(ArithmeticError::ShiftOutOfRange);
    }

    let negative = (numerator < 0) ^ (denominator < 0);
    let shifted = magnitude_i32(numerator) << shift;
    let rounded = rounded_unsigned_ratio(shifted, magnitude_i32(denominator));
    Ok(saturate_signed_magnitude(rounded, negative))
}

pub fn interpolate_fixed(x: i32, xs: &[i32], ys: &[i32]) -> i32 {
    debug_assert!(!xs.is_empty());
    debug_assert_eq!(xs.len(), ys.len());

    if x <= xs[0] {
        return ys[0];
    }

    let last = xs.len() - 1;
    if x >= xs[last] {
        return ys[last];
    }

    let mut index = 0usize;
    while index < last {
        let x0 = xs[index];
        let x1 = xs[index + 1];
        if x < x1 {
            let mut fraction = divide_scaled(x - x0, x1 - x0, 16)
                .expect("environment knots must be strictly increasing");
            fraction = fraction.clamp(0, 65_535);
            let delta = multiply_scaled(ys[index + 1] - ys[index], fraction, 16)
                .expect("Phase 0 interpolation shift is valid");
            return saturate_i64(ys[index] as i64 + delta as i64);
        }
        index += 1;
    }

    unreachable!()
}

pub fn run_arithmetic_vectors() -> u16 {
    let mut failures = 0u16;

    let mut index = 0usize;
    while index < vectors::MULTIPLY_VECTORS.len() {
        let vector = vectors::MULTIPLY_VECTORS[index];
        match multiply_scaled(vector.a, vector.b, vector.shift) {
            Ok(actual) if actual == vector.expected => {}
            _ => failures += 1,
        }
        index += 1;
    }

    index = 0;
    while index < vectors::DIVIDE_VECTORS.len() {
        let vector = vectors::DIVIDE_VECTORS[index];
        match divide_scaled(vector.numerator, vector.denominator, vector.shift) {
            Ok(actual) if actual == vector.expected => {}
            _ => failures += 1,
        }
        index += 1;
    }

    index = 0;
    while index < vectors::INTERPOLATION_VECTORS.len() {
        let vector = vectors::INTERPOLATION_VECTORS[index];
        let density = interpolate_fixed(
            vector.altitude_q12,
            vectors::ALTITUDE_KNOTS_Q12,
            vectors::DENSITY_Q28,
        );
        let gravity = interpolate_fixed(
            vector.altitude_q12,
            vectors::ALTITUDE_KNOTS_Q12,
            vectors::GRAVITY_Q28,
        );
        if density != vector.density_q28 {
            failures += 1;
        }
        if gravity != vector.gravity_q28 {
            failures += 1;
        }
        index += 1;
    }

    match divide_scaled(1, 0, 0) {
        Err(ArithmeticError::DivisionByZero) => {}
        _ => failures += 1,
    }
    match multiply_scaled(1, 1, 32) {
        Err(ArithmeticError::ShiftOutOfRange) => {}
        _ => failures += 1,
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_arithmetic_vectors_pass() {
        assert_eq!(run_arithmetic_vectors(), 0);
    }

    #[test]
    fn wrappers_preserve_exact_storage_width() {
        #[repr(transparent)]
        struct Altitude(i32);
        assert_eq!(core::mem::size_of::<Altitude>(), 4);
    }
}
