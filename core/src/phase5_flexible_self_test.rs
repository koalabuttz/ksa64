use crate::flexible::{
    step_flexible_modes, AxisModesQ24, FlexibleDriveQ24, FlexibleParametersQ16, FlexibleStateQ24,
    ModalParametersQ16, ModalStateQ24,
};
use crate::numeric::NumericStatus;

#[allow(dead_code)]
mod vectors {
    include!("../../phase5/generated/flexible_vectors_v1.rs");
}

#[inline]
fn failure(value: bool) -> u32 {
    if value {
        0
    } else {
        1
    }
}

pub fn run_phase5_flexible_self_tests() -> u32 {
    let mut failures = 0u32;
    let initial = FlexibleStateQ24::new(
        AxisModesQ24::new(
            ModalStateQ24::new(vectors::INITIAL_Q24[0], vectors::INITIAL_Q24[1]),
            ModalStateQ24::new(vectors::INITIAL_Q24[2], vectors::INITIAL_Q24[3]),
        ),
        AxisModesQ24::new(
            ModalStateQ24::new(vectors::INITIAL_Q24[4], vectors::INITIAL_Q24[5]),
            ModalStateQ24::new(vectors::INITIAL_Q24[6], vectors::INITIAL_Q24[7]),
        ),
    );
    let parameters = FlexibleParametersQ16::new(
        ModalParametersQ16::new(
            vectors::BENDING_PARAMETERS_Q16[0],
            vectors::BENDING_PARAMETERS_Q16[1],
            vectors::BENDING_PARAMETERS_Q16[2],
        ),
        ModalParametersQ16::new(
            vectors::SLOSH_PARAMETERS_Q16[0],
            vectors::SLOSH_PARAMETERS_Q16[1],
            vectors::SLOSH_PARAMETERS_Q16[2],
        ),
    );
    let drive = FlexibleDriveQ24::new(
        vectors::DRIVE_Q24[0],
        vectors::DRIVE_Q24[1],
        vectors::DRIVE_Q24[2],
        vectors::DRIVE_Q24[3],
    );
    let mut status = NumericStatus::CLEAR;
    let result = step_flexible_modes(initial, parameters, drive, vectors::DT_Q16, &mut status);
    failures |= failure(result.y().bending().displacement() == vectors::ONE_STEP_Q24[0]);
    failures |= failure(result.y().bending().rate() == vectors::ONE_STEP_Q24[1]);
    failures |= failure(result.y().slosh().displacement() == vectors::ONE_STEP_Q24[2]);
    failures |= failure(result.y().slosh().rate() == vectors::ONE_STEP_Q24[3]);
    failures |= failure(result.z().bending().displacement() == vectors::ONE_STEP_Q24[4]);
    failures |= failure(result.z().bending().rate() == vectors::ONE_STEP_Q24[5]);
    failures |= failure(result.z().slosh().displacement() == vectors::ONE_STEP_Q24[6]);
    failures |= failure(result.z().slosh().rate() == vectors::ONE_STEP_Q24[7]);
    failures | failure(status.is_clear())
}
