#ifndef KSA64_PHASE0_ARITHMETIC_HPP
#define KSA64_PHASE0_ARITHMETIC_HPP

enum ArithmeticStatus {
    ARITHMETIC_OK = 0,
    ARITHMETIC_DIVISION_BY_ZERO = 1,
    ARITHMETIC_SHIFT_OUT_OF_RANGE = 2
};

long multiply_scaled(long a, long b, unsigned char shift, ArithmeticStatus & status);

long divide_scaled(
    long numerator,
    long denominator,
    unsigned char shift,
    ArithmeticStatus & status
);

long interpolate_fixed(
    long x,
    const long * xs,
    const long * ys,
    unsigned count
);

unsigned run_arithmetic_vectors(void);

#endif
