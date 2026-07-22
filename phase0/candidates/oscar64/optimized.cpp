#include "optimized.hpp"
#include "arithmetic.hpp"

static unsigned long divide_unsigned_32_by_16(
    unsigned long numerator,
    unsigned denominator,
    unsigned & remainder
) {
    unsigned long quotient = 0;
    remainder = 0;
    unsigned char position = 32;

    while (position != 0) {
        --position;
        remainder = (unsigned)((remainder << 1)
            | ((numerator >> position) & 1UL));
        if (remainder >= denominator) {
            remainder -= denominator;
            quotient |= 1UL << position;
        }
    }
    return quotient;
}

long divide_fraction_q16(long numerator_q12, long denominator_q12) {
    unsigned long scaled_numerator =
        (unsigned long)numerator_q12 << 4;
    unsigned denominator_units = (unsigned)(denominator_q12 >> 12);
    unsigned remainder;
    unsigned long quotient = divide_unsigned_32_by_16(
        scaled_numerator,
        denominator_units,
        remainder
    );
    if (remainder >= denominator_units - remainder) {
        ++quotient;
    }
    return (long)quotient;
}

long halve_round_nonnegative(long value) {
    return (value >> 1) + (value & 1L);
}

static long saturating_add_fast(long a, long b) {
    if (b > 0 && a > 0x7fffffffL - b) {
        return 0x7fffffffL;
    }
    if (b < 0 && a < (-0x7fffffffL - 1L) - b) {
        return -0x7fffffffL - 1L;
    }
    return a + b;
}

long interpolate_fixed_fast(
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
            long fraction = divide_fraction_q16(x - x0, x1 - x0);
            ArithmeticStatus status;
            long delta = multiply_scaled(
                ys[index + 1] - ys[index],
                fraction,
                16,
                status
            );
            return saturating_add_fast(ys[index], delta);
        }
        ++index;
    }
    return ys[count - 1];
}
