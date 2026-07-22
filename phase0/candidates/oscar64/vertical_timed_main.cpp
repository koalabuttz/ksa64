#include "c64_timer.hpp"
#include "vertical.hpp"
#include "../../generated/phase0_vertical.hpp"

static const unsigned long TIMING_MAGIC = 0x5441534bUL;
static const unsigned TIMING_SCHEMA = 1;
static const unsigned CANDIDATE_OSCAR64 = 2;

int main(void) {
    VerticalState state;
    state.time_q12 = 0;
    state.altitude_q12 = 0;
    state.velocity_q24 = 0;
    state.acceleration_q28 = 0;
    state.mass_q12 = INITIAL_MASS_Q12;
    state.propellant_q12 = INITIAL_PROPELLANT_Q12;
    state.cutoff_events = 0;

    volatile unsigned long * const magic =
        (volatile unsigned long *)0xc000;
    *magic = 0;
    prepare_cia_timing();
    unsigned long overhead = measure_cia_boundary_overhead();

    start_cia_timer();
    unsigned step = 0;
    while (step < VERTICAL_TOTAL_STEPS) {
        vertical_step_optimized(state);
        ++step;
    }
    unsigned long elapsed = stop_cia_timer();

    const VerticalCheckpoint & expected =
        VERTICAL_CHECKPOINTS[VERTICAL_CHECKPOINT_COUNT - 1];
    unsigned status = vertical_state_matches_checkpoint(
        state,
        expected
    ) ? 0 : 1;
    unsigned long net = elapsed - overhead;

    *(volatile unsigned *)0xc004 = TIMING_SCHEMA;
    *(volatile unsigned *)0xc006 = CANDIDATE_OSCAR64;
    *(volatile unsigned *)0xc008 = status;
    *(volatile unsigned long *)0xc00c = elapsed;
    *(volatile unsigned long *)0xc010 = overhead;
    *(volatile unsigned long *)0xc014 = net;
    *(volatile long *)0xc018 = state.altitude_q12;
    *(volatile long *)0xc01c = state.velocity_q24;
    *(volatile long *)0xc020 = state.acceleration_q28;
    *(volatile long *)0xc024 = state.mass_q12;
    *(volatile long *)0xc028 = state.propellant_q12;
    *(volatile unsigned char *)0xc02c = state.cutoff_events;
    *(volatile unsigned char *)0xd020 = status == 0 ? 5 : 2;
    *magic = TIMING_MAGIC;

    while (true) {
    }
}
