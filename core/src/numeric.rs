//! Deterministic fixed-point primitives for host and MOS targets.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NumericFault {
    Saturation = 0x01,
    DivisionByZero = 0x02,
    InvalidShift = 0x04,
    InvalidInput = 0x08,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct NumericStatus(u8);

impl NumericStatus {
    pub const CLEAR: Self = Self(0);

    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn is_clear(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn contains(self, fault: NumericFault) -> bool {
        self.0 & fault as u8 != 0
    }

    #[inline]
    pub fn record(&mut self, fault: NumericFault) {
        self.0 |= fault as u8;
    }
}

#[derive(Clone, Copy)]
struct Unsigned64 {
    low: u32,
    high: u32,
}

#[inline]
fn magnitude(value: i32) -> u32 {
    if value == i32::MIN {
        0x8000_0000
    } else if value < 0 {
        (-value) as u32
    } else {
        value as u32
    }
}

#[inline]
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

#[inline]
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

#[inline]
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

#[inline]
fn increment(value: &mut Unsigned64) {
    value.low = value.low.wrapping_add(1);
    if value.low == 0 {
        value.high = value.high.wrapping_add(1);
    }
}

#[inline]
fn bit_at(value: Unsigned64, position: u8) -> u8 {
    if position < 32 {
        ((value.low >> position) & 1) as u8
    } else {
        ((value.high >> (position - 32)) & 1) as u8
    }
}

#[inline]
fn set_bit(value: &mut Unsigned64, position: u8) {
    if position < 32 {
        value.low |= 1u32 << position;
    } else {
        value.high |= 1u32 << (position - 32);
    }
}

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

fn divide_unsigned_fraction_q16_integral_q12(
    numerator_q12: i32,
    denominator_q12: i32,
    status: &mut NumericStatus,
) -> i32 {
    if numerator_q12 < 0
        || denominator_q12 <= 0
        || numerator_q12 >= denominator_q12
        || denominator_q12 & 0x0fff != 0
        || (denominator_q12 >> 12) > u16::MAX as i32
        || numerator_q12 > (u32::MAX >> 4) as i32
    {
        status.record(NumericFault::InvalidInput);
        return 0;
    }

    let scaled_numerator = (numerator_q12 as u32) << 4;
    let denominator_units = (denominator_q12 >> 12) as u16;
    let (mut quotient, remainder) = divide_unsigned_32_by_16(scaled_numerator, denominator_units);
    if remainder as u32 >= denominator_units as u32 - remainder as u32 {
        quotient += 1;
    }
    quotient as i32
}

fn divide_unsigned_64_by_16_bits(
    numerator: Unsigned64,
    denominator: u16,
    bit_count: u8,
) -> (Unsigned64, u16) {
    let divisor = denominator as u32;
    let mut quotient = Unsigned64 { low: 0, high: 0 };
    let mut remainder = 0u32;
    let mut position = bit_count;

    while position != 0 {
        position -= 1;
        remainder = (remainder << 1) | bit_at(numerator, position) as u32;
        if remainder >= divisor {
            remainder -= divisor;
            set_bit(&mut quotient, position);
        }
    }
    (quotient, remainder as u16)
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

fn signed_from_magnitude(value: Unsigned64, negative: bool, status: &mut NumericStatus) -> i32 {
    if negative {
        if value.high != 0 || value.low > 0x8000_0000 {
            status.record(NumericFault::Saturation);
            i32::MIN
        } else if value.low == 0x8000_0000 {
            i32::MIN
        } else {
            -(value.low as i32)
        }
    } else if value.high != 0 || value.low > 0x7fff_ffff {
        status.record(NumericFault::Saturation);
        i32::MAX
    } else {
        value.low as i32
    }
}

/// Computes `(a * b) / 2^shift`, rounded to nearest with halves away from zero.
pub fn multiply_scaled(a: i32, b: i32, shift: u8, status: &mut NumericStatus) -> i32 {
    if shift > 31 {
        status.record(NumericFault::InvalidShift);
        return 0;
    }

    let negative = (a < 0) ^ (b < 0);
    let mut product = multiply_unsigned_32(magnitude(a), magnitude(b));
    if shift != 0 {
        add_words(&mut product, 1u32 << (shift - 1), 0);
    }
    signed_from_magnitude(shift_right(product, shift), negative, status)
}

/// Computes `(numerator * 2^shift) / denominator` with contract rounding.
pub fn divide_scaled(
    numerator: i32,
    denominator: i32,
    shift: u8,
    status: &mut NumericStatus,
) -> i32 {
    if denominator == 0 {
        status.record(NumericFault::DivisionByZero);
        return 0;
    }
    if shift > 31 {
        status.record(NumericFault::InvalidShift);
        return 0;
    }

    let negative = (numerator < 0) ^ (denominator < 0);
    let divisor = magnitude(denominator);
    let shifted = shift_left_32(magnitude(numerator), shift);
    let (mut quotient, remainder) = divide_unsigned_64_by_32(shifted, divisor);
    if remainder >= divisor - remainder {
        increment(&mut quotient);
    }
    signed_from_magnitude(quotient, negative, status)
}

/// Attempts an exact reduced-denominator division before using the general path.
///
/// The fast path is selected only when the denominator has the declared power-of-two
/// factor, its reduced magnitude fits `u16`, and the numerator fits the declared bit
/// envelope. All other inputs retain [`divide_scaled`] behavior.
#[inline]
pub fn divide_scaled_reduced_u16<
    const SHIFT: u8,
    const DENOMINATOR_REDUCTION: u8,
    const NUMERATOR_BITS: u8,
>(
    numerator: i32,
    denominator: i32,
    status: &mut NumericStatus,
) -> i32 {
    if SHIFT > 31
        || DENOMINATOR_REDUCTION > SHIFT
        || NUMERATOR_BITS == 0
        || NUMERATOR_BITS > 32
        || NUMERATOR_BITS + SHIFT - DENOMINATOR_REDUCTION > 64
    {
        return divide_scaled(numerator, denominator, SHIFT, status);
    }

    let divisor = magnitude(denominator);
    let reduction_mask = if DENOMINATOR_REDUCTION == 0 {
        0
    } else {
        (1u32 << DENOMINATOR_REDUCTION) - 1
    };
    let reduced_divisor = divisor >> DENOMINATOR_REDUCTION;
    let numerator_magnitude = magnitude(numerator);
    let numerator_fits = NUMERATOR_BITS == 32 || numerator_magnitude < (1u32 << NUMERATOR_BITS);
    if denominator == 0
        || divisor & reduction_mask != 0
        || reduced_divisor == 0
        || reduced_divisor > u16::MAX as u32
        || !numerator_fits
    {
        return divide_scaled(numerator, denominator, SHIFT, status);
    }

    let reduced_shift = SHIFT - DENOMINATOR_REDUCTION;
    let shifted = shift_left_32(numerator_magnitude, reduced_shift);
    let bit_count = NUMERATOR_BITS + reduced_shift;
    let (mut quotient, remainder) =
        divide_unsigned_64_by_16_bits(shifted, reduced_divisor as u16, bit_count);
    if remainder as u32 >= reduced_divisor - remainder as u32 {
        increment(&mut quotient);
    }
    signed_from_magnitude(quotient, (numerator < 0) ^ (denominator < 0), status)
}

pub fn add(a: i32, b: i32, status: &mut NumericStatus) -> i32 {
    if b > 0 && a > i32::MAX - b {
        status.record(NumericFault::Saturation);
        i32::MAX
    } else if b < 0 && a < i32::MIN - b {
        status.record(NumericFault::Saturation);
        i32::MIN
    } else {
        a + b
    }
}

pub fn subtract(a: i32, b: i32, status: &mut NumericStatus) -> i32 {
    if b > 0 && a < i32::MIN + b {
        status.record(NumericFault::Saturation);
        i32::MIN
    } else if b < 0 && a > i32::MAX + b {
        status.record(NumericFault::Saturation);
        i32::MAX
    } else {
        a - b
    }
}

/// Clamped piecewise-linear interpolation using an unsigned Q0.16 fraction.
pub fn interpolate_clamped(x: i32, xs: &[i32], ys: &[i32], status: &mut NumericStatus) -> i32 {
    if xs.is_empty() || xs.len() != ys.len() {
        status.record(NumericFault::InvalidInput);
        return 0;
    }
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
        if x1 <= x0 {
            status.record(NumericFault::InvalidInput);
            return 0;
        }
        if x < x1 {
            let numerator = subtract(x, x0, status);
            let denominator = subtract(x1, x0, status);
            let mut fraction = divide_scaled(numerator, denominator, 16, status);
            fraction = fraction.clamp(0, 65_535);
            let range = subtract(ys[index + 1], ys[index], status);
            let delta = multiply_scaled(range, fraction, 16, status);
            return add(ys[index], delta, status);
        }
        index += 1;
    }

    status.record(NumericFault::InvalidInput);
    0
}

/// Clamped interpolation specialized for integral Q20.12 knot spans.
///
/// This preserves the general Q0.16 rounding contract while reducing the
/// fraction division from 64-by-32 to 32-by-16 arithmetic.
pub fn interpolate_clamped_integral_q12(
    x: i32,
    xs: &[i32],
    ys: &[i32],
    status: &mut NumericStatus,
) -> i32 {
    if xs.is_empty() || xs.len() != ys.len() {
        status.record(NumericFault::InvalidInput);
        return 0;
    }
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
        if x1 <= x0 {
            status.record(NumericFault::InvalidInput);
            return 0;
        }
        if x < x1 {
            let numerator = subtract(x, x0, status);
            let denominator = subtract(x1, x0, status);
            let fraction =
                divide_unsigned_fraction_q16_integral_q12(numerator, denominator, status);
            let range = subtract(ys[index + 1], ys[index], status);
            let delta = multiply_scaled(range, fraction, 16, status);
            return add(ys[index], delta, status);
        }
        index += 1;
    }

    status.record(NumericFault::InvalidInput);
    0
}
