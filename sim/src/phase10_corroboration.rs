//! Phase 10 cross-profile KSA-5A orbital-coast corroboration.
//!
//! The accepted Phase 5 ascent remains the sole producer of the insertion
//! state. This module copies that frozen state into the Phase 10 GCRF numeric
//! contract and propagates only the subsequent coast.

use crate::phase5_mission::{
    run_phase5_mission, Phase5MissionCase, Phase5MissionOutcome, Phase5MissionSummary,
};
use ksa64_core::numeric::{magnitude3_floor, multiply_scaled, NumericStatus};
use ksa64_core::phase10_contract::{EarthModelPack, TransformPack};
use ksa64_core::phase10_environment::central_j2_gravity;
use ksa64_core::phase10_frames::interpolate_transform;
use ksa64_core::phase10_numeric::{
    integrate_with_residual, GlobalAccelerationVec, GlobalPositionVec, GlobalVelocityVec,
    MissionTimeQ16, GLOBAL_COAST_STEP_Q16,
};
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::spatial_numeric::FixedVec3;

pub const KSA5_HANDOFF_IDENTITY: u32 = 0x105a_0001;
pub const KSA5_COAST_DURATION_Q16: u32 = 5_350 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorroborationError {
    Phase5,
    Identity,
    Coverage,
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ksa5HandoffFixture {
    pub identity: u32,
    pub phase5_summary_checksum: u32,
    pub position_q12_km: [i32; 3],
    pub velocity_q24_km_s: [i32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ksa5CoastSummary {
    pub handoff: Ksa5HandoffFixture,
    pub duration_q16: u32,
    pub steps: u32,
    pub terminal_position_q12_km: [i32; 3],
    pub terminal_velocity_q24_km_s: [i32; 3],
    pub minimum_altitude_q12_km: i32,
    pub maximum_altitude_q12_km: i32,
    pub maximum_position_delta_q12_km: i32,
    pub maximum_velocity_delta_q24_km_s: i32,
    pub checksum: u32,
}

pub fn frozen_ksa5_handoff() -> Result<Ksa5HandoffFixture, CorroborationError> {
    let summary =
        run_phase5_mission(Phase5MissionCase::Nominal).map_err(|_| CorroborationError::Phase5)?;
    handoff_from_summary(summary)
}

pub fn handoff_from_summary(
    summary: Phase5MissionSummary,
) -> Result<Ksa5HandoffFixture, CorroborationError> {
    if summary.outcome != Phase5MissionOutcome::StableOrbit
        || summary.summary_checksum != 557_491_580
    {
        return Err(CorroborationError::Identity);
    }
    Ok(Ksa5HandoffFixture {
        identity: KSA5_HANDOFF_IDENTITY,
        phase5_summary_checksum: summary.summary_checksum,
        position_q12_km: summary.terminal_position_q12,
        velocity_q24_km_s: summary.terminal_velocity_q24,
    })
}

pub fn coast_frozen_ksa5_one_orbit(
    earth: &EarthModelPack,
    transforms: &TransformPack,
) -> Result<Ksa5CoastSummary, CorroborationError> {
    let handoff = frozen_ksa5_handoff()?;
    coast_ksa5_handoff(earth, transforms, handoff, KSA5_COAST_DURATION_Q16)
}

pub fn coast_ksa5_handoff(
    earth: &EarthModelPack,
    transforms: &TransformPack,
    handoff: Ksa5HandoffFixture,
    duration_q16: u32,
) -> Result<Ksa5CoastSummary, CorroborationError> {
    if handoff.identity != KSA5_HANDOFF_IDENTITY
        || handoff.phase5_summary_checksum != 557_491_580
        || duration_q16 == 0
        || !duration_q16.is_multiple_of(GLOBAL_COAST_STEP_Q16)
    {
        return Err(CorroborationError::Identity);
    }
    let mut position = GlobalPositionVec::new(
        handoff.position_q12_km[0],
        handoff.position_q12_km[1],
        handoff.position_q12_km[2],
    );
    let initial_position = position;
    let mut velocity = GlobalVelocityVec::new(
        handoff.velocity_q24_km_s[0],
        handoff.velocity_q24_km_s[1],
        handoff.velocity_q24_km_s[2],
    );
    let initial_velocity = velocity;
    let mut time = MissionTimeQ16::ZERO;
    let mut minimum_altitude = i32::MAX;
    let mut maximum_altitude = i32::MIN;
    let steps = duration_q16 / GLOBAL_COAST_STEP_Q16;
    let mut checksum = 0x811c_9dc5;
    let mut position_residual_q28 = [0i64; 3];
    let mut velocity_residual_q20 = [0i64; 3];
    for step in 0..steps {
        let a0 = inertial_gravity(earth, transforms, time, position)?;
        let mut status = NumericStatus::CLEAR;
        let half = (GLOBAL_COAST_STEP_Q16 / 2) as i32;
        let midpoint_velocity = velocity.checked_add(
            FixedVec3::new(
                multiply_scaled(a0.x(), half, 20, &mut status),
                multiply_scaled(a0.y(), half, 20, &mut status),
                multiply_scaled(a0.z(), half, 20, &mut status),
            ),
            &mut status,
        );
        let midpoint_position = position.checked_add(
            FixedVec3::new(
                multiply_scaled(velocity.x(), half, 28, &mut status),
                multiply_scaled(velocity.y(), half, 28, &mut status),
                multiply_scaled(velocity.z(), half, 28, &mut status),
            ),
            &mut status,
        );
        let midpoint_time = time.checked_add(GLOBAL_COAST_STEP_Q16 / 2, &mut status);
        let am = inertial_gravity(earth, transforms, midpoint_time, midpoint_position)?;
        let acceleration = [am.x(), am.y(), am.z()];
        let midpoint_rate = [
            midpoint_velocity.x(),
            midpoint_velocity.y(),
            midpoint_velocity.z(),
        ];
        let mut velocity_delta = [0; 3];
        let mut position_delta = [0; 3];
        for axis in 0..3 {
            velocity_delta[axis] = integrate_with_residual(
                acceleration[axis],
                GLOBAL_COAST_STEP_Q16,
                20,
                &mut velocity_residual_q20[axis],
                &mut status,
            );
            position_delta[axis] = integrate_with_residual(
                midpoint_rate[axis],
                GLOBAL_COAST_STEP_Q16,
                28,
                &mut position_residual_q28[axis],
                &mut status,
            );
        }
        velocity = velocity.checked_add(
            FixedVec3::<24>::new(velocity_delta[0], velocity_delta[1], velocity_delta[2]),
            &mut status,
        );
        position = position.checked_add(
            FixedVec3::<12>::new(position_delta[0], position_delta[1], position_delta[2]),
            &mut status,
        );
        time = time.checked_add(GLOBAL_COAST_STEP_Q16, &mut status);
        if !status.is_clear() {
            return Err(CorroborationError::Numeric);
        }
        let altitude = magnitude(position)?.saturating_sub(EARTH_RADIUS_Q12);
        minimum_altitude = minimum_altitude.min(altitude);
        maximum_altitude = maximum_altitude.max(altitude);
        if step % 32 == 0 {
            checksum = hash_state(checksum, time.raw(), position, velocity);
        }
    }
    Ok(Ksa5CoastSummary {
        handoff,
        duration_q16,
        steps,
        terminal_position_q12_km: [position.x(), position.y(), position.z()],
        terminal_velocity_q24_km_s: [velocity.x(), velocity.y(), velocity.z()],
        minimum_altitude_q12_km: minimum_altitude,
        maximum_altitude_q12_km: maximum_altitude,
        maximum_position_delta_q12_km: vector_delta(position, initial_position)?,
        maximum_velocity_delta_q24_km_s: vector_delta(velocity, initial_velocity)?,
        checksum,
    })
}

fn inertial_gravity(
    earth: &EarthModelPack,
    transforms: &TransformPack,
    time: MissionTimeQ16,
    position_gcrf: GlobalPositionVec,
) -> Result<GlobalAccelerationVec, CorroborationError> {
    let transform =
        interpolate_transform(transforms, time).map_err(|_| CorroborationError::Coverage)?;
    let mut status = NumericStatus::CLEAR;
    let position_ecef = transform
        .ecef_to_gcrf
        .conjugate()
        .rotate(position_gcrf, &mut status);
    let gravity_ecef =
        central_j2_gravity(earth, position_ecef).map_err(|_| CorroborationError::Numeric)?;
    let gravity_gcrf = transform.ecef_to_gcrf.rotate(gravity_ecef, &mut status);
    if status.is_clear() {
        Ok(gravity_gcrf)
    } else {
        Err(CorroborationError::Numeric)
    }
}

fn magnitude<const F: u8>(value: FixedVec3<F>) -> Result<i32, CorroborationError> {
    let mut status = NumericStatus::CLEAR;
    let result = magnitude3_floor(value.x(), value.y(), value.z(), &mut status);
    if status.is_clear() && result <= i32::MAX as u32 {
        Ok(result as i32)
    } else {
        Err(CorroborationError::Numeric)
    }
}

fn vector_delta<const F: u8>(a: FixedVec3<F>, b: FixedVec3<F>) -> Result<i32, CorroborationError> {
    magnitude(FixedVec3::<F>::new(
        a.x().saturating_sub(b.x()),
        a.y().saturating_sub(b.y()),
        a.z().saturating_sub(b.z()),
    ))
}

fn hash_state(
    mut hash: u32,
    time: u32,
    position: GlobalPositionVec,
    velocity: GlobalVelocityVec,
) -> u32 {
    for value in [
        time,
        position.x() as u32,
        position.y() as u32,
        position.z() as u32,
        velocity.x() as u32,
        velocity.y() as u32,
        velocity.z() as u32,
    ] {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    mod reference {
        include!("../../phase10/generated/ksa5_coast_reference_v1.rs");
    }

    #[test]
    fn frozen_ascent_handoff_is_byte_exact() {
        let handoff = frozen_ksa5_handoff().unwrap();
        assert_eq!(handoff.position_q12_km, [21_468_577, 3_871_182, 15_698_368]);
        assert_eq!(
            handoff.velocity_q24_km_s,
            [-66_327_286, 89_767_125, 68_337_641]
        );
    }

    #[test]
    fn one_orbit_coast_remains_near_the_accepted_orbit() {
        let earth =
            EarthModelPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kem10"))
                .unwrap();
        let transforms =
            TransformPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kft10"))
                .unwrap();
        let result = coast_frozen_ksa5_one_orbit(&earth, &transforms).unwrap();
        assert_eq!(result.steps, 171_200);
        assert!(
            result.minimum_altitude_q12_km >= 170 << 12,
            "min={} max={} final_p={:?} final_v={:?}",
            result.minimum_altitude_q12_km as f64 / 4096.0,
            result.maximum_altitude_q12_km as f64 / 4096.0,
            result.terminal_position_q12_km,
            result.terminal_velocity_q24_km_s
        );
        assert!(result.maximum_altitude_q12_km <= 220 << 12);
        assert_ne!(result.checksum, 0);
        for axis in 0..3 {
            assert!(
                (result.terminal_position_q12_km[axis]
                    - reference::KSA5_FLOAT64_POSITION_Q12[axis])
                    .abs()
                    <= 5 << 12
            );
            assert!(
                (result.terminal_velocity_q24_km_s[axis]
                    - reference::KSA5_FLOAT64_VELOCITY_Q24[axis])
                    .abs()
                    <= 84_000
            );
        }
        assert!(
            (result.minimum_altitude_q12_km - reference::KSA5_FLOAT64_MIN_ALTITUDE_Q12).abs()
                <= 5 << 12
        );
        assert!(
            (result.maximum_altitude_q12_km - reference::KSA5_FLOAT64_MAX_ALTITUDE_Q12).abs()
                <= 5 << 12
        );
    }
}
