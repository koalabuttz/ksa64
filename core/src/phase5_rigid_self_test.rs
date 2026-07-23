use crate::numeric::NumericStatus;
use crate::rigid_body::{
    angular_acceleration, step_rigid_body, DiagonalInertiaQ12, RigidBodyState,
};
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

#[inline(never)]
fn probe(case: u8) -> u32 {
    let (state, inertia_raw, torque_raw, q_expected, rate_expected, alpha_expected) = if case == 0 {
        (
            RigidBodyState::REST,
            vectors::SPHERICAL_INERTIA_Q12,
            vectors::TORQUE_X_Q16,
            vectors::SPHERICAL_ONE_STEP_ATTITUDE_Q30,
            vectors::SPHERICAL_ONE_STEP_RATE_Q24,
            vectors::SPHERICAL_ALPHA_Q24,
        )
    } else {
        (
            RigidBodyState::new(
                QuaternionQ30::new(
                    vectors::ASYMMETRIC_INITIAL_ATTITUDE_Q30[0],
                    vectors::ASYMMETRIC_INITIAL_ATTITUDE_Q30[1],
                    vectors::ASYMMETRIC_INITIAL_ATTITUDE_Q30[2],
                    vectors::ASYMMETRIC_INITIAL_ATTITUDE_Q30[3],
                ),
                AngularRateVec::new(
                    vectors::ASYMMETRIC_INITIAL_RATE_Q24[0],
                    vectors::ASYMMETRIC_INITIAL_RATE_Q24[1],
                    vectors::ASYMMETRIC_INITIAL_RATE_Q24[2],
                ),
            ),
            vectors::ASYMMETRIC_INERTIA_Q12,
            [0, 0, 0],
            vectors::ASYMMETRIC_ONE_STEP_ATTITUDE_Q30,
            vectors::ASYMMETRIC_ONE_STEP_RATE_Q24,
            vectors::ASYMMETRIC_ALPHA_Q24,
        )
    };
    let mut status = NumericStatus::CLEAR;
    let result = step_rigid_body(
        state,
        DiagonalInertiaQ12::new(inertia_raw[0], inertia_raw[1], inertia_raw[2]),
        TorqueVec::new(torque_raw[0], torque_raw[1], torque_raw[2]),
        vectors::DT_Q16,
        &mut status,
    );
    let q = result.state().attitude();
    let omega = result.state().angular_rate();
    let alpha = result.angular_acceleration();
    let mut failures = failure(status.is_clear());
    failures |= failure([q.w(), q.x(), q.y(), q.z()] == q_expected);
    failures |= failure([omega.x(), omega.y(), omega.z()] == rate_expected);
    failures |= failure([alpha.x(), alpha.y(), alpha.z()] == alpha_expected);
    failures
}

pub fn run_phase5_rigid_spherical_self_test() -> u32 {
    probe(0)
}
#[inline(never)]
fn asymmetric_acceleration_probe() -> u32 {
    let mut status = NumericStatus::CLEAR;
    let inertia = vectors::ASYMMETRIC_INERTIA_Q12;
    let rate = vectors::ASYMMETRIC_INITIAL_RATE_Q24;
    let alpha = angular_acceleration(
        DiagonalInertiaQ12::new(inertia[0], inertia[1], inertia[2]),
        AngularRateVec::new(rate[0], rate[1], rate[2]),
        TorqueVec::ZERO,
        &mut status,
    );
    failure(status.is_clear())
        | failure([alpha.x(), alpha.y(), alpha.z()] == vectors::ASYMMETRIC_ALPHA_Q24)
}

pub fn run_phase5_rigid_asymmetric_self_test() -> u32 {
    asymmetric_acceleration_probe()
}
pub fn run_phase5_rigid_self_tests() -> u32 {
    let spherical = probe(0);
    if spherical != 0 {
        return 1;
    }
    if probe(1) != 0 {
        return 2;
    }
    0
}
