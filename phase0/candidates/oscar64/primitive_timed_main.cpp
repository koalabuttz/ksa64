#include "arithmetic.hpp"
#include "c64_timer.hpp"
#include "optimized.hpp"

static const unsigned long RESULT_MAGIC = 0x5250534bUL;
static const unsigned RESULT_SCHEMA = 1;
static const unsigned CANDIDATE_OSCAR64 = 2;
static const unsigned ITERATIONS = 512;

static const unsigned long MULTIPLY_EXPECTED = 20084UL * ITERATIONS;
static const unsigned long DIVIDE_EXPECTED = 1449525UL * ITERATIONS;
static const unsigned long FRACTION_EXPECTED = 32768UL * ITERATIONS;

int main(void) {
    volatile long * const multiply_a = (volatile long *)0xc080;
    volatile long * const multiply_b = (volatile long *)0xc084;
    volatile long * const divide_numerator = (volatile long *)0xc08c;
    volatile long * const divide_denominator = (volatile long *)0xc090;
    volatile long * const fraction_numerator = (volatile long *)0xc098;
    volatile long * const fraction_denominator = (volatile long *)0xc09c;
    volatile unsigned long * const magic = (volatile unsigned long *)0xc100;

    *magic = 0;
    *multiply_a = 2048000L;
    *multiply_b = 2632453L;
    *divide_numerator = 11059L;
    *divide_denominator = 2048000L;
    *fraction_numerator = 4096L;
    *fraction_denominator = 8192L;
    prepare_cia_timing();

    unsigned long overhead = measure_cia_boundary_overhead();
    ArithmeticStatus arithmetic_status;

    unsigned long multiply_accumulator = 0;
    unsigned iteration = 0;
    start_cia_timer();
    while (iteration < ITERATIONS) {
        long value = multiply_scaled(
            *multiply_a,
            *multiply_b,
            28,
            arithmetic_status
        );
        multiply_accumulator += (unsigned long)value;
        ++iteration;
    }
    unsigned long multiply_elapsed = stop_cia_timer();
    bool arithmetic_failed = arithmetic_status != ARITHMETIC_OK;

    unsigned long divide_accumulator = 0;
    iteration = 0;
    start_cia_timer();
    while (iteration < ITERATIONS) {
        long value = divide_scaled(
            *divide_numerator,
            *divide_denominator,
            28,
            arithmetic_status
        );
        divide_accumulator += (unsigned long)value;
        ++iteration;
    }
    unsigned long divide_elapsed = stop_cia_timer();
    arithmetic_failed = arithmetic_failed || arithmetic_status != ARITHMETIC_OK;

    unsigned long fraction_accumulator = 0;
    iteration = 0;
    start_cia_timer();
    while (iteration < ITERATIONS) {
        long value = divide_fraction_q16(
            *fraction_numerator,
            *fraction_denominator
        );
        fraction_accumulator += (unsigned long)value;
        ++iteration;
    }
    unsigned long fraction_elapsed = stop_cia_timer();

    unsigned status = 0;
    if (multiply_accumulator != MULTIPLY_EXPECTED) {
        status |= 1;
    }
    if (divide_accumulator != DIVIDE_EXPECTED) {
        status |= 2;
    }
    if (fraction_accumulator != FRACTION_EXPECTED) {
        status |= 4;
    }
    if (arithmetic_failed) {
        status |= 8;
    }

    *(volatile unsigned *)0xc104 = RESULT_SCHEMA;
    *(volatile unsigned *)0xc106 = CANDIDATE_OSCAR64;
    *(volatile unsigned *)0xc108 = status;
    *(volatile unsigned *)0xc10a = ITERATIONS;
    *(volatile unsigned long *)0xc10c = overhead;
    *(volatile unsigned long *)0xc110 = multiply_elapsed;
    *(volatile unsigned long *)0xc114 = divide_elapsed;
    *(volatile unsigned long *)0xc118 = fraction_elapsed;
    *(volatile unsigned long *)0xc11c = multiply_accumulator;
    *(volatile unsigned long *)0xc120 = divide_accumulator;
    *(volatile unsigned long *)0xc124 = fraction_accumulator;
    *(volatile unsigned char *)0xd020 = status == 0 ? 5 : 2;
    *magic = RESULT_MAGIC;

    while (true) {
    }
}
