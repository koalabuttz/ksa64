use crate::multiply_scaled_manual;

fn divide_unsigned_32_by_16(numerator: u32, denominator: u16) -> (u32, u16) {
    let divisor = denominator as u32;
    let mut quotient = 0u32;
    let mut remainder = 0u32;
    let mut position = 32u8;

    while position != 0 {
        position -= 1;
        remainder = (remainder << 1) | ((numerator >> position) & 1);
        if remainder >= divisor {
            remainder -= divisor;
            quotient |= 1u32 << position;
        }
    }
    (quotient, remainder as u16)
}

pub(crate) fn divide_fraction_q16(numerator_q12: i32, denominator_q12: i32) -> i32 {
    debug_assert!(numerator_q12 >= 0);
    debug_assert!(denominator_q12 > 0);
    debug_assert!(numerator_q12 < denominator_q12);
    debug_assert_eq!(denominator_q12 & 0x0fff, 0);
    debug_assert!((denominator_q12 >> 12) <= u16::MAX as i32);
    debug_assert!(numerator_q12 <= (u32::MAX >> 4) as i32);

    let scaled_numerator = (numerator_q12 as u32) << 4;
    let denominator_units = (denominator_q12 >> 12) as u16;
    let (mut quotient, remainder) = divide_unsigned_32_by_16(scaled_numerator, denominator_units);
    if remainder as u32 >= denominator_units as u32 - remainder as u32 {
        quotient += 1;
    }
    quotient as i32
}

pub(crate) fn halve_round_nonnegative(value: i32) -> i32 {
    debug_assert!(value >= 0);
    (value >> 1) + (value & 1)
}

pub(crate) fn interpolate_fixed_fast(x: i32, xs: &[i32], ys: &[i32]) -> i32 {
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
            let fraction = divide_fraction_q16(x - x0, x1 - x0);
            let delta =
                multiply_scaled_manual(ys[index + 1] - ys[index], fraction, 16).unwrap_or(0);
            return saturating_add(ys[index], delta);
        }
        index += 1;
    }
    ys[last]
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
