use ksa64_core::numeric::{NumericFault, NumericStatus};
use ksa64_core::rigid_body::{step_rigid_body, DiagonalInertiaQ12, RigidBodyState};
use ksa64_core::spatial_numeric::{AngularRateVec, QuaternionQ30, TorqueVec};

mod vectors {
    include!("../../phase5/generated/rigid_body_vectors_v1.rs");
}

fn inertia(raw: [i32; 3]) -> DiagonalInertiaQ12 {
    DiagonalInertiaQ12::new(raw[0], raw[1], raw[2])
}
fn vector(raw: [i32; 3]) -> AngularRateVec {
    AngularRateVec::new(raw[0], raw[1], raw[2])
}
fn torque(raw: [i32; 3]) -> TorqueVec {
    TorqueVec::new(raw[0], raw[1], raw[2])
}
fn quaternion(raw: [i32; 4]) -> QuaternionQ30 {
    QuaternionQ30::new(raw[0], raw[1], raw[2], raw[3])
}
fn state(attitude: [i32; 4], rate: [i32; 3]) -> RigidBodyState {
    RigidBodyState::new(quaternion(attitude), vector(rate))
}
fn raw(state: RigidBodyState) -> ([i32; 4], [i32; 3]) {
    let q = state.attitude();
    let w = state.angular_rate();
    ([q.w(), q.x(), q.y(), q.z()], [w.x(), w.y(), w.z()])
}

#[test]
fn spherical_constant_torque_step_matches_independent_integer_oracle() {
    let mut status = NumericStatus::CLEAR;
    let result = step_rigid_body(
        RigidBodyState::REST,
        inertia(vectors::SPHERICAL_INERTIA_Q12),
        torque(vectors::TORQUE_X_Q16),
        vectors::DT_Q16,
        &mut status,
    );
    assert_eq!(
        raw(result.state()),
        (
            vectors::SPHERICAL_ONE_STEP_ATTITUDE_Q30,
            vectors::SPHERICAL_ONE_STEP_RATE_Q24
        )
    );
    let alpha = result.angular_acceleration();
    assert_eq!(
        [alpha.x(), alpha.y(), alpha.z()],
        vectors::SPHERICAL_ALPHA_Q24
    );
    assert!(status.is_clear());
}

#[test]
fn asymmetric_torque_free_step_includes_euler_coupling() {
    let mut status = NumericStatus::CLEAR;
    let result = step_rigid_body(
        state(
            vectors::ASYMMETRIC_INITIAL_ATTITUDE_Q30,
            vectors::ASYMMETRIC_INITIAL_RATE_Q24,
        ),
        inertia(vectors::ASYMMETRIC_INERTIA_Q12),
        TorqueVec::ZERO,
        vectors::DT_Q16,
        &mut status,
    );
    assert_eq!(
        raw(result.state()),
        (
            vectors::ASYMMETRIC_ONE_STEP_ATTITUDE_Q30,
            vectors::ASYMMETRIC_ONE_STEP_RATE_Q24
        )
    );
    let alpha = result.angular_acceleration();
    assert_eq!(
        [alpha.x(), alpha.y(), alpha.z()],
        vectors::ASYMMETRIC_ALPHA_Q24
    );
    assert_ne!(vectors::ASYMMETRIC_ALPHA_Q24, [0, 0, 0]);
    assert!(status.is_clear());
}

#[test]
fn sixty_four_step_cases_match_exact_oracle_and_float64_special_cases() {
    let spherical = inertia(vectors::SPHERICAL_INERTIA_Q12);
    let mut torque_state = RigidBodyState::REST;
    let mut rate_state = state([QuaternionQ30::ONE, 0, 0, 0], [0, 0, 1 << 22]);
    let mut status = NumericStatus::CLEAR;
    for _ in 0..64 {
        torque_state = step_rigid_body(
            torque_state,
            spherical,
            torque(vectors::TORQUE_X_Q16),
            vectors::DT_Q16,
            &mut status,
        )
        .state();
        rate_state = step_rigid_body(
            rate_state,
            spherical,
            TorqueVec::ZERO,
            vectors::DT_Q16,
            &mut status,
        )
        .state();
    }
    assert_eq!(
        raw(torque_state),
        (
            vectors::CONSTANT_TORQUE_64_ATTITUDE_Q30,
            vectors::CONSTANT_TORQUE_64_RATE_Q24
        )
    );
    assert_eq!(
        raw(rate_state),
        (
            vectors::CONSTANT_RATE_64_ATTITUDE_Q30,
            vectors::CONSTANT_RATE_64_RATE_Q24
        )
    );
    let torque_angle = 2.0 * (torque_state.attitude().x() as f64 / (1u64 << 30) as f64).asin();
    let rate_angle = 2.0 * (rate_state.attitude().z() as f64 / (1u64 << 30) as f64).asin();
    assert!((torque_state.angular_rate().x() as f64 / (1u64 << 24) as f64 - 1.0).abs() < 1e-6);
    assert!((torque_angle - 1.0).abs() < 0.025);
    assert!((rate_angle - 0.5).abs() < 0.005);
    assert!(status.is_clear());
}

#[test]
fn invalid_configuration_and_preexisting_fault_preserve_state() {
    let original = state(
        vectors::ASYMMETRIC_INITIAL_ATTITUDE_Q30,
        vectors::ASYMMETRIC_INITIAL_RATE_Q24,
    );
    let mut status = NumericStatus::CLEAR;
    let result = step_rigid_body(
        original,
        DiagonalInertiaQ12::new(0, 1, 1),
        TorqueVec::ZERO,
        vectors::DT_Q16,
        &mut status,
    );
    assert_eq!(result.state(), original);
    assert!(status.contains(NumericFault::InvalidInput));

    let mut status = NumericStatus::from_bits(NumericFault::Saturation as u8);
    let result = step_rigid_body(
        original,
        inertia(vectors::SPHERICAL_INERTIA_Q12),
        torque(vectors::TORQUE_X_Q16),
        vectors::DT_Q16,
        &mut status,
    );
    assert_eq!(result.state(), original);
}
