//! Bounded transverse bending and active-stage slosh modes for Phase 5.

use crate::numeric::{add, multiply_scaled, subtract, NumericFault, NumericStatus};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ModalStateQ24 {
    displacement: i32,
    rate: i32,
}

impl ModalStateQ24 {
    pub const ZERO: Self = Self::new(0, 0);

    pub const fn new(displacement: i32, rate: i32) -> Self {
        Self { displacement, rate }
    }
    pub const fn displacement(self) -> i32 {
        self.displacement
    }
    pub const fn rate(self) -> i32 {
        self.rate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ModalParametersQ16 {
    natural_frequency: i32,
    damping_ratio: i32,
    drive_gain: i32,
}

impl ModalParametersQ16 {
    pub const fn new(natural_frequency: i32, damping_ratio: i32, drive_gain: i32) -> Self {
        Self {
            natural_frequency,
            damping_ratio,
            drive_gain,
        }
    }
    pub const fn natural_frequency(self) -> i32 {
        self.natural_frequency
    }
    pub const fn damping_ratio(self) -> i32 {
        self.damping_ratio
    }
    pub const fn drive_gain(self) -> i32 {
        self.drive_gain
    }
    pub const fn is_valid(self) -> bool {
        self.natural_frequency > 0
            && self.damping_ratio >= 0
            && self.damping_ratio <= 65_536
            && self.drive_gain >= -1_048_576
            && self.drive_gain <= 1_048_576
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct AxisModesQ24 {
    bending: ModalStateQ24,
    slosh: ModalStateQ24,
}

impl AxisModesQ24 {
    pub const ZERO: Self = Self::new(ModalStateQ24::ZERO, ModalStateQ24::ZERO);

    pub const fn new(bending: ModalStateQ24, slosh: ModalStateQ24) -> Self {
        Self { bending, slosh }
    }
    pub const fn bending(self) -> ModalStateQ24 {
        self.bending
    }
    pub const fn slosh(self) -> ModalStateQ24 {
        self.slosh
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FlexibleStateQ24 {
    y: AxisModesQ24,
    z: AxisModesQ24,
}

impl FlexibleStateQ24 {
    pub const ZERO: Self = Self::new(AxisModesQ24::ZERO, AxisModesQ24::ZERO);

    pub const fn new(y: AxisModesQ24, z: AxisModesQ24) -> Self {
        Self { y, z }
    }
    pub const fn y(self) -> AxisModesQ24 {
        self.y
    }
    pub const fn z(self) -> AxisModesQ24 {
        self.z
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct FlexibleParametersQ16 {
    bending: ModalParametersQ16,
    slosh: ModalParametersQ16,
}

impl FlexibleParametersQ16 {
    pub const fn new(bending: ModalParametersQ16, slosh: ModalParametersQ16) -> Self {
        Self { bending, slosh }
    }
    pub const fn bending(self) -> ModalParametersQ16 {
        self.bending
    }
    pub const fn slosh(self) -> ModalParametersQ16 {
        self.slosh
    }
    pub const fn is_valid(self) -> bool {
        self.bending.is_valid() && self.slosh.is_valid()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FlexibleDriveQ24 {
    bending_y: i32,
    bending_z: i32,
    slosh_y: i32,
    slosh_z: i32,
}

impl FlexibleDriveQ24 {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);

    pub const fn new(bending_y: i32, bending_z: i32, slosh_y: i32, slosh_z: i32) -> Self {
        Self {
            bending_y,
            bending_z,
            slosh_y,
            slosh_z,
        }
    }
}

fn modal_acceleration_q24(
    state: ModalStateQ24,
    parameters: ModalParametersQ16,
    drive_q24: i32,
    status: &mut NumericStatus,
) -> i32 {
    let omega_squared_q16 = multiply_scaled(
        parameters.natural_frequency,
        parameters.natural_frequency,
        16,
        status,
    );
    let damping_q16 = multiply_scaled(
        parameters.damping_ratio,
        parameters.natural_frequency,
        15,
        status,
    );
    let driven = multiply_scaled(parameters.drive_gain, drive_q24, 16, status);
    let spring = multiply_scaled(omega_squared_q16, state.displacement, 16, status);
    let damping = multiply_scaled(damping_q16, state.rate, 16, status);
    subtract(subtract(driven, spring, status), damping, status)
}

fn step_mode(
    state: ModalStateQ24,
    parameters: ModalParametersQ16,
    drive_q24: i32,
    timestep_q16: i32,
    status: &mut NumericStatus,
) -> ModalStateQ24 {
    let acceleration = modal_acceleration_q24(state, parameters, drive_q24, status);
    let rate = add(
        state.rate,
        multiply_scaled(acceleration, timestep_q16, 16, status),
        status,
    );
    let displacement = add(
        state.displacement,
        multiply_scaled(rate, timestep_q16, 16, status),
        status,
    );
    ModalStateQ24::new(displacement, rate)
}

/// Advances all four transverse modes. Any bad parameter, timestep, or numeric
/// result preserves the complete previous modal state.
pub fn step_flexible_modes(
    state: FlexibleStateQ24,
    parameters: FlexibleParametersQ16,
    drive: FlexibleDriveQ24,
    timestep_q16: i32,
    status: &mut NumericStatus,
) -> FlexibleStateQ24 {
    if !status.is_clear() || !parameters.is_valid() || timestep_q16 <= 0 {
        if !parameters.is_valid() || timestep_q16 <= 0 {
            status.record(NumericFault::InvalidInput);
        }
        return state;
    }
    let y = AxisModesQ24::new(
        step_mode(
            state.y.bending,
            parameters.bending,
            drive.bending_y,
            timestep_q16,
            status,
        ),
        step_mode(
            state.y.slosh,
            parameters.slosh,
            drive.slosh_y,
            timestep_q16,
            status,
        ),
    );
    let z = AxisModesQ24::new(
        step_mode(
            state.z.bending,
            parameters.bending,
            drive.bending_z,
            timestep_q16,
            status,
        ),
        step_mode(
            state.z.slosh,
            parameters.slosh,
            drive.slosh_z,
            timestep_q16,
            status,
        ),
    );
    if status.is_clear() {
        FlexibleStateQ24::new(y, z)
    } else {
        state
    }
}
