//! Phase 10 kilogram-scale diagonal rigid-body propagation.
//!
//! The global profile retains Phase 10's Q24 body angular-rate and Q30
//! quaternion contracts while using kilogram-metre-squared Q19 inertia and
//! newton-metre Q12 torque.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus};
use crate::phase10_numeric::GlobalAngularRateVec;
use crate::spatial_numeric::{FixedVec3, QuaternionQ30};

pub type GlobalBodyTorque = FixedVec3<12>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GlobalDiagonalInertiaQ19 {
    pub axes: [i32; 3],
}

impl GlobalDiagonalInertiaQ19 {
    pub const fn new(axes: [i32; 3]) -> Self {
        Self { axes }
    }

    pub const fn is_valid(self) -> bool {
        self.axes[0] > 0 && self.axes[1] > 0 && self.axes[2] > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GlobalRigidBodyState {
    pub attitude: QuaternionQ30,
    pub angular_rate: GlobalAngularRateVec,
}

fn coupled_torque_q12(
    inertia_difference_q19: i32,
    rate_a_q24: i32,
    rate_b_q24: i32,
    status: &mut NumericStatus,
) -> i32 {
    let rate_product_q24 = multiply_scaled(rate_a_q24, rate_b_q24, 24, status);
    multiply_scaled(inertia_difference_q19, rate_product_q24, 31, status)
}

pub fn global_angular_acceleration(
    inertia: GlobalDiagonalInertiaQ19,
    angular_rate: GlobalAngularRateVec,
    torque: GlobalBodyTorque,
    status: &mut NumericStatus,
) -> GlobalAngularRateVec {
    if !inertia.is_valid() {
        status.record(NumericFault::InvalidInput);
        return GlobalAngularRateVec::ZERO;
    }
    let coupled = [
        coupled_torque_q12(
            subtract(inertia.axes[2], inertia.axes[1], status),
            angular_rate.y(),
            angular_rate.z(),
            status,
        ),
        coupled_torque_q12(
            subtract(inertia.axes[0], inertia.axes[2], status),
            angular_rate.z(),
            angular_rate.x(),
            status,
        ),
        coupled_torque_q12(
            subtract(inertia.axes[1], inertia.axes[0], status),
            angular_rate.x(),
            angular_rate.y(),
            status,
        ),
    ];
    GlobalAngularRateVec::new(
        divide_scaled(
            subtract(torque.x(), coupled[0], status),
            inertia.axes[0],
            31,
            status,
        ),
        divide_scaled(
            subtract(torque.y(), coupled[1], status),
            inertia.axes[1],
            31,
            status,
        ),
        divide_scaled(
            subtract(torque.z(), coupled[2], status),
            inertia.axes[2],
            31,
            status,
        ),
    )
}

fn integrate_attitude(
    attitude: QuaternionQ30,
    angular_rate: GlobalAngularRateVec,
    timestep_q16: i32,
    status: &mut NumericStatus,
) -> QuaternionQ30 {
    let w = attitude.w();
    let x = attitude.x();
    let y = attitude.y();
    let z = attitude.z();
    let wx = angular_rate.x();
    let wy = angular_rate.y();
    let wz = angular_rate.z();
    let dw_q24 = subtract(
        subtract(
            subtract(0, multiply_scaled(x, wx, 30, status), status),
            multiply_scaled(y, wy, 30, status),
            status,
        ),
        multiply_scaled(z, wz, 30, status),
        status,
    );
    let dx_q24 = add(
        add(
            multiply_scaled(w, wx, 30, status),
            multiply_scaled(y, wz, 30, status),
            status,
        ),
        subtract(0, multiply_scaled(z, wy, 30, status), status),
        status,
    );
    let dy_q24 = add(
        add(
            multiply_scaled(w, wy, 30, status),
            multiply_scaled(z, wx, 30, status),
            status,
        ),
        subtract(0, multiply_scaled(x, wz, 30, status), status),
        status,
    );
    let dz_q24 = add(
        add(
            multiply_scaled(w, wz, 30, status),
            multiply_scaled(x, wy, 30, status),
            status,
        ),
        subtract(0, multiply_scaled(y, wx, 30, status), status),
        status,
    );
    QuaternionQ30::new(
        add(w, multiply_scaled(dw_q24, timestep_q16, 11, status), status),
        add(x, multiply_scaled(dx_q24, timestep_q16, 11, status), status),
        add(y, multiply_scaled(dy_q24, timestep_q16, 11, status), status),
        add(z, multiply_scaled(dz_q24, timestep_q16, 11, status), status),
    )
    .normalized(status)
}

pub fn step_global_rigid_body(
    state: GlobalRigidBodyState,
    inertia: GlobalDiagonalInertiaQ19,
    torque: GlobalBodyTorque,
    timestep_q16: i32,
    status: &mut NumericStatus,
) -> GlobalRigidBodyState {
    if timestep_q16 <= 0 {
        status.record(NumericFault::InvalidInput);
        return state;
    }
    let acceleration = global_angular_acceleration(inertia, state.angular_rate, torque, status);
    let next_rate = state
        .angular_rate
        .checked_add(acceleration.scale::<16>(timestep_q16, status), status);
    let next_attitude = integrate_attitude(state.attitude, next_rate, timestep_q16, status);
    if status.is_clear() {
        GlobalRigidBodyState {
            attitude: next_attitude,
            angular_rate: next_rate,
        }
    } else {
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_torque_matches_one_step_rate() {
        let inertia = GlobalDiagonalInertiaQ19::new([10 << 19, 20 << 19, 30 << 19]);
        let mut status = NumericStatus::CLEAR;
        let next = step_global_rigid_body(
            GlobalRigidBodyState {
                attitude: QuaternionQ30::IDENTITY,
                angular_rate: GlobalAngularRateVec::ZERO,
            },
            inertia,
            GlobalBodyTorque::new(10 << 12, 0, 0),
            1 << 16,
            &mut status,
        );
        assert!(status.is_clear());
        assert!((next.angular_rate.x() - (1 << 24)).abs() <= 1);
        assert_eq!(next.angular_rate.y(), 0);
        assert_eq!(next.angular_rate.z(), 0);
    }

    #[test]
    fn torque_free_rest_is_exact() {
        let mut status = NumericStatus::CLEAR;
        let state = GlobalRigidBodyState {
            attitude: QuaternionQ30::IDENTITY,
            angular_rate: GlobalAngularRateVec::ZERO,
        };
        let next = step_global_rigid_body(
            state,
            GlobalDiagonalInertiaQ19::new([1 << 19; 3]),
            GlobalBodyTorque::ZERO,
            512,
            &mut status,
        );
        assert!(status.is_clear());
        assert_eq!(next, state);
    }
}
