#include "vertical.hpp"
#include "../../generated/phase0_vertical.hpp"

int main(void) {
    VerticalState state;
    run_vertical_kernel(state);
    const VerticalCheckpoint & final_checkpoint =
        VERTICAL_CHECKPOINTS[VERTICAL_CHECKPOINT_COUNT - 1];
    unsigned failures = vertical_state_matches_checkpoint(
        state,
        final_checkpoint
    ) ? 0 : 1;

#ifdef KSA64_OSCAR64
    volatile unsigned * const result = (volatile unsigned *)0xc000;
    volatile unsigned char * const border = (volatile unsigned char *)0xd020;
    *result = failures;
    *border = failures == 0 ? 5 : 2;
#endif
    return failures;
}
