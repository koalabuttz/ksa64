use crate::{vectors, ArithmeticError};

#[derive(Clone, Copy)]
struct Unsigned64 {
    low: u32,
    high: u32,
}

fn magnitude(value: i32) -> u32 {
    if value == i32::MIN {
        0x8000_0000
    } else if value < 0 {
        (-value) as u32
    } else {
        value as u32
    }
}

fn add_words(value: &mut Unsigned64, add_low: u32, add_high: u32) {
    let previous = value.low;
    value.low = value.low.wrapping_add(add_low);
    value.high = value.high.wrapping_add(add_high);
    if value.low < previous {
        value.high = value.high.wrapping_add(1);
    }
}

fn multiply_unsigned_32(a: u32, b: u32) -> Unsigned64 {
    let a_low = a & 0xffff;
    let a_high = a >> 16;
    let b_low = b & 0xffff;
    let b_high = b >> 16;

    let product_00 = a_low * b_low;
    let product_01 = a_low * b_high;
    let product_10 = a_high * b_low;
    let product_11 = a_high * b_high;

    let mut result = Unsigned64 {
        low: product_00,
        high: 0,
    };
    add_words(&mut result, product_01 << 16, product_01 >> 16);
    add_words(&mut result, product_10 << 16, product_10 >> 16);
    result.high = result.high.wrapping_add(product_11);
    result
}

fn shift_left_32(value: u32, shift: u8) -> Unsigned64 {
    if shift == 0 {
        Unsigned64 {
            low: value,
            high: 0,
        }
    } else {
        Unsigned64 {
            low: value << shift,
            high: value >> (32 - shift),
        }
    }
}

fn shift_right(value: Unsigned64, shift: u8) -> Unsigned64 {
    if shift == 0 {
        value
    } else {
        Unsigned64 {
            low: (value.low >> shift) | (value.high << (32 - shift)),
            high: value.high >> shift,
        }
    }
}

fn increment(value: &mut Unsigned64) {
    value.low = value.low.wrapping_add(1);
    if value.low == 0 {
        value.high = value.high.wrapping_add(1);
    }
}

fn bit_at(value: Unsigned64, position: u8) -> u8 {
    if position < 32 {
        ((value.low >> position) & 1) as u8
    } else {
        ((value.high >> (position - 32)) & 1) as u8
    }
}

fn set_bit(value: &mut Unsigned64, position: u8) {
    if position < 32 {
        value.low |= 1u32 << position;
    } else {
        value.high |= 1u32 << (position - 32);
    }
}

fn divide_unsigned_64_by_32(numerator: Unsigned64, denominator: u32) -> (Unsigned64, u32) {
    let mut quotient = Unsigned64 { low: 0, high: 0 };
    let mut remainder = 0u32;
    let mut position = 64u8;

    while position != 0 {
        position -= 1;
        remainder = (remainder << 1) | bit_at(numerator, position) as u32;
        if remainder >= denominator {
            remainder -= denominator;
            set_bit(&mut quotient, position);
        }
    }
    (quotient, remainder)
}

fn signed_saturate(value: Unsigned64, negative: bool) -> i32 {
    if negative {
        if value.high != 0 || value.low >= 0x8000_0000 {
            i32::MIN
        } else {
            -(value.low as i32)
        }
    } else if value.high != 0 || value.low > 0x7fff_ffff {
        i32::MAX
    } else {
        value.low as i32
    }
}

fn saturating_add(a: i32, b: i32) -> i32 {
    if b > 0 && a > i32::MAX - b {
        i32::MAX
    } else if b < 0 && a < i32::MIN - b {
        i32::MIN
    } else {
        a + b
    }
}

pub fn multiply_scaled_manual(a: i32, b: i32, shift: u8) -> Result<i32, ArithmeticError> {
    if shift > 31 {
        return Err(ArithmeticError::ShiftOutOfRange);
    }

    let negative = (a < 0) ^ (b < 0);
    let mut product = multiply_unsigned_32(magnitude(a), magnitude(b));
    if shift != 0 {
        add_words(&mut product, 1u32 << (shift - 1), 0);
    }
    Ok(signed_saturate(shift_right(product, shift), negative))
}

pub fn divide_scaled_manual(
    numerator: i32,
    denominator: i32,
    shift: u8,
) -> Result<i32, ArithmeticError> {
    if denominator == 0 {
        return Err(ArithmeticError::DivisionByZero);
    }
    if shift > 31 {
        return Err(ArithmeticError::ShiftOutOfRange);
    }

    let negative = (numerator < 0) ^ (denominator < 0);
    let divisor = magnitude(denominator);
    let shifted = shift_left_32(magnitude(numerator), shift);
    let (mut quotient, remainder) = divide_unsigned_64_by_32(shifted, divisor);
    if remainder >= divisor - remainder {
        increment(&mut quotient);
    }
    Ok(signed_saturate(quotient, negative))
}

pub fn interpolate_fixed_manual(x: i32, xs: &[i32], ys: &[i32]) -> i32 {
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
            let mut fraction = divide_scaled_manual(x - x0, x1 - x0, 16).unwrap_or(0);
            if fraction < 0 {
                fraction = 0;
            } else if fraction > 65_535 {
                fraction = 65_535;
            }
            let delta =
                multiply_scaled_manual(ys[index + 1] - ys[index], fraction, 16).unwrap_or(0);
            return saturating_add(ys[index], delta);
        }
        index += 1;
    }
    ys[last]
}

pub fn run_manual_arithmetic_vectors() -> u16 {
    let mut failures = 0u16;
    let mut index = 0usize;

    while index < vectors::MULTIPLY_VECTORS.len() {
        let vector = vectors::MULTIPLY_VECTORS[index];
        let actual = multiply_scaled_manual(vector.a, vector.b, vector.shift).unwrap_or(0);
        if actual != vector.expected {
            failures += 1;
        }
        index += 1;
    }

    index = 0;
    while index < vectors::DIVIDE_VECTORS.len() {
        let vector = vectors::DIVIDE_VECTORS[index];
        let actual =
            divide_scaled_manual(vector.numerator, vector.denominator, vector.shift).unwrap_or(0);
        if actual != vector.expected {
            failures += 1;
        }
        index += 1;
    }

    index = 0;
    while index < vectors::INTERPOLATION_VECTORS.len() {
        let vector = vectors::INTERPOLATION_VECTORS[index];
        let density = interpolate_fixed_manual(
            vector.altitude_q12,
            vectors::ALTITUDE_KNOTS_Q12,
            vectors::DENSITY_Q28,
        );
        let gravity = interpolate_fixed_manual(
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
    failures
}
