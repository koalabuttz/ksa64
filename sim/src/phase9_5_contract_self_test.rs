//! Finite native/MOS exactness probe for the Phase 9.5 numeric and wire contracts.

use ksa64_core::phase9_5_contract::{
    parse_advanced_evaluation_request, write_advanced_evaluation_request,
    AdvancedEvaluationRequest, KLE9_LENGTH,
};
use ksa64_core::phase9_5_numeric::*;
use ksa64_interface::phase9_5::{
    parse_advanced_command, parse_advanced_fast_sensor, write_advanced_command,
    write_advanced_fast_sensor, AdvancedCommandCell, AdvancedFastSensorCell,
    ADVANCED_COMMAND_LENGTH, ADVANCED_FAST_SENSOR_LENGTH,
};

#[allow(dead_code)]
mod independent {
    include!("../../phase9_5/generated/contract_vectors_v1.rs");
}

fn fast() -> AdvancedFastSensorCell {
    AdvancedFastSensorCell {
        session: 1,
        measurement_epoch: 2,
        production_epoch: 3,
        validity: 63,
        platform_angle: [1, 2, 3],
        angular_rate: [4, 5, 6],
        delta_velocity: [7, 8, 9],
        dynamic_pressure_q10: 10,
        mach_q12: 11,
        gimbal_applied: [12, 13],
        canard_applied: [14, 15, 16, 17],
        valve_open_mask: 0x555,
        propellant_q21: 18,
        supply_scale_q15: 19,
        vehicle_status: 20,
        actuator_feedback: 21,
        flags: 22,
    }
}
fn command() -> AdvancedCommandCell {
    AdvancedCommandCell {
        session: 1,
        source_epoch: 2,
        effective_epoch: 3,
        flags: 0,
        discrete: 3,
        gimbal: [1, 2],
        canards: [3, 4, 5, 6],
        torque_demand_q12: [7, 8, 9],
        rcs_pulse_quanta: [1; 12],
        status: 10,
        authority_mode: 2,
        command_checksum: 11,
    }
}
fn request() -> AdvancedEvaluationRequest {
    AdvancedEvaluationRequest {
        identity: 8,
        model_profile: 4,
        reference_frame: 1,
        vehicle_identity: 2,
        motor_identity: 3,
        mission_identity: 4,
        wind_identity: 5,
        avionics_identity: 6,
        legacy_gimbal_identity: 7,
        effector_identity: 1,
        allocator_identity: 6,
        uncertainty_identity: 0,
        evaluator_identity: 9,
    }
}
fn fnv(mut hash: u32, bytes: &[u8]) -> u32 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1
    }
    hash
}
fn fnv_word(hash: u32, word: u32) -> u32 {
    fnv(hash, &word.to_le_bytes())
}

pub fn phase95_contract_signature() -> u32 {
    let mut fast_bytes = [0; ADVANCED_FAST_SENSOR_LENGTH];
    let mut command_bytes = [0; ADVANCED_COMMAND_LENGTH];
    let mut request_bytes = [0; KLE9_LENGTH];
    if write_advanced_fast_sensor(&fast(), &mut fast_bytes).is_err()
        || write_advanced_command(&command(), &mut command_bytes).is_err()
        || write_advanced_evaluation_request(&request(), &mut request_bytes).is_err()
    {
        return u32::MAX;
    }
    let mut hash = 0x811c_9dc5;
    for word in [
        ADVANCED_EFFECTOR_NUMERIC_CONTRACT_ID,
        BODY_TORQUE_DEMAND_FRACTIONAL_BITS as u32,
        ADVANCED_TIME_FRACTIONAL_BITS as u32,
        RCS_PULSE_QUANTUM_Q18 as u32,
        ADVANCED_EFFECTOR_FORCE_FRACTIONAL_BITS as u32,
        ADVANCED_HINGE_MOMENT_FRACTIONAL_BITS as u32,
        ADVANCED_SUPPLY_PRESSURE_FRACTIONAL_BITS as u32,
        ADVANCED_PULSE_IMPULSE_FRACTIONAL_BITS as u32,
        ADVANCED_MASS_FLOW_FRACTIONAL_BITS as u32,
        ADVANCED_SUPPLY_SCALE_FRACTIONAL_BITS as u32,
    ] {
        hash = fnv_word(hash, word);
    }
    hash = fnv(hash, &fast_bytes);
    hash = fnv(hash, &command_bytes);
    fnv(hash, &request_bytes)
}

pub fn run_phase95_contract_self_tests() -> u32 {
    let mut failures = u32::from(!advanced_numeric_contract_is_valid());
    let mut fast_bytes = [0; ADVANCED_FAST_SENSOR_LENGTH];
    let mut command_bytes = [0; ADVANCED_COMMAND_LENGTH];
    let mut request_bytes = [0; KLE9_LENGTH];
    if write_advanced_fast_sensor(&fast(), &mut fast_bytes).is_err()
        || fast_bytes != independent::KLR9_FAST_VECTOR
        || parse_advanced_fast_sensor(&fast_bytes) != Ok(fast())
    {
        failures += 1
    }
    if write_advanced_command(&command(), &mut command_bytes).is_err()
        || command_bytes != independent::KLR9_COMMAND_VECTOR
        || parse_advanced_command(&command_bytes) != Ok(command())
    {
        failures += 1
    }
    if write_advanced_evaluation_request(&request(), &mut request_bytes).is_err()
        || request_bytes != independent::KLE9_VECTOR
        || parse_advanced_evaluation_request(&request_bytes) != Ok(request())
    {
        failures += 1
    }
    if phase95_contract_signature() != independent::PHASE95_CONTRACT_SIGNATURE {
        failures += 1
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_signature_matches_independent_vectors() {
        assert_eq!(run_phase95_contract_self_tests(), 0);
        assert_eq!(
            phase95_contract_signature(),
            independent::PHASE95_CONTRACT_SIGNATURE
        );
    }
}
