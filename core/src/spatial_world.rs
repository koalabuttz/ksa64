//! Spherical rotating-Earth 3-D translation, aerodynamics, and orbit analysis.

use crate::numeric::{
    add, divide_scaled, magnitude3_floor, multiply_scaled, subtract, NumericFault, NumericStatus,
};
use crate::phase2_numeric::{EARTH_MU_Q12, EARTH_RADIUS_Q12, EARTH_ROTATION_RAD_Q30};
use crate::phase2_quantities::{DynamicPressure, Eccentricity, Mach, Radius, SpecificEnergy};
use crate::planar::OrbitClass;
use crate::planar_environment::RotatingEarthEnvironment;
use crate::spatial_numeric::{
    cross_mixed_scaled, AccelerationVec, FixedVec3, ForceVec, PositionVec, QuaternionQ30,
    TorqueVec, VelocityVec,
};

const MIN_RADIUS_Q12: i32 = 26_116_096;
const MAX_RADIUS_Q12: i32 = 34_320_384;
const ATMOSPHERE_TOP_Q12: i32 = 491_520;

#[allow(dead_code)]
mod data {
    include!("../../phase5/generated/spatial_world_tables_v1.rs");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpatialState {
    position: PositionVec,
    velocity: VelocityVec,
    acceleration: AccelerationVec,
}

impl SpatialState {
    pub const fn new(position: PositionVec, velocity: VelocityVec) -> Self {
        Self {
            position,
            velocity,
            acceleration: AccelerationVec::ZERO,
        }
    }
    pub const fn position(self) -> PositionVec {
        self.position
    }
    pub const fn velocity(self) -> VelocityVec {
        self.velocity
    }
    pub const fn acceleration(self) -> AccelerationVec {
        self.acceleration
    }
    const fn successor(
        self,
        position: PositionVec,
        velocity: VelocityVec,
        acceleration: AccelerationVec,
    ) -> Self {
        Self {
            position,
            velocity,
            acceleration,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialEnvironmentSnapshot {
    radius_q12: i32,
    altitude_q12: i32,
    gravity: AccelerationVec,
    atmosphere_velocity: VelocityVec,
    air_velocity: VelocityVec,
    air_speed_q24: i32,
    density_q28: i32,
    sound_speed_q24: i32,
    dynamic_pressure: DynamicPressure,
}

impl SpatialEnvironmentSnapshot {
    pub const fn radius_q12(self) -> i32 {
        self.radius_q12
    }
    pub const fn altitude_q12(self) -> i32 {
        self.altitude_q12
    }
    pub const fn gravity(self) -> AccelerationVec {
        self.gravity
    }
    pub const fn atmosphere_velocity(self) -> VelocityVec {
        self.atmosphere_velocity
    }
    pub const fn air_velocity(self) -> VelocityVec {
        self.air_velocity
    }
    pub const fn air_speed_q24(self) -> i32 {
        self.air_speed_q24
    }
    pub const fn density_q28(self) -> i32 {
        self.density_q28
    }
    pub const fn sound_speed_q24(self) -> i32 {
        self.sound_speed_q24
    }
    pub const fn dynamic_pressure(self) -> DynamicPressure {
        self.dynamic_pressure
    }
}

pub fn position_radius_q12(position: PositionVec, status: &mut NumericStatus) -> i32 {
    let radius = magnitude3_floor(position.x(), position.y(), position.z(), status);
    if radius > i32::MAX as u32 {
        status.record(NumericFault::Saturation);
        i32::MAX
    } else {
        radius as i32
    }
}

fn vector_speed_q24(velocity: VelocityVec, status: &mut NumericStatus) -> i32 {
    let speed = magnitude3_floor(velocity.x(), velocity.y(), velocity.z(), status);
    if speed > i32::MAX as u32 {
        status.record(NumericFault::Saturation);
        i32::MAX
    } else {
        speed as i32
    }
}

fn gravity_at(
    position: PositionVec,
    radius_q12: i32,
    status: &mut NumericStatus,
) -> AccelerationVec {
    let mu_over_r_q24 = divide_scaled(EARTH_MU_Q12, radius_q12, 24, status);
    let magnitude_q28 = divide_scaled(mu_over_r_q24, radius_q12, 16, status);
    let unit = FixedVec3::<30>::new(
        divide_scaled(position.x(), radius_q12, 30, status),
        divide_scaled(position.y(), radius_q12, 30, status),
        divide_scaled(position.z(), radius_q12, 30, status),
    );
    AccelerationVec::new(
        subtract(
            0,
            multiply_scaled(unit.x(), magnitude_q28, 30, status),
            status,
        ),
        subtract(
            0,
            multiply_scaled(unit.y(), magnitude_q28, 30, status),
            status,
        ),
        subtract(
            0,
            multiply_scaled(unit.z(), magnitude_q28, 30, status),
            status,
        ),
    )
}

pub fn evaluate_spatial_environment(
    state: SpatialState,
    status: &mut NumericStatus,
) -> SpatialEnvironmentSnapshot {
    let radius_q12 = position_radius_q12(state.position, status);
    if !(MIN_RADIUS_Q12..=MAX_RADIUS_Q12).contains(&radius_q12) {
        status.record(NumericFault::InvalidInput);
    }
    let gravity = gravity_at(state.position, radius_q12, status);
    let earth_rate = FixedVec3::<30>::new(0, 0, EARTH_ROTATION_RAD_Q30);
    let atmosphere_velocity = cross_mixed_scaled::<30, 12, 24>(earth_rate, state.position, status);
    let air_velocity = state.velocity.checked_sub(atmosphere_velocity, status);
    let air_speed_q24 = vector_speed_q24(air_velocity, status);
    let sample = RotatingEarthEnvironment::new().sample(Radius::from_raw(radius_q12), status);
    let radial2_q20 = multiply_scaled(air_velocity.x(), air_velocity.x(), 28, status);
    let lateral2_q20 = add(
        multiply_scaled(air_velocity.y(), air_velocity.y(), 28, status),
        multiply_scaled(air_velocity.z(), air_velocity.z(), 28, status),
        status,
    );
    let speed2_q20 = add(radial2_q20, lateral2_q20, status);
    let density_speed_q17 = multiply_scaled(sample.density().raw(), speed2_q20, 31, status);
    let dynamic_pressure_q16 = multiply_scaled(density_speed_q17, 128_000, 9, status);
    SpatialEnvironmentSnapshot {
        radius_q12,
        altitude_q12: subtract(radius_q12, EARTH_RADIUS_Q12, status),
        gravity,
        atmosphere_velocity,
        air_velocity,
        air_speed_q24,
        density_q28: sample.density().raw(),
        sound_speed_q24: sample.sound_speed().raw(),
        dynamic_pressure: DynamicPressure::from_raw(dynamic_pressure_q16),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialAeroConfig {
    area_q16: i32,
    cd_q14: i32,
    normal_slope_q14: i32,
    center_of_pressure_aft_q16: i32,
}

impl SpatialAeroConfig {
    pub const fn new(area_q16: i32, cd_q14: i32, normal_slope_q14: i32, cp_aft_q16: i32) -> Self {
        Self {
            area_q16,
            cd_q14,
            normal_slope_q14,
            center_of_pressure_aft_q16: cp_aft_q16,
        }
    }
    pub const fn is_valid(self) -> bool {
        self.area_q16 >= 0
            && self.cd_q14 >= 0
            && self.normal_slope_q14 >= 0
            && self.center_of_pressure_aft_q16 >= 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialAeroSnapshot {
    force_eci: ForceVec,
    torque_body: TorqueVec,
    mach: Mach,
    dynamic_pressure: DynamicPressure,
    angle_of_attack_sine_q16: i32,
}

impl SpatialAeroSnapshot {
    pub const fn force_eci(self) -> ForceVec {
        self.force_eci
    }
    pub const fn torque_body(self) -> TorqueVec {
        self.torque_body
    }
    pub const fn mach(self) -> Mach {
        self.mach
    }
    pub const fn dynamic_pressure(self) -> DynamicPressure {
        self.dynamic_pressure
    }
    pub const fn angle_of_attack_sine_q16(self) -> i32 {
        self.angle_of_attack_sine_q16
    }
}

fn signed_drag_component(
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

pub fn evaluate_spatial_aerodynamics(
    attitude: QuaternionQ30,
    environment: SpatialEnvironmentSnapshot,
    config: SpatialAeroConfig,
    status: &mut NumericStatus,
) -> SpatialAeroSnapshot {
    if !config.is_valid() {
        status.record(NumericFault::InvalidInput);
    }
    let speed_q12 = environment.air_speed_q24 >> 12;
    let drag = ForceVec::new(
        signed_drag_component(
            environment.air_velocity.x(),
            speed_q12,
            environment.density_q28,
            config.cd_q14,
            config.area_q16,
            status,
        ),
        signed_drag_component(
            environment.air_velocity.y(),
            speed_q12,
            environment.density_q28,
            config.cd_q14,
            config.area_q16,
            status,
        ),
        signed_drag_component(
            environment.air_velocity.z(),
            speed_q12,
            environment.density_q28,
            config.cd_q14,
            config.area_q16,
            status,
        ),
    );
    let body_air = attitude
        .conjugate()
        .rotate(environment.air_velocity, status);
    let lateral_speed =
        magnitude3_floor(0, body_air.y(), body_air.z(), status).min(i32::MAX as u32) as i32;
    let aoa_sine_q16 = if environment.air_speed_q24 == 0 {
        0
    } else {
        divide_scaled(lateral_speed, environment.air_speed_q24, 16, status)
    };
    let q_area_q12 = multiply_scaled(
        environment.dynamic_pressure.raw(),
        config.area_q16,
        20,
        status,
    );
    let normal_per_alpha_q12 = multiply_scaled(q_area_q12, config.normal_slope_q14, 14, status);
    let alpha_y_q16 = if environment.air_speed_q24 == 0 {
        0
    } else {
        divide_scaled(body_air.y(), environment.air_speed_q24, 16, status)
    };
    let alpha_z_q16 = if environment.air_speed_q24 == 0 {
        0
    } else {
        divide_scaled(body_air.z(), environment.air_speed_q24, 16, status)
    };
    let normal_y_kn_q12 = subtract(
        0,
        multiply_scaled(normal_per_alpha_q12, alpha_y_q16, 16, status),
        status,
    );
    let normal_z_kn_q12 = subtract(
        0,
        multiply_scaled(normal_per_alpha_q12, alpha_z_q16, 16, status),
        status,
    );
    let normal_body = ForceVec::new(
        0,
        divide_scaled(normal_y_kn_q12, 1_000, 0, status),
        divide_scaled(normal_z_kn_q12, 1_000, 0, status),
    );
    let normal_eci = attitude.rotate(normal_body, status);
    let force_eci = drag.checked_add(normal_eci, status);
    let torque_y_q16 = multiply_scaled(
        config.center_of_pressure_aft_q16,
        normal_body.z(),
        12,
        status,
    );
    let torque_z_q16 = subtract(
        0,
        multiply_scaled(
            config.center_of_pressure_aft_q16,
            normal_body.y(),
            12,
            status,
        ),
        status,
    );
    let torque_body = TorqueVec::new(0, torque_y_q16, torque_z_q16);
    let mach_q16 = if environment.sound_speed_q24 == 0 {
        0
    } else {
        divide_scaled(
            environment.air_speed_q24,
            environment.sound_speed_q24,
            16,
            status,
        )
    };
    SpatialAeroSnapshot {
        force_eci,
        torque_body,
        mach: Mach::from_raw(mach_q16),
        dynamic_pressure: environment.dynamic_pressure,
        angle_of_attack_sine_q16: aoa_sine_q16,
    }
}

#[inline(always)]
pub fn advance_spatial_state(
    state: SpatialState,
    non_gravity_force_eci: ForceVec,
    mass_q12: i32,
    timestep_q16: i32,
    status: &mut NumericStatus,
) -> SpatialState {
    if !status.is_clear() || mass_q12 <= 0 || timestep_q16 <= 0 {
        if mass_q12 <= 0 || timestep_q16 <= 0 {
            status.record(NumericFault::InvalidInput);
        }
        return state;
    }
    let environment = evaluate_spatial_environment(state, status);
    // The inherited force unit is MN: 1 MN / 1 tonne = 1 km/s^2.
    let non_gravity = AccelerationVec::new(
        divide_scaled(non_gravity_force_eci.x(), mass_q12, 28, status),
        divide_scaled(non_gravity_force_eci.y(), mass_q12, 28, status),
        divide_scaled(non_gravity_force_eci.z(), mass_q12, 28, status),
    );
    let acceleration = environment.gravity.checked_add(non_gravity, status);
    let velocity = state.velocity.checked_add(
        VelocityVec::new(
            multiply_scaled(acceleration.x(), timestep_q16, 20, status),
            multiply_scaled(acceleration.y(), timestep_q16, 20, status),
            multiply_scaled(acceleration.z(), timestep_q16, 20, status),
        ),
        status,
    );
    let position = state.position.checked_add(
        PositionVec::new(
            multiply_scaled(velocity.x(), timestep_q16, 28, status),
            multiply_scaled(velocity.y(), timestep_q16, 28, status),
            multiply_scaled(velocity.z(), timestep_q16, 28, status),
        ),
        status,
    );
    if status.is_clear() {
        state.successor(position, velocity, acceleration)
    } else {
        state
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialOrbitSolution {
    class: OrbitClass,
    specific_energy: SpecificEnergy,
    eccentricity: Eccentricity,
    perigee: Radius,
    apogee: Radius,
    inclination_turn16: u16,
}

impl SpatialOrbitSolution {
    pub const fn class(self) -> OrbitClass {
        self.class
    }
    pub const fn specific_energy(self) -> SpecificEnergy {
        self.specific_energy
    }
    pub const fn eccentricity(self) -> Eccentricity {
        self.eccentricity
    }
    pub const fn perigee(self) -> Radius {
        self.perigee
    }
    pub const fn apogee(self) -> Radius {
        self.apogee
    }
    pub const fn inclination_turn16(self) -> u16 {
        self.inclination_turn16
    }
}

fn acos_turn16(cosine_q30: i32) -> u16 {
    let negative = cosine_q30 < 0;
    let magnitude = cosine_q30.unsigned_abs().min(1 << 30);
    let index = (magnitude >> 22) as usize;
    let angle = if index >= 256 {
        data::ACOS_TURN16[256] as i32
    } else {
        let fraction = (magnitude & ((1 << 22) - 1)) as i32;
        let left = data::ACOS_TURN16[index] as i32;
        let right = data::ACOS_TURN16[index + 1] as i32;
        left + (((right - left) * fraction + (1 << 21)) >> 22)
    };
    if negative {
        (32_768 - angle) as u16
    } else {
        angle as u16
    }
}

pub fn classify_spatial_orbit(
    state: SpatialState,
    status: &mut NumericStatus,
) -> Option<SpatialOrbitSolution> {
    let radius_q12 = position_radius_q12(state.position, status);
    let speed2_q20 = add(
        multiply_scaled(state.velocity.x(), state.velocity.x(), 28, status),
        add(
            multiply_scaled(state.velocity.y(), state.velocity.y(), 28, status),
            multiply_scaled(state.velocity.z(), state.velocity.z(), 28, status),
            status,
        ),
        status,
    );
    let kinetic_q24 = (speed2_q20 >> 1).checked_shl(4).unwrap_or(i32::MAX);
    let potential_q24 = divide_scaled(EARTH_MU_Q12, radius_q12, 24, status);
    let energy_q24 = subtract(kinetic_q24, potential_q24, status);
    let h = cross_mixed_scaled::<12, 24, 14>(state.position, state.velocity, status);
    let h_magnitude_q14 = magnitude3_floor(h.x(), h.y(), h.z(), status).min(i32::MAX as u32) as i32;
    let inclination_turn16 = if h_magnitude_q14 == 0 {
        0
    } else {
        acos_turn16(divide_scaled(h.z(), h_magnitude_q14, 30, status))
    };
    if !status.is_clear() {
        return None;
    }
    if energy_q24 >= 0 {
        return Some(SpatialOrbitSolution {
            class: OrbitClass::Escape,
            specific_energy: SpecificEnergy::from_raw(energy_q24),
            eccentricity: Eccentricity::from_raw(1 << 16),
            perigee: Radius::from_raw(radius_q12),
            apogee: Radius::from_raw(i32::MAX),
            inclination_turn16,
        });
    }
    let radial_dot_q14 = add(
        multiply_scaled(state.position.x(), state.velocity.x(), 22, status),
        add(
            multiply_scaled(state.position.y(), state.velocity.y(), 22, status),
            multiply_scaled(state.position.z(), state.velocity.z(), 22, status),
            status,
        ),
        status,
    );
    let radial_velocity_q24 = divide_scaled(radial_dot_q14, radius_q12, 22, status);
    let tangential_velocity_q24 = divide_scaled(h_magnitude_q14, radius_q12, 22, status);
    let vh_q12 = multiply_scaled(tangential_velocity_q24, h_magnitude_q14, 26, status);
    let radial_h_q12 = multiply_scaled(radial_velocity_q24, h_magnitude_q14, 26, status);
    let e_cos_q28 = subtract(
        divide_scaled(vh_q12, EARTH_MU_Q12, 28, status),
        1 << 28,
        status,
    );
    let e_sin_q28 = divide_scaled(radial_h_q12, EARTH_MU_Q12, 28, status);
    let e2_q28 = add(
        multiply_scaled(e_cos_q28, e_cos_q28, 28, status),
        multiply_scaled(e_sin_q28, e_sin_q28, 28, status),
        status,
    );
    let e2_q16 = add(e2_q28, 1 << 11, status) >> 12;
    let eccentricity_q16 =
        crate::phase2_numeric::sqrt_floor_u32((e2_q16.max(0) as u32) << 16) as i32;
    let twice_energy = energy_q24.checked_mul(2).unwrap_or(i32::MIN);
    let semi_major_q12 = divide_scaled(subtract(0, EARTH_MU_Q12, status), twice_energy, 24, status);
    let perigee_q12 = multiply_scaled(semi_major_q12, (1 << 16) - eccentricity_q16, 16, status);
    let apogee_q12 = multiply_scaled(semi_major_q12, (1 << 16) + eccentricity_q16, 16, status);
    if !status.is_clear() {
        return None;
    }
    let atmosphere_radius = add(EARTH_RADIUS_Q12, ATMOSPHERE_TOP_Q12, status);
    let class = if perigee_q12 <= EARTH_RADIUS_Q12 {
        OrbitClass::Impact
    } else if perigee_q12 < atmosphere_radius {
        OrbitClass::Suborbital
    } else {
        OrbitClass::StableOrbit
    };
    Some(SpatialOrbitSolution {
        class,
        specific_energy: SpecificEnergy::from_raw(energy_q24),
        eccentricity: Eccentricity::from_raw(eccentricity_q16),
        perigee: Radius::from_raw(perigee_q12),
        apogee: Radius::from_raw(apogee_q12),
        inclination_turn16,
    })
}
