#ifndef KSA64_PHASE0_VERTICAL_CANDIDATE_HPP
#define KSA64_PHASE0_VERTICAL_CANDIDATE_HPP

#include "../../generated/phase0_vertical.hpp"

struct VerticalState {
    long time_q12;
    long altitude_q12;
    long velocity_q24;
    long acceleration_q28;
    long mass_q12;
    long propellant_q12;
    unsigned char cutoff_events;
};

struct VerticalRun {
    VerticalState state;
    unsigned long checksum;
    unsigned checkpoint_failures;
};

bool vertical_engine_active(const VerticalState & state);
bool vertical_state_matches_checkpoint(
    const VerticalState & state,
    const VerticalCheckpoint & checkpoint
);
void vertical_step(VerticalState & state);
void vertical_step_optimized(VerticalState & state);
void run_vertical_kernel(VerticalState & state);
void run_vertical_kernel_optimized(VerticalState & state);
unsigned long hash_vertical_state(
    unsigned long hash,
    const VerticalState & state
);
void run_vertical_workload(VerticalRun & run);
void run_vertical_workload_optimized(VerticalRun & run);

#endif
