use crate::dynamics::{advance_vertical_state, evaluate_vertical_forces, VerticalStepError};
use crate::environment::SimpleEarthEnvironment;
use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus};
use crate::quantities::{Altitude, Mass, Time, Velocity};
use crate::scenario::{parse_scenario_image, SCENARIO_IMAGE_LENGTH, SIMPLE_EARTH_ENVIRONMENT_ID};
use crate::vehicle::VerticalTruthState;

const SCENARIO_IMAGE: &[u8; SCENARIO_IMAGE_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase0/numeric/scenario-v1.bin"
));

mod vectors {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase1/generated/numeric_v1.rs"
    ));
}

mod force_vectors {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase1/generated/force_v1.rs"
    ));
}

mod transition_vectors {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase1/generated/transition_v1.rs"
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
    let environment = SimpleEarthEnvironment::new();
    let mut failures = failure_count(environment.tables_are_valid());
    let mut index = 0usize;
    while index < vectors::INTERPOLATION_VECTORS.len() {
        let vector = vectors::INTERPOLATION_VECTORS[index];
        let mut status = NumericStatus::CLEAR;
        let sample = environment.sample(Altitude::from_raw(vector.altitude_q12), &mut status);
        failures += failure_count(sample.density().raw() == vector.density_q28);
        failures += failure_count(sample.gravity().raw() == vector.gravity_q28);
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

fn check_force_model() -> u16 {
    let scenario = match parse_scenario_image(SCENARIO_IMAGE) {
        Ok(scenario) => scenario,
        Err(_) => return 1,
    };
    let environment = SimpleEarthEnvironment::from_scenario(&scenario);
    let mut failures = 0u16;
    let mut index = 0usize;
    while index < force_vectors::FORCE_CASES.len() {
        let case = force_vectors::FORCE_CASES[index];
        let truth = VerticalTruthState::fixture(
            0,
            Time::from_raw(case.time_q16),
            Altitude::from_raw(case.altitude_q12),
            Velocity::from_raw(case.velocity_q24),
            Mass::from_raw(case.mass_q12),
            Mass::from_raw(case.propellant_q12),
        );
        let mut status = NumericStatus::CLEAR;
        let sample = environment.sample(truth.altitude(), &mut status);
        failures += failure_count(sample.density().raw() == case.density_q28);
        failures += failure_count(sample.gravity().raw() == case.gravity_q28);
        let snapshot = evaluate_vertical_forces(scenario.vehicle(), &truth, sample, &mut status);
        failures += failure_count(snapshot.engine_active() == (case.engine_active != 0));
        failures += failure_count(snapshot.thrust().raw() == case.thrust_q12);
        failures += failure_count(snapshot.weight().raw() == case.weight_q12);
        failures += failure_count(snapshot.drag().raw() == case.drag_q12);
        failures += failure_count(snapshot.net_force().raw() == case.net_force_q12);
        failures += failure_count(snapshot.acceleration().raw() == case.acceleration_q28);
        failures += failure_count(status.bits() == case.expected_faults);
        index += 1;
    }
    failures
}

fn check_transitions() -> u16 {
    let scenario = match parse_scenario_image(SCENARIO_IMAGE) {
        Ok(scenario) => scenario,
        Err(_) => return 1,
    };
    let environment = SimpleEarthEnvironment::from_scenario(&scenario);
    let mut failures = 0u16;
    let mut index = 0usize;
    while index < transition_vectors::TRANSITION_CASES.len() {
        let case = transition_vectors::TRANSITION_CASES[index];
        let truth = VerticalTruthState::fixture(
            case.step,
            Time::from_raw(case.time_q16),
            Altitude::from_raw(case.altitude_q12),
            Velocity::from_raw(case.velocity_q24),
            Mass::from_raw(case.mass_q12),
            Mass::from_raw(case.propellant_q12),
        );
        let mut status = NumericStatus::CLEAR;
        let result = advance_vertical_state(&scenario, environment, &truth, &mut status);
        if case.succeeds != 0 {
            match result {
                Ok(step) => {
                    let next = step.truth();
                    failures += failure_count(next.step() == case.next_step);
                    failures += failure_count(next.time().raw() == case.next_time_q16);
                    failures += failure_count(next.altitude().raw() == case.next_altitude_q12);
                    failures += failure_count(next.velocity().raw() == case.next_velocity_q24);
                    failures +=
                        failure_count(next.acceleration().raw() == case.next_acceleration_q28);
                    failures += failure_count(next.total_mass().raw() == case.next_mass_q12);
                    failures += failure_count(next.propellant().raw() == case.next_propellant_q12);
                    failures +=
                        failure_count(step.propellant_consumed().raw() == case.consumed_q12);
                    failures += failure_count(step.engine_cutoff() == (case.engine_cutoff != 0));
                }
                Err(_) => failures += 1,
            }
        } else {
            failures += failure_count(result == Err(VerticalStepError::NumericFault));
        }
        failures += failure_count(status.bits() == case.expected_faults);
        index += 1;
    }

    let complete = VerticalTruthState::fixture(
        scenario.steps(),
        Time::from_raw(scenario.timestep().raw() * scenario.steps() as i32),
        scenario.initial().altitude(),
        scenario.initial().velocity(),
        scenario.initial().total_mass(),
        scenario.initial().propellant(),
    );
    let mut status = NumericStatus::CLEAR;
    failures += failure_count(
        advance_vertical_state(&scenario, environment, &complete, &mut status)
            == Err(VerticalStepError::ScenarioComplete),
    );
    failures += failure_count(status.is_clear());
    failures
}

fn check_scenario() -> u16 {
    let mut crc_failures = failure_count(crate::scenario::crc32_ieee(b"123456789") == 0xcbf4_3926);
    match parse_scenario_image(SCENARIO_IMAGE) {
        Ok(scenario) => {
            let mut failures = 0u16;
            failures += failure_count(scenario.scenario_id() == 0xef03_0ab2);
            failures += failure_count(scenario.timestep().raw() == 8_192);
            failures += failure_count(scenario.steps() == 2_048);
            failures += failure_count(scenario.initial().total_mass().raw() == 2_048_000);
            failures += failure_count(scenario.vehicle().thrust().raw() == 31_130);
            failures += failure_count(scenario.environment_id() == SIMPLE_EARTH_ENVIRONMENT_ID);
            let environment = SimpleEarthEnvironment::from_scenario(&scenario);
            failures += failure_count(environment.tables_are_valid());
            let truth = VerticalTruthState::initial(&scenario);
            failures += failure_count(truth.step() == 0);
            failures += failure_count(truth.time().raw() == 0);
            failures += failure_count(truth.altitude() == scenario.initial().altitude());
            failures += failure_count(truth.total_mass() == scenario.initial().total_mass());
            let mut status = NumericStatus::CLEAR;
            let sample = environment.sample(truth.altitude(), &mut status);
            failures += failure_count(sample.density().raw() == 328_833_434);
            failures += failure_count(sample.gravity().raw() == 2_632_453);
            failures += failure_count(status.is_clear());
            crc_failures += failures;
            crc_failures
        }
        Err(_) => crc_failures + 1,
    }
}

pub fn run_numeric_self_tests() -> u16 {
    let mut failures = 0u16;
    failures += failure_count(vectors::NUMERIC_CONTRACT == "ksa64.numeric.phase1-v1");
    failures += check_arithmetic();
    failures += check_interpolation();
    failures += check_constant_velocity();
    failures += check_acceleration_cases();
    failures += check_mass_flow();
    failures += check_force_model();
    failures += check_transitions();
    failures += check_scenario();
    failures
}
