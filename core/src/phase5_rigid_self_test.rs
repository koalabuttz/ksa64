use crate::numeric::NumericStatus;
use crate::rigid_body::{step_rigid_body, DiagonalInertiaQ12, RigidBodyState};
use crate::spatial_numeric::{AngularRateVec, QuaternionQ30, TorqueVec};

#[allow(dead_code)]
mod vectors {
    include!("../../phase5/generated/rigid_body_vectors_v1.rs");
}

#[inline]
fn failure(value: bool) -> u32 {
    if value {
        0
    } else {
        1
    }
}
fn inertia(raw: [i32; 3]) -> DiagonalInertiaQ12 {
    DiagonalInertiaQ12::new(raw[0], raw[1], raw[2])
}
fn rate(raw: [i32; 3]) -> AngularRateVec {
    AngularRateVec::new(raw[0], raw[1], raw[2])
}
fn torque(raw: [i32; 3]) -> TorqueVec {
    TorqueVec::new(raw[0], raw[1], raw[2])
}
fn attitude(raw: [i32; 4]) -> QuaternionQ30 {
    QuaternionQ30::new(raw[0], raw[1], raw[2], raw[3])
}

#[inline(never)]
fn spherical_probe() -> u32 {
    let mut failures = 0u32;
    let mut status = NumericStatus::CLEAR;
    let result = step_rigid_body(
        RigidBodyState::REST,
        inertia(vectors::SPHERICAL_INERTIA_Q12),
        torque(vectors::TORQUE_X_Q16),
        vectors::DT_Q16,
        &mut status,
    );
    let q = result.state().attitude();
    let omega = result.state().angular_rate();
    let alpha = result.angular_acceleration();
    failures |= failure(q.w() == vectors::SPHERICAL_ONE_STEP_ATTITUDE_Q30[0]);
    failures |= failure(q.x() == vectors::SPHERICAL_ONE_STEP_ATTITUDE_Q30[1]);
    failures |= failure(q.y() == vectors::SPHERICAL_ONE_STEP_ATTITUDE_Q30[2]);
    failures |= failure(q.z() == vectors::SPHERICAL_ONE_STEP_ATTITUDE_Q30[3]);
    failures |= failure(omega.x() == vectors::SPHERICAL_ONE_STEP_RATE_Q24[0]);
    failures |= failure(omega.y() == vectors::SPHERICAL_ONE_STEP_RATE_Q24[1]);
    failures |= failure(omega.z() == vectors::SPHERICAL_ONE_STEP_RATE_Q24[2]);
    failures |= failure(alpha.x() == vectors::SPHERICAL_ALPHA_Q24[0]);
    failures |= failure(alpha.y() == vectors::SPHERICAL_ALPHA_Q24[1]);
    failures |= failure(alpha.z() == vectors::SPHERICAL_ALPHA_Q24[2]);
    failures | failure(status.is_clear())
}

#[inline(never)]
fn asymmetric_probe() -> u32 {
    let mut failures = 0u32;
    let mut status = NumericStatus::CLEAR;
    let result = step_rigid_body(
        RigidBodyState::new(
            attitude(vectors::ASYMMETRIC_INITIAL_ATTITUDE_Q30),
            rate(vectors::ASYMMETRIC_INITIAL_RATE_Q24),
        ),
        inertia(vectors::ASYMMETRIC_INERTIA_Q12),
        TorqueVec::ZERO,
        vectors::DT_Q16,
        &mut status,
    );
    let q = result.state().attitude();
    let omega = result.state().angular_rate();
    let alpha = result.angular_acceleration();
    failures |= failure(q.w() == vectors::ASYMMETRIC_ONE_STEP_ATTITUDE_Q30[0]);
    failures |= failure(q.x() == vectors::ASYMMETRIC_ONE_STEP_ATTITUDE_Q30[1]);
    failures |= failure(q.y() == vectors::ASYMMETRIC_ONE_STEP_ATTITUDE_Q30[2]);
    failures |= failure(q.z() == vectors::ASYMMETRIC_ONE_STEP_ATTITUDE_Q30[3]);
    failures |= failure(omega.x() == vectors::ASYMMETRIC_ONE_STEP_RATE_Q24[0]);
    failures |= failure(omega.y() == vectors::ASYMMETRIC_ONE_STEP_RATE_Q24[1]);
    failures |= failure(omega.z() == vectors::ASYMMETRIC_ONE_STEP_RATE_Q24[2]);
    failures |= failure(alpha.x() == vectors::ASYMMETRIC_ALPHA_Q24[0]);
    failures |= failure(alpha.y() == vectors::ASYMMETRIC_ALPHA_Q24[1]);
    failures |= failure(alpha.z() == vectors::ASYMMETRIC_ALPHA_Q24[2]);
    failures | failure(status.is_clear())
}

pub fn run_phase5_rigid_self_tests() -> u32 {
    let spherical = spherical_probe();
    if spherical != 0 {
        return 1;
    }
    if asymmetric_probe() != 0 {
        return 2;
    }
    0
}
