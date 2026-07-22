#include "arithmetic.hpp"
#include "../../generated/phase0_vectors.hpp"

static_assert(sizeof(long) == 4, "Candidate requires 32-bit long");
#ifdef KSA64_OSCAR64
static_assert(sizeof(unsigned) == 2, "Oscar64 unsigned must be 16 bits");
#endif

struct Unsigned64 {
    unsigned long low;
    unsigned long high;
};

static unsigned long magnitude(long value) {
    if (value == KSA64_I32_MIN) {
        return 0x80000000UL;
    }
    if (value < 0) {
        return (unsigned long)(-value);
    }
    return (unsigned long)value;
}

static void add_words(
    Unsigned64 & value,
    unsigned long add_low,
    unsigned long add_high
) {
    unsigned long previous = value.low;
    value.low += add_low;
    value.high += add_high;
    if (value.low < previous) {
        ++value.high;
    }
}

static Unsigned64 multiply_unsigned_32(
    unsigned long a,
    unsigned long b
) {
    unsigned long a_low = a & 0xffffUL;
    unsigned long a_high = a >> 16;
    unsigned long b_low = b & 0xffffUL;
    unsigned long b_high = b >> 16;

    unsigned long product_00 = a_low * b_low;
    unsigned long product_01 = a_low * b_high;
    unsigned long product_10 = a_high * b_low;
    unsigned long product_11 = a_high * b_high;

    Unsigned64 result;
    result.low = product_00;
    result.high = 0;
    add_words(result, product_01 << 16, product_01 >> 16);
    add_words(result, product_10 << 16, product_10 >> 16);
    result.high += product_11;
    return result;
}

static Unsigned64 shift_left_32(unsigned long value, unsigned char shift) {
    Unsigned64 result;
    if (shift == 0) {
        result.low = value;
        result.high = 0;
    } else {
        result.low = value << shift;
        result.high = value >> (32 - shift);
    }
    return result;
}

static Unsigned64 shift_right(Unsigned64 value, unsigned char shift) {
    if (shift == 0) {
        return value;
    }

    Unsigned64 result;
    result.low = (value.low >> shift) | (value.high << (32 - shift));
    result.high = value.high >> shift;
    return result;
}

static void increment(Unsigned64 & value) {
    ++value.low;
    if (value.low == 0) {
        ++value.high;
    }
}

static unsigned char bit_at(Unsigned64 value, unsigned char position) {
    if (position < 32) {
        return (unsigned char)((value.low >> position) & 1UL);
    }
    return (unsigned char)((value.high >> (position - 32)) & 1UL);
}

static void set_bit(Unsigned64 & value, unsigned char position) {
    if (position < 32) {
        value.low |= 1UL << position;
    } else {
        value.high |= 1UL << (position - 32);
    }
}

static Unsigned64 divide_unsigned_64_by_32(
    Unsigned64 numerator,
    unsigned long denominator,
    unsigned long & remainder
) {
    Unsigned64 quotient;
    quotient.low = 0;
    quotient.high = 0;
    remainder = 0;

    unsigned char position = 64;
    while (position != 0) {
        --position;
        remainder = (remainder << 1) | bit_at(numerator, position);
        if (remainder >= denominator) {
            remainder -= denominator;
            set_bit(quotient, position);
        }
    }
    return quotient;
}

static long signed_saturate(Unsigned64 value, bool negative) {
    if (negative) {
        if (value.high != 0 || value.low >= 0x80000000UL) {
            return KSA64_I32_MIN;
        }
        return -(long)value.low;
    }

    if (value.high != 0 || value.low > 0x7fffffffUL) {
        return KSA64_I32_MAX;
    }
    return (long)value.low;
}

static long saturating_add(long a, long b) {
    if (b > 0 && a > KSA64_I32_MAX - b) {
        return KSA64_I32_MAX;
    }
    if (b < 0 && a < KSA64_I32_MIN - b) {
        return KSA64_I32_MIN;
    }
    return a + b;
}

long multiply_scaled(
    long a,
    long b,
    unsigned char shift,
    ArithmeticStatus & status
) {
    if (shift > 31) {
        status = ARITHMETIC_SHIFT_OUT_OF_RANGE;
        return 0;
    }

    status = ARITHMETIC_OK;
    bool negative = (a < 0) != (b < 0);
    Unsigned64 product = multiply_unsigned_32(magnitude(a), magnitude(b));
    if (shift != 0) {
        add_words(product, 1UL << (shift - 1), 0);
    }
    return signed_saturate(shift_right(product, shift), negative);
}

long divide_scaled(
    long numerator,
    long denominator,
    unsigned char shift,
    ArithmeticStatus & status
) {
    if (denominator == 0) {
        status = ARITHMETIC_DIVISION_BY_ZERO;
        return 0;
    }
    if (shift > 31) {
        status = ARITHMETIC_SHIFT_OUT_OF_RANGE;
        return 0;
    }

    status = ARITHMETIC_OK;
    bool negative = (numerator < 0) != (denominator < 0);
    unsigned long divisor = magnitude(denominator);
    Unsigned64 shifted = shift_left_32(magnitude(numerator), shift);
    unsigned long remainder;
    Unsigned64 quotient = divide_unsigned_64_by_32(shifted, divisor, remainder);
    if (remainder >= divisor - remainder) {
        increment(quotient);
    }
    return signed_saturate(quotient, negative);
}

long interpolate_fixed(
    long x,
    const long * xs,
    const long * ys,
    unsigned count
) {
    if (x <= xs[0]) {
        return ys[0];
    }
    if (x >= xs[count - 1]) {
        return ys[count - 1];
    }

    unsigned index = 0;
    while (index + 1 < count) {
        long x0 = xs[index];
        long x1 = xs[index + 1];
        if (x < x1) {
            ArithmeticStatus status;
            long fraction = divide_scaled(x - x0, x1 - x0, 16, status);
            if (fraction < 0) {
                fraction = 0;
            } else if (fraction > 65535L) {
                fraction = 65535L;
            }
            long delta = multiply_scaled(
                ys[index + 1] - ys[index],
                fraction,
                16,
                status
            );
            return saturating_add(ys[index], delta);
        }
        ++index;
    }
    return ys[count - 1];
}

unsigned run_arithmetic_vectors(void) {
    unsigned failures = 0;
    unsigned index;
    ArithmeticStatus status;

    for (index = 0; index < MULTIPLY_VECTOR_COUNT; ++index) {
        const MultiplyVector & vector = MULTIPLY_VECTORS[index];
        long actual = multiply_scaled(vector.a, vector.b, vector.shift, status);
        if (status != ARITHMETIC_OK || actual != vector.expected) {
            ++failures;
        }
    }

    for (index = 0; index < DIVIDE_VECTOR_COUNT; ++index) {
        const DivideVector & vector = DIVIDE_VECTORS[index];
        long actual = divide_scaled(
            vector.numerator,
            vector.denominator,
            vector.shift,
            status
        );
        if (status != ARITHMETIC_OK || actual != vector.expected) {
            ++failures;
        }
    }

    for (index = 0; index < INTERPOLATION_VECTOR_COUNT; ++index) {
        const InterpolationVector & vector = INTERPOLATION_VECTORS[index];
        long density = interpolate_fixed(
            vector.altitude_q12,
            ALTITUDE_KNOTS_Q12,
            DENSITY_Q28,
            ENVIRONMENT_KNOT_COUNT
        );
        long gravity = interpolate_fixed(
            vector.altitude_q12,
            ALTITUDE_KNOTS_Q12,
            GRAVITY_Q28,
            ENVIRONMENT_KNOT_COUNT
        );
        if (density != vector.density_q28) {
            ++failures;
        }
        if (gravity != vector.gravity_q28) {
            ++failures;
        }
    }

    divide_scaled(1, 0, 0, status);
    if (status != ARITHMETIC_DIVISION_BY_ZERO) {
        ++failures;
    }
    multiply_scaled(1, 1, 32, status);
    if (status != ARITHMETIC_SHIFT_OUT_OF_RANGE) {
        ++failures;
    }
    return failures;
}
