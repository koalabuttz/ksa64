#ifndef KSA64_PHASE0_OPTIMIZED_HPP
#define KSA64_PHASE0_OPTIMIZED_HPP

long divide_fraction_q16(long numerator_q12, long denominator_q12);
long halve_round_nonnegative(long value);
long interpolate_fixed_fast(
    long x,
    const long * xs,
    const long * ys,
    unsigned count
);

#endif
