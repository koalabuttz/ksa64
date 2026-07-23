//! Checked diagonal-inertia rigid-body propagation for Phase 5.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus};
use crate::spatial_numeric::{AngularRateVec, QuaternionQ30, TorqueVec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct DiagonalInertiaQ12 {
    x: i32,
    y: i32,
    z: i32,
}

impl DiagonalInertiaQ12 {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn y(self) -> i32 {
        self.y
    }

    pub const fn z(self) -> i32 {
        self.z
    }

    pub const fn is_valid(self) -> bool {
        self.x > 0 && self.y > 0 && self.z > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct RigidBodyState {
    attitude: QuaternionQ30,
    angular_rate: AngularRateVec,
}

impl RigidBodyState {
    pub const REST: Self = Self::new(QuaternionQ30::IDENTITY, AngularRateVec::ZERO);

    pub const fn new(attitude: QuaternionQ30, angular_rate: AngularRateVec) -> Self {
        Self {
            attitude,
            angular_rate,
        }
    }

    pub const fn attitude(self) -> QuaternionQ30 {
        self.attitude
    }

    pub const fn angular_rate(self) -> AngularRateVec {
        self.angular_rate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct RigidBodyStep {
    state: RigidBodyState,
    angular_acceleration: AngularRateVec,
}

impl RigidBodyStep {
    pub const fn state(self) -> RigidBodyState {
        self.state
    }

    pub const fn angular_acceleration(self) -> AngularRateVec {
        self.angular_acceleration
    }
}

fn coupled_moment_q16(
    inertia_difference_q12: i32,
    rate_a_q24: i32,
    rate_b_q24: i32,
    status: &mut NumericStatus,
) -> i32 {
    let rate_product_q24 = multiply_scaled(rate_a_q24, rate_b_q24, 24, status);
    multiply_scaled(inertia_difference_q12, rate_product_q24, 20, status)
}

fn axis_acceleration_q24(
    torque_mnm_q16: i32,
    coupled_moment_q16: i32,
    inertia_q12: i32,
    status: &mut NumericStatus,
) -> i32 {
    // One MN*m is one thousand kN*m, while t*m^2 * rad/s^2 is kN*m.
    let torque_knm_q16 = multiply_scaled(torque_mnm_q16, 1_000, 0, status);
    let net_q16 = subtract(torque_knm_q16, coupled_moment_q16, status);
    divide_scaled(net_q16, inertia_q12, 20, status)
}

pub fn angular_acceleration(
    inertia: DiagonalInertiaQ12,
    angular_rate: AngularRateVec,
    torque: TorqueVec,
    status: &mut NumericStatus,
) -> AngularRateVec {
    if !inertia.is_valid() {
        status.record(NumericFault::InvalidInput);
        return AngularRateVec::ZERO;
    }
    let x_coupling = coupled_moment_q16(
        subtract(inertia.z, inertia.y, status),
        angular_rate.y(),
        angular_rate.z(),
        status,
    );
    let y_coupling = coupled_moment_q16(
        subtract(inertia.x, inertia.z, status),
        angular_rate.z(),
        angular_rate.x(),
        status,
    );
    let z_coupling = coupled_moment_q16(
        subtract(inertia.y, inertia.x, status),
        angular_rate.x(),
        angular_rate.y(),
        status,
    );
    AngularRateVec::new(
        axis_acceleration_q24(torque.x(), x_coupling, inertia.x, status),
        axis_acceleration_q24(torque.y(), y_coupling, inertia.y, status),
        axis_acceleration_q24(torque.z(), z_coupling, inertia.z, status),
    )
}

fn integrate_attitude(
    attitude: QuaternionQ30,
    angular_rate: AngularRateVec,
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

    // q_dot = 0.5 * q (x) [0, omega_body]. Products are retained in Q24;
    // multiplying by Q16 time and shifting 11 yields a Q30 half-step delta.
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

/// Advances one fast-cadence step with semi-implicit angular rate and a
/// normalized first-order quaternion update. Any numeric fault returns the
/// original state and preserves the sticky fault evidence.
#[inline(always)]
pub fn step_rigid_body(
    state: RigidBodyState,
    inertia: DiagonalInertiaQ12,
    torque: TorqueVec,
    timestep_q16: i32,
    status: &mut NumericStatus,
) -> RigidBodyStep {
    if !status.is_clear() || timestep_q16 <= 0 {
        if timestep_q16 <= 0 {
            status.record(NumericFault::InvalidInput);
        }
        return RigidBodyStep {
            state,
            angular_acceleration: AngularRateVec::ZERO,
        };
    }
    let acceleration = angular_acceleration(inertia, state.angular_rate, torque, status);
    let delta_rate = acceleration.scale::<16>(timestep_q16, status);
    let next_rate = state.angular_rate.checked_add(delta_rate, status);
    let next_attitude = integrate_attitude(state.attitude, next_rate, timestep_q16, status);
    if !status.is_clear() {
        return RigidBodyStep {
            state,
            angular_acceleration: acceleration,
        };
    }
    RigidBodyStep {
        state: RigidBodyState::new(next_attitude, next_rate),
        angular_acceleration: acceleration,
    }
}
