use crate::optimized::{halve_round_nonnegative, interpolate_fixed_fast};
use crate::{
    divide_scaled_manual, interpolate_fixed_manual, multiply_scaled_manual, vectors,
    vertical_vectors,
};

const FNV_OFFSET: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalState {
    pub time_q12: i32,
    pub altitude_q12: i32,
    pub velocity_q24: i32,
    pub acceleration_q28: i32,
    pub mass_q12: i32,
    pub propellant_q12: i32,
    pub cutoff_events: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalRun {
    pub state: VerticalState,
    pub checksum: u32,
    pub checkpoint_failures: u16,
}

impl VerticalState {
    pub const fn initial() -> Self {
        Self {
            time_q12: 0,
            altitude_q12: 0,
            velocity_q24: 0,
            acceleration_q28: 0,
            mass_q12: vertical_vectors::INITIAL_MASS_Q12,
            propellant_q12: vertical_vectors::INITIAL_PROPELLANT_Q12,
            cutoff_events: 0,
        }
    }

    pub fn engine_active(&self) -> bool {
        self.propellant_q12 > 0 && self.time_q12 < vertical_vectors::BURN_DURATION_Q12
    }
}

fn velocity_magnitude(value: i32) -> i32 {
    if value < 0 {
        -value
    } else {
        value
    }
}

pub fn vertical_step_manual(state: &mut VerticalState) {
    let engine_active = state.engine_active();
    let density = interpolate_fixed_manual(
        state.altitude_q12,
        vectors::ALTITUDE_KNOTS_Q12,
        vectors::DENSITY_Q28,
    );
    let gravity = interpolate_fixed_manual(
        state.altitude_q12,
        vectors::ALTITUDE_KNOTS_Q12,
        vectors::GRAVITY_Q28,
    );

    let speed_squared = multiply_scaled_manual(
        state.velocity_q24,
        velocity_magnitude(state.velocity_q24),
        24,
    )
    .unwrap_or(0);
    let rho_v2 = multiply_scaled_manual(density, speed_squared, 28).unwrap_or(0);
    let drag_with_two = multiply_scaled_manual(rho_v2, vertical_vectors::CDA_Q16, 28).unwrap_or(0);
    let drag = divide_scaled_manual(drag_with_two, 2, 0).unwrap_or(0);
    let weight = multiply_scaled_manual(state.mass_q12, gravity, 28).unwrap_or(0);
    let thrust = if engine_active {
        vertical_vectors::THRUST_Q12
    } else {
        0
    };
    let net_force = thrust - weight - drag;
    state.acceleration_q28 = divide_scaled_manual(net_force, state.mass_q12, 28).unwrap_or(0);

    let delta_velocity =
        multiply_scaled_manual(state.acceleration_q28, vertical_vectors::TIMESTEP_Q12, 16)
            .unwrap_or(0);
    state.velocity_q24 += delta_velocity;

    let delta_altitude =
        multiply_scaled_manual(state.velocity_q24, vertical_vectors::TIMESTEP_Q12, 24).unwrap_or(0);
    state.altitude_q12 += delta_altitude;

    if engine_active {
        let mut consumed = multiply_scaled_manual(
            vertical_vectors::MASS_FLOW_Q12,
            vertical_vectors::TIMESTEP_Q12,
            12,
        )
        .unwrap_or(0);
        if consumed > state.propellant_q12 {
            consumed = state.propellant_q12;
        }
        state.propellant_q12 -= consumed;
        state.mass_q12 -= consumed;
        if state.mass_q12 < vertical_vectors::DRY_MASS_Q12 {
            state.mass_q12 = vertical_vectors::DRY_MASS_Q12;
        }
        if state.propellant_q12 == 0 {
            state.cutoff_events += 1;
        }
    }

    state.time_q12 += vertical_vectors::TIMESTEP_Q12;
}

pub fn vertical_step_optimized(state: &mut VerticalState) {
    let engine_active = state.engine_active();
    let density = interpolate_fixed_fast(
        state.altitude_q12,
        vectors::ALTITUDE_KNOTS_Q12,
        vectors::DENSITY_Q28,
    );
    let gravity = interpolate_fixed_fast(
        state.altitude_q12,
        vectors::ALTITUDE_KNOTS_Q12,
        vectors::GRAVITY_Q28,
    );

    let speed_squared = multiply_scaled_manual(
        state.velocity_q24,
        velocity_magnitude(state.velocity_q24),
        24,
    )
    .unwrap_or(0);
    let rho_v2 = multiply_scaled_manual(density, speed_squared, 28).unwrap_or(0);
    let drag_with_two = multiply_scaled_manual(rho_v2, vertical_vectors::CDA_Q16, 28).unwrap_or(0);
    let drag = halve_round_nonnegative(drag_with_two);
    let weight = multiply_scaled_manual(state.mass_q12, gravity, 28).unwrap_or(0);
    let thrust = if engine_active {
        vertical_vectors::THRUST_Q12
    } else {
        0
    };
    let net_force = thrust - weight - drag;
    state.acceleration_q28 = divide_scaled_manual(net_force, state.mass_q12, 28).unwrap_or(0);

    let delta_velocity =
        multiply_scaled_manual(state.acceleration_q28, vertical_vectors::TIMESTEP_Q12, 16)
            .unwrap_or(0);
    state.velocity_q24 += delta_velocity;

    let delta_altitude =
        multiply_scaled_manual(state.velocity_q24, vertical_vectors::TIMESTEP_Q12, 24).unwrap_or(0);
    state.altitude_q12 += delta_altitude;

    if engine_active {
        let mut consumed = multiply_scaled_manual(
            vertical_vectors::MASS_FLOW_Q12,
            vertical_vectors::TIMESTEP_Q12,
            12,
        )
        .unwrap_or(0);
        if consumed > state.propellant_q12 {
            consumed = state.propellant_q12;
        }
        state.propellant_q12 -= consumed;
        state.mass_q12 -= consumed;
        if state.mass_q12 < vertical_vectors::DRY_MASS_Q12 {
            state.mass_q12 = vertical_vectors::DRY_MASS_Q12;
        }
        if state.propellant_q12 == 0 {
            state.cutoff_events += 1;
        }
    }

    state.time_q12 += vertical_vectors::TIMESTEP_Q12;
}
fn hash_word(mut hash: u32, value: i32) -> u32 {
    let raw = value as u32;
    let mut byte_index = 0u8;
    while byte_index < 4 {
        hash ^= (raw >> (byte_index * 8)) & 0xff;
        hash = hash.wrapping_mul(FNV_PRIME);
        byte_index += 1;
    }
    hash
}

pub fn hash_vertical_state(mut hash: u32, state: &VerticalState) -> u32 {
    hash = hash_word(hash, state.time_q12);
    hash = hash_word(hash, state.altitude_q12);
    hash = hash_word(hash, state.velocity_q24);
    hash = hash_word(hash, state.acceleration_q28);
    hash = hash_word(hash, state.mass_q12);
    hash = hash_word(hash, state.propellant_q12);
    hash = hash_word(hash, if state.engine_active() { 1 } else { 0 });
    hash_word(hash, state.cutoff_events as i32)
}

pub fn vertical_state_matches_checkpoint(
    state: &VerticalState,
    checkpoint: vertical_vectors::VerticalCheckpoint,
) -> bool {
    state.time_q12 == checkpoint.time_q12
        && state.altitude_q12 == checkpoint.altitude_q12
        && state.velocity_q24 == checkpoint.velocity_q24
        && state.acceleration_q28 == checkpoint.acceleration_q28
        && state.mass_q12 == checkpoint.mass_q12
        && state.propellant_q12 == checkpoint.propellant_q12
        && state.engine_active() as u8 == checkpoint.engine_active
        && state.cutoff_events == checkpoint.cutoff_events
}

pub fn run_vertical_kernel_manual() -> VerticalState {
    let mut state = VerticalState::initial();
    let mut step = 0u16;
    while step < vertical_vectors::VERTICAL_TOTAL_STEPS {
        vertical_step_manual(&mut state);
        step += 1;
    }
    state
}
pub fn run_vertical_kernel_optimized() -> VerticalState {
    let mut state = VerticalState::initial();
    let mut step = 0u16;
    while step < vertical_vectors::VERTICAL_TOTAL_STEPS {
        vertical_step_optimized(&mut state);
        step += 1;
    }
    state
}
pub fn run_vertical_manual() -> VerticalRun {
    let mut state = VerticalState::initial();
    let mut checksum = FNV_OFFSET;
    let mut checkpoint_failures = 0u16;
    let mut checkpoint_index = 0usize;

    if !vertical_state_matches_checkpoint(&state, vertical_vectors::VERTICAL_CHECKPOINTS[0]) {
        checkpoint_failures += 1;
    }
    checkpoint_index += 1;

    let mut step = 1u16;
    while step <= vertical_vectors::VERTICAL_TOTAL_STEPS {
        vertical_step_manual(&mut state);
        checksum = hash_vertical_state(checksum, &state);

        if checkpoint_index < vertical_vectors::VERTICAL_CHECKPOINTS.len()
            && step == vertical_vectors::VERTICAL_CHECKPOINTS[checkpoint_index].step
        {
            if !vertical_state_matches_checkpoint(
                &state,
                vertical_vectors::VERTICAL_CHECKPOINTS[checkpoint_index],
            ) {
                checkpoint_failures += 1;
            }
            checkpoint_index += 1;
        }
        step += 1;
    }

    VerticalRun {
        state,
        checksum,
        checkpoint_failures,
    }
}
pub fn run_vertical_optimized() -> VerticalRun {
    let mut state = VerticalState::initial();
    let mut checksum = FNV_OFFSET;
    let mut checkpoint_failures = 0u16;
    let mut checkpoint_index = 0usize;

    if !vertical_state_matches_checkpoint(&state, vertical_vectors::VERTICAL_CHECKPOINTS[0]) {
        checkpoint_failures += 1;
    }
    checkpoint_index += 1;

    let mut step = 1u16;
    while step <= vertical_vectors::VERTICAL_TOTAL_STEPS {
        vertical_step_optimized(&mut state);
        checksum = hash_vertical_state(checksum, &state);

        if checkpoint_index < vertical_vectors::VERTICAL_CHECKPOINTS.len()
            && step == vertical_vectors::VERTICAL_CHECKPOINTS[checkpoint_index].step
        {
            if !vertical_state_matches_checkpoint(
                &state,
                vertical_vectors::VERTICAL_CHECKPOINTS[checkpoint_index],
            ) {
                checkpoint_failures += 1;
            }
            checkpoint_index += 1;
        }
        step += 1;
    }

    VerticalRun {
        state,
        checksum,
        checkpoint_failures,
    }
}
