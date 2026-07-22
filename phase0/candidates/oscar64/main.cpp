#include "arithmetic.hpp"

#ifdef KSA64_OSCAR64

int main(void) {
    unsigned failures = run_arithmetic_vectors();
    volatile unsigned * const result = (volatile unsigned *)0xc000;
    volatile unsigned char * const border = (volatile unsigned char *)0xd020;
    *result = failures;
    *border = failures == 0 ? 5 : 2;
    return failures;
}

#else

#include <stdio.h>

int main(void) {
    unsigned failures = run_arithmetic_vectors();
    if (failures == 0) {
        puts("Phase 0 Oscar64-compatible arithmetic vectors: PASS");
        return 0;
    }
    printf("Phase 0 Oscar64-compatible arithmetic vectors: %u failure(s)\n", failures);
    return 1;
}

#endif
