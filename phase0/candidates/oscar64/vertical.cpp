#include "vertical.hpp"
#include "arithmetic.hpp"
#include "../../generated/phase0_vectors.hpp"
#include "../../generated/phase0_vertical.hpp"

static const unsigned long FNV_OFFSET = 2166136261UL;
static const unsigned long FNV_PRIME = 16777619UL;

bool vertical_engine_active(const VerticalState & state) {
    return state.propellant_q12 > 0 && state.time_q12 < BURN_DURATION_Q12;
}

static long velocity_magnitude(long value) {
    return value < 0 ? -value : value;
}

void vertical_step(VerticalState & state) {
    bool engine_active = vertical_engine_active(state);
    long density = interpolate_fixed(
        state.altitude_q12,
        ALTITUDE_KNOTS_Q12,
        DENSITY_Q28,
        ENVIRONMENT_KNOT_COUNT
    );
    long gravity = interpolate_fixed(
        state.altitude_q12,
        ALTITUDE_KNOTS_Q12,
        GRAVITY_Q28,
        ENVIRONMENT_KNOT_COUNT
    );

    ArithmeticStatus status;
    long speed_squared = multiply_scaled(
        state.velocity_q24,
        velocity_magnitude(state.velocity_q24),
        24,
        status
    );
    long rho_v2 = multiply_scaled(density, speed_squared, 28, status);
    long drag_with_two = multiply_scaled(rho_v2, CDA_Q16, 28, status);
    long drag = divide_scaled(drag_with_two, 2, 0, status);
    long weight = multiply_scaled(state.mass_q12, gravity, 28, status);
    long thrust = engine_active ? THRUST_Q12 : 0;
    long net_force = thrust - weight - drag;
    state.acceleration_q28 = divide_scaled(
        net_force,
        state.mass_q12,
        28,
        status
    );

    long delta_velocity = multiply_scaled(
        state.acceleration_q28,
        TIMESTEP_Q12,
        16,
        status
    );
    state.velocity_q24 += delta_velocity;

    long delta_altitude = multiply_scaled(
        state.velocity_q24,
        TIMESTEP_Q12,
        24,
        status
    );
    state.altitude_q12 += delta_altitude;

    if (engine_active) {
        long consumed = multiply_scaled(
            MASS_FLOW_Q12,
            TIMESTEP_Q12,
            12,
            status
        );
        if (consumed > state.propellant_q12) {
            consumed = state.propellant_q12;
        }
        state.propellant_q12 -= consumed;
        state.mass_q12 -= consumed;
        if (state.mass_q12 < DRY_MASS_Q12) {
            state.mass_q12 = DRY_MASS_Q12;
        }
        if (state.propellant_q12 == 0) {
            ++state.cutoff_events;
        }
    }

    state.time_q12 += TIMESTEP_Q12;
}

static unsigned long hash_word(unsigned long hash, long value) {
    unsigned long raw = (unsigned long)value;
    unsigned char byte_index = 0;
    while (byte_index < 4) {
        hash ^= (raw >> (byte_index * 8)) & 0xffUL;
        hash *= FNV_PRIME;
        ++byte_index;
    }
    return hash;
}

unsigned long hash_vertical_state(
    unsigned long hash,
    const VerticalState & state
) {
    hash = hash_word(hash, state.time_q12);
    hash = hash_word(hash, state.altitude_q12);
    hash = hash_word(hash, state.velocity_q24);
    hash = hash_word(hash, state.acceleration_q28);
    hash = hash_word(hash, state.mass_q12);
    hash = hash_word(hash, state.propellant_q12);
    hash = hash_word(hash, vertical_engine_active(state) ? 1 : 0);
    return hash_word(hash, (long)state.cutoff_events);
}

static bool checkpoint_matches(
    const VerticalState & state,
    const VerticalCheckpoint & checkpoint
) {
    return state.time_q12 == checkpoint.time_q12
        && state.altitude_q12 == checkpoint.altitude_q12
        && state.velocity_q24 == checkpoint.velocity_q24
        && state.acceleration_q28 == checkpoint.acceleration_q28
        && state.mass_q12 == checkpoint.mass_q12
        && state.propellant_q12 == checkpoint.propellant_q12
        && (vertical_engine_active(state) ? 1 : 0) == checkpoint.engine_active
        && state.cutoff_events == checkpoint.cutoff_events;
}

void run_vertical_workload(VerticalRun & run) {
    run.state.time_q12 = 0;
    run.state.altitude_q12 = 0;
    run.state.velocity_q24 = 0;
    run.state.acceleration_q28 = 0;
    run.state.mass_q12 = INITIAL_MASS_Q12;
    run.state.propellant_q12 = INITIAL_PROPELLANT_Q12;
    run.state.cutoff_events = 0;
    run.checksum = FNV_OFFSET;
    run.checkpoint_failures = 0;

    unsigned checkpoint_index = 0;
    if (!checkpoint_matches(run.state, VERTICAL_CHECKPOINTS[0])) {
        ++run.checkpoint_failures;
    }
    ++checkpoint_index;

    unsigned step = 1;
    while (step <= VERTICAL_TOTAL_STEPS) {
        vertical_step(run.state);
        run.checksum = hash_vertical_state(run.checksum, run.state);

        if (
            checkpoint_index < VERTICAL_CHECKPOINT_COUNT
            && step == VERTICAL_CHECKPOINTS[checkpoint_index].step
        ) {
            if (!checkpoint_matches(
                run.state,
                VERTICAL_CHECKPOINTS[checkpoint_index]
            )) {
                ++run.checkpoint_failures;
            }
            ++checkpoint_index;
        }
        ++step;
    }
}
