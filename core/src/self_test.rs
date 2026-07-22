use crate::numeric::{
    add, divide_scaled, interpolate_clamped, multiply_scaled, subtract, NumericFault, NumericStatus,
};

mod vectors {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase1/generated/numeric_v1.rs"
    ));
}

#[inline]
fn failure_count(condition: bool) -> u16 {
    if condition {
        0
    } else {
        1
    }
}

fn check_arithmetic() -> u16 {
    let mut failures = 0u16;
    let mut index = 0usize;
    while index < vectors::MULTIPLY_VECTORS.len() {
        let vector = vectors::MULTIPLY_VECTORS[index];
        let mut status = NumericStatus::CLEAR;
        let actual = multiply_scaled(vector.a, vector.b, vector.shift, &mut status);
        failures += failure_count(actual == vector.expected);
        failures += failure_count(status.bits() == vector.expected_faults);
        index += 1;
    }

    index = 0;
    while index < vectors::DIVIDE_VECTORS.len() {
        let vector = vectors::DIVIDE_VECTORS[index];
        let mut status = NumericStatus::CLEAR;
        let actual = divide_scaled(
            vector.numerator,
            vector.denominator,
            vector.shift,
            &mut status,
        );
        failures += failure_count(actual == vector.expected);
        failures += failure_count(status.bits() == vector.expected_faults);
        index += 1;
    }

    let mut status = NumericStatus::CLEAR;
    failures += failure_count(divide_scaled(1, 0, 0, &mut status) == 0);
    failures += failure_count(status.contains(NumericFault::DivisionByZero));
    failures += failure_count(multiply_scaled(1, 1, 32, &mut status) == 0);
    failures += failure_count(status.contains(NumericFault::InvalidShift));
    failures
}

fn check_interpolation() -> u16 {
    let mut failures = 0u16;
    let mut index = 0usize;
    while index < vectors::INTERPOLATION_VECTORS.len() {
        let vector = vectors::INTERPOLATION_VECTORS[index];
        let mut status = NumericStatus::CLEAR;
        let density = interpolate_clamped(
            vector.altitude_q12,
            vectors::ALTITUDE_KNOTS_Q12,
            vectors::DENSITY_Q28,
            &mut status,
        );
        let gravity = interpolate_clamped(
            vector.altitude_q12,
            vectors::ALTITUDE_KNOTS_Q12,
            vectors::GRAVITY_Q28,
            &mut status,
        );
        failures += failure_count(density == vector.density_q28);
        failures += failure_count(gravity == vector.gravity_q28);
        failures += failure_count(status.is_clear());
        index += 1;
    }
    failures
}

fn check_constant_velocity() -> u16 {
    let mut failures = 0u16;
    let first = vectors::CONSTANT_VELOCITY_CHECKPOINTS[0];
    let mut altitude = first.altitude_q12;
    let velocity = first.velocity_q24;
    let mut checkpoint_index = 0usize;
    let mut step = 0u32;
    let mut status = NumericStatus::CLEAR;
    while step <= vectors::CONSTANT_VELOCITY_STEPS {
        if checkpoint_index < vectors::CONSTANT_VELOCITY_CHECKPOINTS.len() {
            let expected = vectors::CONSTANT_VELOCITY_CHECKPOINTS[checkpoint_index];
            if step == expected.step {
                failures += failure_count(altitude == expected.altitude_q12);
                failures += failure_count(velocity == expected.velocity_q24);
                checkpoint_index += 1;
            }
        }
        if step != vectors::CONSTANT_VELOCITY_STEPS {
            let delta = multiply_scaled(
                velocity,
                vectors::CONSTANT_VELOCITY_TIMESTEP_Q16,
                28,
                &mut status,
            );
            altitude = add(altitude, delta, &mut status);
        }
        step += 1;
    }
    failures += failure_count(status.is_clear());
    failures += failure_count(checkpoint_index == vectors::CONSTANT_VELOCITY_CHECKPOINTS.len());
    failures
}

fn check_acceleration_cases() -> u16 {
    let mut failures = 0u16;
    let mut case_index = 0usize;
    while case_index < vectors::ACCELERATION_CASES.len() {
        let case = vectors::ACCELERATION_CASES[case_index];
        let mut altitude = 0i32;
        let mut velocity = 0i32;
        let mut checkpoint_index = 0usize;
        let mut step = 0u32;
        let mut status = NumericStatus::CLEAR;
        while step <= case.steps {
            if checkpoint_index < case.checkpoints.len() {
                let expected = case.checkpoints[checkpoint_index];
                if step == expected.step {
                    failures += failure_count(altitude == expected.altitude_q12);
                    failures += failure_count(velocity == expected.velocity_q24);
                    checkpoint_index += 1;
                }
            }
            if step != case.steps {
                let delta_velocity =
                    multiply_scaled(case.acceleration_q28, case.timestep_q16, 20, &mut status);
                velocity = add(velocity, delta_velocity, &mut status);
                let delta_altitude = multiply_scaled(velocity, case.timestep_q16, 28, &mut status);
                altitude = add(altitude, delta_altitude, &mut status);
            }
            step += 1;
        }
        failures += failure_count(status.is_clear());
        failures += failure_count(checkpoint_index == case.checkpoints.len());
        case_index += 1;
    }
    failures
}

fn check_mass_flow() -> u16 {
    let mut failures = 0u16;
    let first = vectors::MASS_FLOW_CHECKPOINTS[0];
    let mut mass = first.mass_q12;
    let mut propellant = first.propellant_q12;
    let mut checkpoint_index = 0usize;
    let mut step = 0u32;
    let mut status = NumericStatus::CLEAR;
    while step <= vectors::MASS_FLOW_STEPS {
        if checkpoint_index < vectors::MASS_FLOW_CHECKPOINTS.len() {
            let expected = vectors::MASS_FLOW_CHECKPOINTS[checkpoint_index];
            if step == expected.step {
                failures += failure_count(mass == expected.mass_q12);
                failures += failure_count(propellant == expected.propellant_q12);
                checkpoint_index += 1;
            }
        }
        if step != vectors::MASS_FLOW_STEPS {
            let requested = multiply_scaled(
                vectors::MASS_FLOW_Q16,
                vectors::MASS_FLOW_TIMESTEP_Q16,
                20,
                &mut status,
            );
            let consumed = requested.min(propellant);
            propellant = subtract(propellant, consumed, &mut status);
            mass = subtract(mass, consumed, &mut status).max(vectors::DRY_MASS_Q12);
        }
        step += 1;
    }
    failures += failure_count(status.is_clear());
    failures += failure_count(checkpoint_index == vectors::MASS_FLOW_CHECKPOINTS.len());
    failures
}

pub fn run_numeric_self_tests() -> u16 {
    let mut failures = 0u16;
    failures += failure_count(vectors::NUMERIC_CONTRACT == "ksa64.numeric.phase1-v1");
    failures += check_arithmetic();
    failures += check_interpolation();
    failures += check_constant_velocity();
    failures += check_acceleration_cases();
    failures += check_mass_flow();
    failures
}
