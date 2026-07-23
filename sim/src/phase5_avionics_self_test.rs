use ksa64_flight::phase5_gnc::{attitude_command, SpatialGuidanceTarget};
use ksa64_flight::phase5_navigation::{
    SpatialNavigationState, INITIAL_ATTITUDE_Q30, INITIAL_POSITION_Q12, INITIAL_VELOCITY_Q24,
};
use ksa64_interface::phase5::{
    parse_spatial_actuator_command, parse_spatial_sensor_frame, write_spatial_actuator_command,
    write_spatial_sensor_frame, SPATIAL_ACTUATOR_COMMAND_LENGTH, SPATIAL_SENSOR_FRAME_LENGTH,
};

mod generated {
    include!("../../phase5/generated/avionics_v1.rs");
}

fn hash_bytes(mut hash: u32, bytes: &[u8]) -> u32 {
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    hash
}

pub fn phase5_avionics_signature() -> u32 {
    let sensor = match parse_spatial_sensor_frame(&generated::SENSOR_BYTES) {
        Ok(value) => value,
        Err(ksa64_interface::CodecError::Length) => return 0,
        Err(ksa64_interface::CodecError::Checksum) => return 0,
        Err(ksa64_interface::CodecError::Reserved) => return 0,
        Err(ksa64_interface::CodecError::Flags) => return 0,
        Err(ksa64_interface::CodecError::Enum) => return 0,
        Err(ksa64_interface::CodecError::Sequence) => return 0,
    };
    let mut sensor_bytes = [0u8; SPATIAL_SENSOR_FRAME_LENGTH];
    if write_spatial_sensor_frame(&sensor, &mut sensor_bytes).is_err()
        || sensor_bytes != generated::SENSOR_BYTES
    {
        return 0;
    }
    let command = match parse_spatial_actuator_command(&generated::COMMAND_BYTES) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let mut command_bytes = [0u8; SPATIAL_ACTUATOR_COMMAND_LENGTH];
    if write_spatial_actuator_command(&command, &mut command_bytes).is_err()
        || command_bytes != generated::COMMAND_BYTES
    {
        return 0;
    }
    let mut signature = hash_bytes(2_166_136_261, &sensor_bytes);
    signature = hash_bytes(signature, &command_bytes);
    let mut case = 0;
    while case < generated::CONTROLLER_EXPECTED.len() {
        let state = SpatialNavigationState {
            sequence: 7,
            time_q16: 57_344,
            position_q12: INITIAL_POSITION_Q12,
            velocity_q24: INITIAL_VELOCITY_Q24,
            attitude_q30: INITIAL_ATTITUDE_Q30,
            angular_rate_q24: generated::CONTROLLER_RATE_Q24[case],
            gps_aided: false,
            star_aided: false,
            barometer_aided: false,
            checksum: 0,
        };
        let output = attitude_command(
            7,
            state,
            SpatialGuidanceTarget {
                attitude_q30: generated::CONTROLLER_TARGET_Q30[case],
                angular_rate_q24: [0; 3],
            },
        );
        let actual = [
            output.gimbal_q16[0],
            output.gimbal_q16[1],
            output.rcs_q15[0],
            output.rcs_q15[1],
            output.rcs_q15[2],
        ];
        if actual != generated::CONTROLLER_EXPECTED[case] {
            return 0;
        }
        let mut word = 0;
        while word < actual.len() {
            signature = hash_bytes(signature, &actual[word].to_le_bytes());
            word += 1;
        }
        case += 1;
    }
    signature
}

pub fn run_phase5_avionics_self_tests() -> u32 {
    if phase5_avionics_signature() == generated::AVIONICS_SIGNATURE {
        0
    } else {
        1
    }
}
