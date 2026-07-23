use crate::numeric::NumericStatus;
use crate::spatial_numeric::{ForceVec, PositionVec, VelocityVec};
use crate::spatial_world::{
    advance_spatial_state, classify_spatial_orbit, evaluate_spatial_environment, SpatialState,
};

#[allow(dead_code)]
mod vectors {
    include!("../../phase5/generated/spatial_world_tables_v1.rs");
}

const EXPECTED_SIGNATURE: u32 = vectors::WORLD_SIGNATURE;

#[inline]
fn hash(mut value: u32, word: u32) -> u32 {
    let bytes = word.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        value = (value ^ bytes[index] as u32).wrapping_mul(16_777_619);
        index += 1;
    }
    value
}

pub fn phase5_world_signature() -> u32 {
    let launch = PositionVec::new(
        vectors::LAUNCH_POSITION_Q12[0],
        vectors::LAUNCH_POSITION_Q12[1],
        vectors::LAUNCH_POSITION_Q12[2],
    );
    let circular = SpatialState::new(
        PositionVec::new(
            vectors::CIRCULAR_POSITION_Q12[0],
            vectors::CIRCULAR_POSITION_Q12[1],
            vectors::CIRCULAR_POSITION_Q12[2],
        ),
        VelocityVec::new(
            vectors::CIRCULAR_VELOCITY_Q24[0],
            vectors::CIRCULAR_VELOCITY_Q24[1],
            vectors::CIRCULAR_VELOCITY_Q24[2],
        ),
    );
    let mut status = NumericStatus::CLEAR;
    let launch_environment =
        evaluate_spatial_environment(SpatialState::new(launch, VelocityVec::ZERO), &mut status);
    let corotating_environment = evaluate_spatial_environment(
        SpatialState::new(launch, launch_environment.atmosphere_velocity()),
        &mut status,
    );
    let orbit = match classify_spatial_orbit(circular, &mut status) {
        Some(value) => value,
        None => return 0,
    };
    let advanced = advance_spatial_state(circular, ForceVec::ZERO, 100 << 12, 8_192, &mut status);
    if !status.is_clear() {
        return 0;
    }
    let mut signature = 2_166_136_261u32;
    let gravity = launch_environment.gravity();
    signature = hash(signature, gravity.x() as u32);
    signature = hash(signature, gravity.y() as u32);
    signature = hash(signature, gravity.z() as u32);
    signature = hash(signature, corotating_environment.air_speed_q24() as u32);
    signature = hash(signature, orbit.class() as u32);
    signature = hash(signature, orbit.specific_energy().raw() as u32);
    signature = hash(signature, orbit.eccentricity().raw() as u32);
    signature = hash(signature, orbit.perigee().raw() as u32);
    signature = hash(signature, orbit.apogee().raw() as u32);
    signature = hash(signature, orbit.inclination_turn16() as u32);
    let position = advanced.position();
    let velocity = advanced.velocity();
    signature = hash(signature, position.x() as u32);
    signature = hash(signature, position.y() as u32);
    signature = hash(signature, position.z() as u32);
    signature = hash(signature, velocity.x() as u32);
    signature = hash(signature, velocity.y() as u32);
    hash(signature, velocity.z() as u32)
}

pub fn run_phase5_world_self_tests() -> u32 {
    if phase5_world_signature() == EXPECTED_SIGNATURE {
        0
    } else {
        1
    }
}
