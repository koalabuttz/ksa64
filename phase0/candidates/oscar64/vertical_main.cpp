#include "vertical.hpp"
#include "../../generated/phase0_vertical.hpp"

#ifdef KSA64_OSCAR64

int main(void) {
    VerticalRun run;
    run_vertical_workload(run);
    unsigned failures = run.checkpoint_failures;
    if (run.checksum != VERTICAL_FINAL_FNV1A32) {
        ++failures;
    }

    volatile unsigned * const result = (volatile unsigned *)0xc000;
    volatile unsigned long * const checksum =
        (volatile unsigned long *)0xc004;
    volatile unsigned char * const border = (volatile unsigned char *)0xd020;
    *result = failures;
    *checksum = run.checksum;
    *border = failures == 0 ? 5 : 2;
    return failures;
}

#else

#include <stdio.h>

int main(void) {
    VerticalRun run;
    run_vertical_workload(run);
    unsigned failures = run.checkpoint_failures;
    if (run.checksum != VERTICAL_FINAL_FNV1A32) {
        ++failures;
    }
    if (failures == 0) {
        printf(
            "Phase 0 vertical workload: PASS (checksum %08lx)\n",
            run.checksum
        );
        return 0;
    }
    printf(
        "Phase 0 vertical workload: %u failure(s), checksum %08lx\n",
        failures,
        run.checksum
    );
    return 1;
}

#endif
