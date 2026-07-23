//! Phase 2 Mach-dependent point-mass aerodynamics.

use crate::numeric::{
    add, divide_scaled, interpolate_clamped, multiply_scaled, subtract, NumericFault, NumericStatus,
};
use crate::phase2_numeric::{sqrt_floor_u32, EARTH_ROTATION_RAD_Q30};
use crate::phase2_quantities::{Coefficient, DynamicPressure, Mach, PlanarVelocity, ReferenceArea};
use crate::planar::{evaluate_vacuum, PlanarTruthState, PlanarWorld};
use crate::planar_environment::PlanarEnvironmentSample;
use crate::quantities::Force;

#[derive(Clone, Copy, Debug)]
pub struct AeroTable<'a> {
    mach_q16: &'a [i32],
    cd_q14: &'a [i32],
}

impl<'a> AeroTable<'a> {
    pub const fn new(mach_q16: &'a [i32], cd_q14: &'a [i32]) -> Self {
        Self { mach_q16, cd_q14 }
    }
    pub fn is_valid(self) -> bool {
        if self.mach_q16.len() < 2
            || self.mach_q16.len() > 16
            || self.mach_q16.len() != self.cd_q14.len()
        {
            return false;
        }
        let mut index = 0;
        while index < self.mach_q16.len() {
            if self.mach_q16[index] < 0
                || self.cd_q14[index] < 0
                || (index != 0 && self.mach_q16[index] <= self.mach_q16[index - 1])
            {
                return false;
            }
            index += 1;
        }
        true
    }
    pub fn coefficient(self, mach: Mach, status: &mut NumericStatus) -> Coefficient {
        Coefficient::from_raw(interpolate_clamped(
            mach.raw(),
            self.mach_q16,
            self.cd_q14,
            status,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AeroConfig<'a> {
    area: ReferenceArea,
    table: AeroTable<'a>,
}

impl<'a> AeroConfig<'a> {
    pub const fn new(area: ReferenceArea, table: AeroTable<'a>) -> Self {
        Self { area, table }
    }
    pub const fn area(self) -> ReferenceArea {
        self.area
    }
    pub const fn table(self) -> AeroTable<'a> {
        self.table
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AeroSnapshot {
    air_radial_velocity: PlanarVelocity,
    air_tangential_velocity: PlanarVelocity,
    air_speed: PlanarVelocity,
    mach: Mach,
    dynamic_pressure: DynamicPressure,
    coefficient: Coefficient,
    radial_drag: Force,
    tangential_drag: Force,
}

impl AeroSnapshot {
    pub const fn air_radial_velocity(self) -> PlanarVelocity {
        self.air_radial_velocity
    }
    pub const fn air_tangential_velocity(self) -> PlanarVelocity {
        self.air_tangential_velocity
    }
    pub const fn air_speed(self) -> PlanarVelocity {
        self.air_speed
    }
    pub const fn mach(self) -> Mach {
        self.mach
    }
    pub const fn dynamic_pressure(self) -> DynamicPressure {
        self.dynamic_pressure
    }
    pub const fn coefficient(self) -> Coefficient {
        self.coefficient
    }
    pub const fn radial_drag(self) -> Force {
        self.radial_drag
    }
    pub const fn tangential_drag(self) -> Force {
        self.tangential_drag
    }
}

fn signed_drag(
    component_q24: i32,
    speed_q12: i32,
    density_q28: i32,
    cd_q14: i32,
    area_q16: i32,
    status: &mut NumericStatus,
) -> i32 {
    let speed_component_q20 = multiply_scaled(speed_q12, component_q24, 16, status);
    let density_speed_q20 = multiply_scaled(density_q28, speed_component_q20, 28, status);
    let with_cd_q20 = multiply_scaled(density_speed_q20, cd_q14, 14, status);
    let twice_drag_q12 = multiply_scaled(with_cd_q20, area_q16, 24, status);
    let magnitude = (twice_drag_q12.abs() >> 1) + (twice_drag_q12.abs() & 1);
    if component_q24 > 0 {
        -magnitude
    } else if component_q24 < 0 {
        magnitude
    } else {
        0
    }
}

pub fn evaluate_aerodynamics(
    world: PlanarWorld,
    truth: PlanarTruthState,
    environment: PlanarEnvironmentSample,
    config: AeroConfig<'_>,
    status: &mut NumericStatus,
) -> AeroSnapshot {
    if !config.table().is_valid() || config.area().raw() < 0 {
        status.record(NumericFault::InvalidInput);
    }
    let vacuum = evaluate_vacuum(world, truth, status);
    let atmosphere_tangential =
        multiply_scaled(EARTH_ROTATION_RAD_Q30, truth.radius().raw(), 18, status);
    let air_radial = truth.radial_velocity().raw();
    let air_tangential = subtract(
        vacuum.tangential_velocity().raw(),
        atmosphere_tangential,
        status,
    );
    let radial2_q20 = multiply_scaled(air_radial, air_radial, 28, status);
    let tangential2_q20 = multiply_scaled(air_tangential, air_tangential, 28, status);
    let speed2_q20 = add(radial2_q20, tangential2_q20, status);
    let speed_q12 = sqrt_floor_u32((speed2_q20.max(0) as u32) << 4) as i32;
    let speed_q24 = speed_q12 << 12;
    let mach_q16 = divide_scaled(speed_q24, environment.sound_speed().raw(), 16, status);
    let mach = Mach::from_raw(mach_q16);
    let cd = config.table().coefficient(mach, status);
    let density_speed_q17 = multiply_scaled(environment.density().raw(), speed2_q20, 31, status);
    let dynamic_pressure_q16 = multiply_scaled(density_speed_q17, 128_000, 9, status);
    let radial_drag = signed_drag(
        air_radial,
        speed_q12,
        environment.density().raw(),
        cd.raw(),
        config.area().raw(),
        status,
    );
    let tangential_drag = signed_drag(
        air_tangential,
        speed_q12,
        environment.density().raw(),
        cd.raw(),
        config.area().raw(),
        status,
    );
    AeroSnapshot {
        air_radial_velocity: PlanarVelocity::from_raw(air_radial),
        air_tangential_velocity: PlanarVelocity::from_raw(air_tangential),
        air_speed: PlanarVelocity::from_raw(speed_q24),
        mach,
        dynamic_pressure: DynamicPressure::from_raw(dynamic_pressure_q16),
        coefficient: cd,
        radial_drag: Force::from_raw(radial_drag),
        tangential_drag: Force::from_raw(tangential_drag),
    }
}
