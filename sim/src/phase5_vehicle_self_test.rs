use crate::phase5_vehicle::{
    ksa5a_stage, GimbalCommandQ16, Phase5VehicleCommand, Phase5VehicleMachine, TwoAxisGimbalQ16,
    PHASE5_GIMBAL_LIMIT_Q16, PHASE5_VEHICLE_SIGNATURE,
};
use ksa64_core::numeric::NumericStatus;
use ksa64_core::spatial_numeric::FixedVec3;
use ksa64_interface::EngineAction;

#[inline]
fn hash(mut value: u32, word: u32) -> u32 {
    for byte in word.to_le_bytes() {
        value = (value ^ byte as u32).wrapping_mul(16_777_619);
    }
    value
}

fn hash_snapshot(
    mut signature: u32,
    snapshot: crate::phase5_vehicle::Phase5VehicleSnapshot,
) -> u32 {
    let truth = snapshot.truth;
    let spatial = truth.spatial();
    let position = spatial.position();
    let velocity = spatial.velocity();
    let rigid = truth.rigid();
    let attitude = rigid.attitude();
    let rate = rigid.angular_rate();
    let flexible = truth.flexible();
    for value in [
        truth.step() as i32,
        truth.time_q16(),
        truth.total_mass_q12(),
        truth.active_propellant_q12(),
        truth.active_stage() as i32,
        truth.phase() as i32,
        position.x(),
        position.y(),
        position.z(),
        velocity.x(),
        velocity.y(),
        velocity.z(),
        attitude.w(),
        attitude.x(),
        attitude.y(),
        attitude.z(),
        rate.x(),
        rate.y(),
        rate.z(),
        flexible.y().bending().displacement(),
        flexible.y().bending().rate(),
        flexible.y().slosh().displacement(),
        flexible.y().slosh().rate(),
        flexible.z().bending().displacement(),
        flexible.z().bending().rate(),
        flexible.z().slosh().displacement(),
        flexible.z().slosh().rate(),
        snapshot.gimbal.applied.pitch,
        snapshot.gimbal.applied.yaw,
        snapshot.inertia.x(),
        snapshot.inertia.y(),
        snapshot.inertia.z(),
        snapshot.rcs_propellant_q12,
        snapshot.events as i32,
        snapshot.mach.raw(),
        snapshot.dynamic_pressure_q16,
        snapshot.angle_of_attack_sine_q16,
    ] {
        signature = hash(signature, value as u32);
    }
    signature
}

pub fn phase5_vehicle_signature() -> u32 {
    let mut signature = 2_166_136_261u32;
    let mut gimbal = TwoAxisGimbalQ16::new();
    let request = GimbalCommandQ16 {
        pitch: PHASE5_GIMBAL_LIMIT_Q16,
        yaw: -PHASE5_GIMBAL_LIMIT_Q16,
    };
    let mut gimbal_snapshot = gimbal.advance(request);
    for _ in 1..8 {
        gimbal_snapshot = gimbal.advance(request);
    }
    signature = hash(signature, gimbal_snapshot.lagged.pitch as u32);
    signature = hash(signature, gimbal_snapshot.lagged.yaw as u32);
    signature = hash(signature, gimbal_snapshot.applied.pitch as u32);
    signature = hash(signature, gimbal_snapshot.applied.yaw as u32);

    let stage = match ksa5a_stage(0) {
        Some(value) => value,
        None => return 0,
    };
    let mut status = NumericStatus::CLEAR;
    let inertia = stage
        .inertia
        .interpolate(stage.propellant_mass_q12 / 2, &mut status);
    if !status.is_clear() {
        return 0;
    }
    signature = hash(signature, inertia.x() as u32);
    signature = hash(signature, inertia.y() as u32);
    signature = hash(signature, inertia.z() as u32);

    let mut machine = match Phase5VehicleMachine::new_ksa5a() {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let first = match machine.step(Phase5VehicleCommand {
        gimbal: request,
        engine_action: EngineAction::Ignite,
        ..Phase5VehicleCommand::HOLD
    }) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    signature = hash_snapshot(signature, first);
    if machine
        .step(Phase5VehicleCommand {
            engine_action: EngineAction::Cutoff,
            ..Phase5VehicleCommand::HOLD
        })
        .is_err()
    {
        return 0;
    }
    for _ in 0..7 {
        if machine.step(Phase5VehicleCommand::HOLD).is_err() {
            return 0;
        }
    }
    let upper = match machine.step(Phase5VehicleCommand {
        separate: true,
        ..Phase5VehicleCommand::HOLD
    }) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    signature = hash_snapshot(signature, upper);
    let rcs = match machine.step(Phase5VehicleCommand {
        rcs_q15: FixedVec3::new(32_767, -16_384, 8_192),
        ..Phase5VehicleCommand::HOLD
    }) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    hash_snapshot(signature, rcs)
}

pub fn run_phase5_vehicle_self_tests() -> u32 {
    if phase5_vehicle_signature() == PHASE5_VEHICLE_SIGNATURE {
        0
    } else {
        1
    }
}
