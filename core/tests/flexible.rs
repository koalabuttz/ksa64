use core::mem::size_of;

use ksa64_core::flexible::{
    step_flexible_modes, AxisModesQ24, FlexibleDriveQ24, FlexibleParametersQ16, FlexibleStateQ24,
    ModalParametersQ16, ModalStateQ24,
};
use ksa64_core::numeric::{NumericFault, NumericStatus};

mod vectors {
    include!("../../phase5/generated/flexible_vectors_v1.rs");
}

fn parameters(bending: [i32; 3]) -> FlexibleParametersQ16 {
    FlexibleParametersQ16::new(
        ModalParametersQ16::new(bending[0], bending[1], bending[2]),
        ModalParametersQ16::new(
            vectors::SLOSH_PARAMETERS_Q16[0],
            vectors::SLOSH_PARAMETERS_Q16[1],
            vectors::SLOSH_PARAMETERS_Q16[2],
        ),
    )
}
fn state(raw: [i32; 8]) -> FlexibleStateQ24 {
    FlexibleStateQ24::new(
        AxisModesQ24::new(
            ModalStateQ24::new(raw[0], raw[1]),
            ModalStateQ24::new(raw[2], raw[3]),
        ),
        AxisModesQ24::new(
            ModalStateQ24::new(raw[4], raw[5]),
            ModalStateQ24::new(raw[6], raw[7]),
        ),
    )
}
fn raw(value: FlexibleStateQ24) -> [i32; 8] {
    [
        value.y().bending().displacement(),
        value.y().bending().rate(),
        value.y().slosh().displacement(),
        value.y().slosh().rate(),
        value.z().bending().displacement(),
        value.z().bending().rate(),
        value.z().slosh().displacement(),
        value.z().slosh().rate(),
    ]
}
fn drive(raw: [i32; 4]) -> FlexibleDriveQ24 {
    FlexibleDriveQ24::new(raw[0], raw[1], raw[2], raw[3])
}

#[test]
fn state_is_four_bounded_second_order_modes() {
    assert_eq!(size_of::<ModalStateQ24>(), 8);
    assert_eq!(size_of::<FlexibleStateQ24>(), 32);
}

#[test]
fn one_step_matches_independent_integer_oracle() {
    let mut status = NumericStatus::CLEAR;
    let result = step_flexible_modes(
        state(vectors::INITIAL_Q24),
        parameters(vectors::BENDING_PARAMETERS_Q16),
        drive(vectors::DRIVE_Q24),
        vectors::DT_Q16,
        &mut status,
    );
    assert_eq!(raw(result), vectors::ONE_STEP_Q24);
    assert!(status.is_clear());
}

#[test]
fn long_damped_undamped_and_driven_cases_match_oracle() {
    let initial = state(vectors::INITIAL_Q24);
    let mut damped = initial;
    let mut undamped = initial;
    let mut driven = initial;
    let mut status = NumericStatus::CLEAR;
    for _ in 0..128 {
        damped = step_flexible_modes(
            damped,
            parameters(vectors::BENDING_PARAMETERS_Q16),
            FlexibleDriveQ24::ZERO,
            vectors::DT_Q16,
            &mut status,
        );
        undamped = step_flexible_modes(
            undamped,
            parameters(vectors::BENDING_UNDAMPED_Q16),
            FlexibleDriveQ24::ZERO,
            vectors::DT_Q16,
            &mut status,
        );
        driven = step_flexible_modes(
            driven,
            parameters(vectors::BENDING_PARAMETERS_Q16),
            drive(vectors::DRIVE_Q24),
            vectors::DT_Q16,
            &mut status,
        );
    }
    assert_eq!(raw(damped), vectors::DAMPED_ZERO_DRIVE_128_Q24);
    assert_eq!(raw(undamped), vectors::UNDAMPED_ZERO_DRIVE_128_Q24);
    assert_eq!(raw(driven), vectors::DRIVEN_128_Q24);
    assert!(
        damped.y().bending().displacement().abs() < undamped.y().bending().displacement().abs()
    );
    assert!(status.is_clear());
}

#[test]
fn identical_transverse_drives_remain_axis_symmetric() {
    let mut value = FlexibleStateQ24::ZERO;
    let mut status = NumericStatus::CLEAR;
    for _ in 0..64 {
        value = step_flexible_modes(
            value,
            parameters(vectors::BENDING_PARAMETERS_Q16),
            drive(vectors::SYMMETRIC_DRIVE_Q24),
            vectors::DT_Q16,
            &mut status,
        );
    }
    assert_eq!(raw(value), vectors::SYMMETRIC_64_Q24);
    assert_eq!(value.y(), value.z());
    assert!(status.is_clear());
}

#[test]
fn invalid_parameters_and_existing_fault_preserve_all_modes() {
    let initial = state(vectors::INITIAL_Q24);
    let invalid = FlexibleParametersQ16::new(
        ModalParametersQ16::new(0, 0, 0),
        ModalParametersQ16::new(1, 0, 0),
    );
    let mut status = NumericStatus::CLEAR;
    assert_eq!(
        step_flexible_modes(
            initial,
            invalid,
            FlexibleDriveQ24::ZERO,
            vectors::DT_Q16,
            &mut status
        ),
        initial
    );
    assert!(status.contains(NumericFault::InvalidInput));
    assert_eq!(
        step_flexible_modes(
            initial,
            parameters(vectors::BENDING_PARAMETERS_Q16),
            FlexibleDriveQ24::ZERO,
            vectors::DT_Q16,
            &mut status,
        ),
        initial
    );
}
