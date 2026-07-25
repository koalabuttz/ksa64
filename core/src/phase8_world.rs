//! Phase 8 local-ENU wind, rail, translation, and hobby-scale rigid-body propagation.

use crate::numeric::{
    add, divide_scaled, magnitude3_floor, multiply_scaled, subtract, NumericFault, NumericStatus,
};
use crate::phase2_numeric::sin_cos_binary_q15;
use crate::phase8_numeric::{
    BodyAngularRate, BodyToEnuQuaternion, BodyTorque, EnuAcceleration, EnuForce, EnuPosition,
    EnuVelocity, EnuWind, SpatialInertia, SpatialMass, SpatialPosition, SpatialTime,
    SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS, SPATIAL_TIME_FRACTIONAL_BITS,
    SPATIAL_VELOCITY_FRACTIONAL_BITS, SPATIAL_WIND_FRACTIONAL_BITS,
};
use crate::phase8_pack::{SpatialMissionPack, WindProfilePack};
use crate::spatial_numeric::{FixedVec3, QuaternionQ30};

const PI_Q28: i32 = 843_314_857;
const Q30_ONE: i32 = 1 << 30;
const PHASE8_ENVIRONMENT_STEP_RAW: i32 = 250 << 13;
pub const PHASE8_ENVIRONMENT_TOP_M: i32 = 3_000;
const PHASE8_DENSITY_Q29: [i32; 13] = [
    657_666_877,
    642_027_311,
    626_675_068,
    611_606_433,
    596_817_722,
    582_305_276,
    568_065_463,
    554_094_682,
    540_389_355,
    526_945_936,
    513_760_903,
    500_830_764,
    488_152_051,
];
const PHASE8_SOUND_SPEED_Q19: [i32; 13] = [
    178_412_054,
    177_908_292,
    177_403_139,
    176_896_584,
    176_388_613,
    175_879_216,
    175_368_379,
    174_856_090,
    174_342_336,
    173_827_104,
    173_310_381,
    172_792_153,
    172_272_407,
];
const PHASE8_GRAVITY_Q19: [i32; 13] = [
    5_141_509, 5_141_105, 5_140_702, 5_140_299, 5_139_895, 5_139_492, 5_139_089, 5_138_686,
    5_138_282, 5_137_879, 5_137_476, 5_137_073, 5_136_670,
];

fn sample_phase8_environment(
    altitude_raw: i32,
    status: &mut NumericStatus,
) -> Result<(i32, i32, i32), Phase8WorldError> {
    let altitude_raw = altitude_raw.max(0);
    if altitude_raw > PHASE8_ENVIRONMENT_TOP_M << 13 {
        return Err(Phase8WorldError::ModelEnvelopeExceeded);
    }
    let index = (altitude_raw / PHASE8_ENVIRONMENT_STEP_RAW) as usize;
    if index >= PHASE8_DENSITY_Q29.len() - 1 {
        let last = PHASE8_DENSITY_Q29.len() - 1;
        return Ok((
            PHASE8_DENSITY_Q29[last],
            PHASE8_SOUND_SPEED_Q19[last],
            PHASE8_GRAVITY_Q19[last],
        ));
    }
    let remainder = altitude_raw - index as i32 * PHASE8_ENVIRONMENT_STEP_RAW;
    let fraction_q16 = divide_scaled(remainder, PHASE8_ENVIRONMENT_STEP_RAW, 16, status);
    let interpolate = |values: &[i32; 13], status: &mut NumericStatus| {
        add(
            values[index],
            multiply_scaled(values[index + 1] - values[index], fraction_q16, 16, status),
            status,
        )
    };
    Ok((
        interpolate(&PHASE8_DENSITY_Q29, status),
        interpolate(&PHASE8_SOUND_SPEED_Q19, status),
        interpolate(&PHASE8_GRAVITY_Q19, status),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase8WorldError {
    InvalidConfiguration,
    ModelEnvelopeExceeded,
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct HobbySpatialState {
    pub time: SpatialTime,
    pub position: EnuPosition,
    pub velocity: EnuVelocity,
    pub acceleration: EnuAcceleration,
    pub attitude: BodyToEnuQuaternion,
    pub angular_rate: BodyAngularRate,
}

impl HobbySpatialState {
    pub const fn at_rest(position: EnuPosition, attitude: BodyToEnuQuaternion) -> Self {
        Self {
            time: SpatialTime::ZERO,
            position,
            velocity: EnuVelocity::ZERO,
            acceleration: EnuAcceleration::ZERO,
            attitude,
            angular_rate: BodyAngularRate::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialWindSample {
    pub mean: EnuWind,
    pub gust: EnuWind,
    pub total: EnuWind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbySpatialEnvironment {
    pub wind: SpatialWindSample,
    pub air_velocity_body: EnuVelocity,
    pub air_velocity_enu: EnuVelocity,
    pub air_speed_q19: i32,
    pub density_q29: i32,
    pub sound_speed_q19: i32,
    pub gravity_q19: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailState {
    pub distance: SpatialPosition,
    pub speed_raw_q19: i32,
}

impl RailState {
    pub const REST: Self = Self {
        distance: SpatialPosition::ZERO,
        speed_raw_q19: 0,
    };
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn keyed_gust_target(
    wind: &WindProfilePack,
    case_seed: u32,
    epoch: u32,
    axis: u32,
    amplitude_q22: i32,
    status: &mut NumericStatus,
) -> i32 {
    if amplitude_q22 == 0 || wind.max_gust.raw() == 0 {
        return 0;
    }
    let key = wind.identity
        ^ wind.gust_seed.rotate_left(7)
        ^ case_seed.rotate_left(13)
        ^ epoch.wrapping_mul(0x9e37_79b9)
        ^ axis.wrapping_mul(0x85eb_ca6b);
    let signed_q15 = (mix32(key) & 0xffff) as i32 - 32_768;
    multiply_scaled(amplitude_q22, signed_q15, 15, status)
        .clamp(-wind.max_gust.raw(), wind.max_gust.raw())
}

fn clamp_gust_magnitude(vector: EnuWind, maximum: i32, status: &mut NumericStatus) -> EnuWind {
    let magnitude = magnitude3_floor(vector.x(), vector.y(), vector.z(), status);
    if maximum <= 0 || magnitude <= maximum as u32 {
        return vector;
    }
    let scale_q30 = divide_scaled(maximum, magnitude.min(i32::MAX as u32) as i32, 30, status);
    vector.scale::<30>(scale_q30, status)
}

fn interpolate_wind_knots(
    wind: &WindProfilePack,
    altitude_q13: i32,
    status: &mut NumericStatus,
) -> EnuWind {
    let count = wind.knot_count as usize;
    if altitude_q13 <= wind.knots[0].altitude.raw() {
        return EnuWind::new(wind.knots[0].east.raw(), wind.knots[0].north.raw(), 0);
    }
    let mut index = 0usize;
    while index + 1 < count && altitude_q13 > wind.knots[index + 1].altitude.raw() {
        index += 1;
    }
    if index + 1 >= count {
        let knot = wind.knots[count - 1];
        return EnuWind::new(knot.east.raw(), knot.north.raw(), 0);
    }
    let low = wind.knots[index];
    let high = wind.knots[index + 1];
    let span = subtract(high.altitude.raw(), low.altitude.raw(), status);
    let offset = subtract(altitude_q13, low.altitude.raw(), status);
    let fraction_q16 = divide_scaled(offset, span, 16, status).clamp(0, 65_536);
    let east = add(
        low.east.raw(),
        multiply_scaled(
            subtract(high.east.raw(), low.east.raw(), status),
            fraction_q16,
            16,
            status,
        ),
        status,
    );
    let north = add(
        low.north.raw(),
        multiply_scaled(
            subtract(high.north.raw(), low.north.raw(), status),
            fraction_q16,
            16,
            status,
        ),
        status,
    );
    EnuWind::new(east, north, 0)
}

pub fn sample_spatial_wind(
    wind: &WindProfilePack,
    altitude: SpatialPosition,
    time: SpatialTime,
    case_seed: u32,
    status: &mut NumericStatus,
) -> Result<SpatialWindSample, Phase8WorldError> {
    if !wind.is_valid() || time.raw() < 0 {
        return Err(Phase8WorldError::InvalidConfiguration);
    }
    let mean = interpolate_wind_knots(wind, altitude.raw().max(0), status);
    let cadence = wind.gust_cadence.raw();
    let epoch = (time.raw() / cadence) as u32;
    let remainder = time.raw() % cadence;
    let fraction_q16 = divide_scaled(remainder, cadence, 16, status).clamp(0, 65_536);
    let current = clamp_gust_magnitude(
        EnuWind::new(
            keyed_gust_target(
                wind,
                case_seed,
                epoch,
                0,
                wind.gust_amplitude_east.raw(),
                status,
            ),
            keyed_gust_target(
                wind,
                case_seed,
                epoch,
                1,
                wind.gust_amplitude_north.raw(),
                status,
            ),
            0,
        ),
        wind.max_gust.raw(),
        status,
    );
    let next = clamp_gust_magnitude(
        EnuWind::new(
            keyed_gust_target(
                wind,
                case_seed,
                epoch.wrapping_add(1),
                0,
                wind.gust_amplitude_east.raw(),
                status,
            ),
            keyed_gust_target(
                wind,
                case_seed,
                epoch.wrapping_add(1),
                1,
                wind.gust_amplitude_north.raw(),
                status,
            ),
            0,
        ),
        wind.max_gust.raw(),
        status,
    );
    let gust = current.checked_add(
        next.checked_sub(current, status)
            .scale::<16>(fraction_q16, status),
        status,
    );
    let total = mean.checked_add(gust, status);
    if status.is_clear() {
        Ok(SpatialWindSample { mean, gust, total })
    } else {
        Err(Phase8WorldError::Numeric)
    }
}

fn radians_q28_to_binary(angle_q28: i32, status: &mut NumericStatus) -> u16 {
    divide_scaled(angle_q28, PI_Q28, 15, status) as u16
}

pub fn rail_axis_from_mission(
    mission: SpatialMissionPack,
    status: &mut NumericStatus,
) -> Result<FixedVec3<30>, Phase8WorldError> {
    if !mission.is_valid() {
        return Err(Phase8WorldError::InvalidConfiguration);
    }
    let (sin_elevation, cos_elevation) = sin_cos_binary_q15(radians_q28_to_binary(
        mission.launch_elevation.raw(),
        status,
    ));
    let (sin_azimuth, cos_azimuth) =
        sin_cos_binary_q15(radians_q28_to_binary(mission.launch_azimuth.raw(), status));
    let axis = FixedVec3::new(
        multiply_scaled(i32::from(cos_elevation), i32::from(sin_azimuth), 0, status),
        multiply_scaled(i32::from(cos_elevation), i32::from(cos_azimuth), 0, status),
        i32::from(sin_elevation) << 15,
    );
    if status.is_clear() {
        Ok(axis)
    } else {
        Err(Phase8WorldError::Numeric)
    }
}

pub fn attitude_from_rail_axis(
    axis_q30: FixedVec3<30>,
    status: &mut NumericStatus,
) -> Result<BodyToEnuQuaternion, Phase8WorldError> {
    let attitude = QuaternionQ30::new(
        add(Q30_ONE, axis_q30.x(), status),
        0,
        subtract(0, axis_q30.z(), status),
        axis_q30.y(),
    )
    .normalized(status);
    if status.is_clear() {
        Ok(attitude)
    } else {
        Err(Phase8WorldError::Numeric)
    }
}

pub fn rail_exit_distance(
    rail_length: SpatialPosition,
    aft_guide_from_tail: SpatialPosition,
) -> SpatialPosition {
    SpatialPosition::from_raw((rail_length.raw() - aft_guide_from_tail.raw()).max(0))
}

pub fn step_rail_constrained(
    state: HobbySpatialState,
    rail: RailState,
    axis_q30: FixedVec3<30>,
    axial_acceleration_q19: i32,
    timestep: SpatialTime,
) -> Result<(HobbySpatialState, RailState), Phase8WorldError> {
    if timestep.raw() <= 0 || axial_acceleration_q19 < 0 {
        return Err(Phase8WorldError::InvalidConfiguration);
    }
    let mut status = NumericStatus::CLEAR;
    let speed = add(
        rail.speed_raw_q19,
        multiply_scaled(
            axial_acceleration_q19,
            timestep.raw(),
            SPATIAL_TIME_FRACTIONAL_BITS,
            &mut status,
        ),
        &mut status,
    );
    let distance = add(
        rail.distance.raw(),
        multiply_scaled(speed, timestep.raw(), 24, &mut status),
        &mut status,
    );
    let position = EnuPosition::new(
        multiply_scaled(axis_q30.x(), distance, 30, &mut status),
        multiply_scaled(axis_q30.y(), distance, 30, &mut status),
        multiply_scaled(axis_q30.z(), distance, 30, &mut status),
    );
    let velocity = EnuVelocity::new(
        multiply_scaled(axis_q30.x(), speed, 30, &mut status),
        multiply_scaled(axis_q30.y(), speed, 30, &mut status),
        multiply_scaled(axis_q30.z(), speed, 30, &mut status),
    );
    let acceleration = EnuAcceleration::new(
        multiply_scaled(axis_q30.x(), axial_acceleration_q19, 30, &mut status),
        multiply_scaled(axis_q30.y(), axial_acceleration_q19, 30, &mut status),
        multiply_scaled(axis_q30.z(), axial_acceleration_q19, 30, &mut status),
    );
    let successor = HobbySpatialState {
        time: SpatialTime::from_raw(add(state.time.raw(), timestep.raw(), &mut status)),
        position,
        velocity,
        acceleration,
        attitude: state.attitude,
        angular_rate: BodyAngularRate::ZERO,
    };
    if status.is_clear() {
        Ok((
            successor,
            RailState {
                distance: SpatialPosition::from_raw(distance),
                speed_raw_q19: speed,
            },
        ))
    } else {
        Err(Phase8WorldError::Numeric)
    }
}

pub fn acceleration_from_force(
    force: EnuForce,
    mass: SpatialMass,
    gravity_q19: i32,
    status: &mut NumericStatus,
) -> EnuAcceleration {
    if mass.raw() <= 0 {
        status.record(NumericFault::InvalidInput);
        return EnuAcceleration::ZERO;
    }
    EnuAcceleration::new(
        divide_scaled(force.x(), mass.raw(), 27, status),
        divide_scaled(force.y(), mass.raw(), 27, status),
        subtract(
            divide_scaled(force.z(), mass.raw(), 27, status),
            gravity_q19,
            status,
        ),
    )
}

pub fn step_free_translation(
    state: HobbySpatialState,
    acceleration: EnuAcceleration,
    timestep: SpatialTime,
) -> Result<HobbySpatialState, Phase8WorldError> {
    if timestep.raw() <= 0 {
        return Err(Phase8WorldError::InvalidConfiguration);
    }
    let mut status = NumericStatus::CLEAR;
    let delta_velocity =
        acceleration.scale::<SPATIAL_TIME_FRACTIONAL_BITS>(timestep.raw(), &mut status);
    let velocity = state.velocity.checked_add(delta_velocity, &mut status);
    let position_delta = EnuPosition::new(
        multiply_scaled(velocity.x(), timestep.raw(), 24, &mut status),
        multiply_scaled(velocity.y(), timestep.raw(), 24, &mut status),
        multiply_scaled(velocity.z(), timestep.raw(), 24, &mut status),
    );
    let position = state.position.checked_add(position_delta, &mut status);
    let successor = HobbySpatialState {
        time: SpatialTime::from_raw(add(state.time.raw(), timestep.raw(), &mut status)),
        position,
        velocity,
        acceleration,
        ..state
    };
    if status.is_clear() {
        Ok(successor)
    } else {
        Err(Phase8WorldError::Numeric)
    }
}

fn hobby_angular_acceleration(
    inertia: [SpatialInertia; 3],
    rate: BodyAngularRate,
    torque: BodyTorque,
    status: &mut NumericStatus,
) -> BodyAngularRate {
    if inertia.iter().any(|value| value.raw() <= 0) {
        status.record(NumericFault::InvalidInput);
        return BodyAngularRate::ZERO;
    }
    let rate_products = [
        multiply_scaled(
            rate.y(),
            rate.z(),
            SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS,
            status,
        ),
        multiply_scaled(
            rate.z(),
            rate.x(),
            SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS,
            status,
        ),
        multiply_scaled(
            rate.x(),
            rate.y(),
            SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS,
            status,
        ),
    ];
    let coupled = [
        multiply_scaled(
            subtract(inertia[2].raw(), inertia[1].raw(), status),
            rate_products[0],
            31,
            status,
        ),
        multiply_scaled(
            subtract(inertia[0].raw(), inertia[2].raw(), status),
            rate_products[1],
            31,
            status,
        ),
        multiply_scaled(
            subtract(inertia[1].raw(), inertia[0].raw(), status),
            rate_products[2],
            31,
            status,
        ),
    ];
    BodyAngularRate::new(
        divide_scaled(
            subtract(torque.x(), coupled[0], status),
            inertia[0].raw(),
            31,
            status,
        ),
        divide_scaled(
            subtract(torque.y(), coupled[1], status),
            inertia[1].raw(),
            31,
            status,
        ),
        divide_scaled(
            subtract(torque.z(), coupled[2], status),
            inertia[2].raw(),
            31,
            status,
        ),
    )
}

fn integrate_hobby_attitude(
    attitude: BodyToEnuQuaternion,
    rate: BodyAngularRate,
    timestep_q18: i32,
    status: &mut NumericStatus,
) -> BodyToEnuQuaternion {
    let omega = QuaternionQ30::new(0, rate.x() << 6, rate.y() << 6, rate.z() << 6);
    let derivative_q30 = attitude.hamilton(omega, status);
    QuaternionQ30::new(
        add(
            attitude.w(),
            multiply_scaled(derivative_q30.w(), timestep_q18, 19, status),
            status,
        ),
        add(
            attitude.x(),
            multiply_scaled(derivative_q30.x(), timestep_q18, 19, status),
            status,
        ),
        add(
            attitude.y(),
            multiply_scaled(derivative_q30.y(), timestep_q18, 19, status),
            status,
        ),
        add(
            attitude.z(),
            multiply_scaled(derivative_q30.z(), timestep_q18, 19, status),
            status,
        ),
    )
    .normalized(status)
}

pub fn step_hobby_attitude(
    state: HobbySpatialState,
    inertia: [SpatialInertia; 3],
    torque: BodyTorque,
    timestep: SpatialTime,
) -> Result<HobbySpatialState, Phase8WorldError> {
    if timestep.raw() <= 0 {
        return Err(Phase8WorldError::InvalidConfiguration);
    }
    let mut status = NumericStatus::CLEAR;
    let angular_acceleration =
        hobby_angular_acceleration(inertia, state.angular_rate, torque, &mut status);
    let angular_rate = state.angular_rate.checked_add(
        angular_acceleration.scale::<SPATIAL_TIME_FRACTIONAL_BITS>(timestep.raw(), &mut status),
        &mut status,
    );
    let attitude =
        integrate_hobby_attitude(state.attitude, angular_rate, timestep.raw(), &mut status);
    let successor = HobbySpatialState {
        attitude,
        angular_rate,
        ..state
    };
    if status.is_clear() {
        Ok(successor)
    } else {
        Err(Phase8WorldError::Numeric)
    }
}

pub fn evaluate_hobby_spatial_environment(
    state: HobbySpatialState,
    wind: &WindProfilePack,
    case_seed: u32,
) -> Result<HobbySpatialEnvironment, Phase8WorldError> {
    let mut status = NumericStatus::CLEAR;
    let wind_sample = sample_spatial_wind(
        wind,
        SpatialPosition::from_raw(state.position.z().max(0)),
        state.time,
        case_seed,
        &mut status,
    )?;
    let wind_velocity_q19 = EnuVelocity::new(
        wind_sample.total.x() >> (SPATIAL_WIND_FRACTIONAL_BITS - SPATIAL_VELOCITY_FRACTIONAL_BITS),
        wind_sample.total.y() >> (SPATIAL_WIND_FRACTIONAL_BITS - SPATIAL_VELOCITY_FRACTIONAL_BITS),
        wind_sample.total.z() >> (SPATIAL_WIND_FRACTIONAL_BITS - SPATIAL_VELOCITY_FRACTIONAL_BITS),
    );
    let air_velocity_enu = state.velocity.checked_sub(wind_velocity_q19, &mut status);
    let body_velocity = state
        .attitude
        .conjugate()
        .rotate(air_velocity_enu, &mut status);
    let speed = magnitude3_floor(
        air_velocity_enu.x(),
        air_velocity_enu.y(),
        air_velocity_enu.z(),
        &mut status,
    );
    let (density_q29, sound_speed_q19, gravity_q19) =
        sample_phase8_environment(state.position.z(), &mut status)?;
    if !status.is_clear() || speed > i32::MAX as u32 {
        return Err(Phase8WorldError::Numeric);
    }
    Ok(HobbySpatialEnvironment {
        wind: wind_sample,
        air_velocity_body: body_velocity,
        air_velocity_enu,
        air_speed_q19: speed as i32,
        density_q29,
        sound_speed_q19,
        gravity_q19,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase8_format::KWP8_MAX_WIND_KNOTS;
    use crate::phase8_pack::{parse_spatial_mission_pack, parse_wind_profile_pack, WindKnot};

    mod wind_vectors {
        include!("../../phase8/generated/wind_vectors_v1.rs");
    }
    fn mission() -> SpatialMissionPack {
        parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
            .unwrap()
    }

    fn calm() -> WindProfilePack {
        parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
            .unwrap()
    }
    fn layered_gust() -> WindProfilePack {
        let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
        knots[0] = WindKnot {
            altitude: SpatialPosition::ZERO,
            east: crate::phase8_numeric::SpatialWind::from_raw(1 << 22),
            north: crate::phase8_numeric::SpatialWind::from_raw(-2 << 22),
        };
        knots[1] = WindKnot {
            altitude: SpatialPosition::from_raw(1_000 << 13),
            east: crate::phase8_numeric::SpatialWind::from_raw(5 << 22),
            north: crate::phase8_numeric::SpatialWind::from_raw(2 << 22),
        };
        WindProfilePack {
            identity: wind_vectors::IDENTITY,
            gust_seed: wind_vectors::GUST_SEED,
            gust_cadence: SpatialTime::from_raw(1 << 18),
            gust_amplitude_east: crate::phase8_numeric::SpatialWind::from_raw(3 << 22),
            gust_amplitude_north: crate::phase8_numeric::SpatialWind::from_raw(2 << 22),
            max_gust: crate::phase8_numeric::SpatialWind::from_raw(4 << 22),
            knot_count: 2,
            knots,
        }
    }

    #[test]
    fn vertical_rail_axis_and_attitude_are_consistent() {
        let mut status = NumericStatus::CLEAR;
        let axis = rail_axis_from_mission(mission(), &mut status).unwrap();
        assert!(axis.x().abs() <= 65_536);
        assert!(axis.y().abs() <= 65_536);
        assert!((axis.z() - Q30_ONE).abs() <= 65_536);
        let attitude = attitude_from_rail_axis(axis, &mut status).unwrap();
        let body_x = attitude.rotate(FixedVec3::<30>::new(Q30_ONE, 0, 0), &mut status);
        assert!((body_x.z() - Q30_ONE).abs() <= 8);
        assert!(status.is_clear());
    }

    #[test]
    fn calm_and_keyed_wind_are_repeatable() {
        let mut status = NumericStatus::CLEAR;
        let a = sample_spatial_wind(
            &calm(),
            SpatialPosition::from_raw(500 << 13),
            SpatialTime::from_raw(123 << 18),
            7,
            &mut status,
        )
        .unwrap();
        let b = sample_spatial_wind(
            &calm(),
            SpatialPosition::from_raw(500 << 13),
            SpatialTime::from_raw(123 << 18),
            7,
            &mut status,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.total, EnuWind::ZERO);
        assert!(status.is_clear());
    }

    #[test]
    fn rail_and_free_translation_are_atomic() {
        let mut status = NumericStatus::CLEAR;
        let axis = rail_axis_from_mission(mission(), &mut status).unwrap();
        let attitude = attitude_from_rail_axis(axis, &mut status).unwrap();
        let initial = HobbySpatialState::at_rest(EnuPosition::ZERO, attitude);
        let (rail_step, rail) = step_rail_constrained(
            initial,
            RailState::REST,
            axis,
            10 << 19,
            SpatialTime::from_raw(2_621),
        )
        .unwrap();
        assert!(rail.distance.raw() > 0);
        assert_eq!(rail_step.position.x(), 0);
        assert_eq!(rail_step.position.y(), 0);
        assert!(rail_step.position.z() > 0);
        let free = step_free_translation(
            rail_step,
            EnuAcceleration::new(1 << 19, 0, 0),
            SpatialTime::from_raw(2_621),
        )
        .unwrap();
        assert!(free.position.x() > rail_step.position.x());
        assert!(free.velocity.x() > rail_step.velocity.x());
    }

    #[test]
    fn torque_free_and_constant_torque_are_bounded() {
        let state = HobbySpatialState::at_rest(EnuPosition::ZERO, QuaternionQ30::IDENTITY);
        let inertia = [
            SpatialInertia::from_raw(1 << 19),
            SpatialInertia::from_raw(2 << 19),
            SpatialInertia::from_raw(2 << 19),
        ];
        let idle = step_hobby_attitude(
            state,
            inertia,
            BodyTorque::ZERO,
            SpatialTime::from_raw(2_621),
        )
        .unwrap();
        assert_eq!(idle.attitude, state.attitude);
        assert_eq!(idle.angular_rate, BodyAngularRate::ZERO);
        let driven = step_hobby_attitude(
            state,
            inertia,
            BodyTorque::new(0, 1 << 12, 0),
            SpatialTime::from_raw(2_621),
        )
        .unwrap();
        assert!(driven.angular_rate.y() > 0);
        assert_ne!(driven.attitude, state.attitude);
    }

    #[test]
    fn layered_gust_vectors_match_independent_generator() {
        let wind = layered_gust();
        for vector in wind_vectors::WIND_VECTORS {
            let mut status = NumericStatus::CLEAR;
            let sample = sample_spatial_wind(
                &wind,
                SpatialPosition::from_raw(vector.altitude_q13),
                SpatialTime::from_raw(vector.time_q18),
                wind_vectors::CASE_SEED,
                &mut status,
            )
            .unwrap();
            assert_eq!(
                [sample.mean.x(), sample.mean.y(), sample.mean.z()],
                vector.mean_q22
            );
            assert_eq!(
                [sample.gust.x(), sample.gust.y(), sample.gust.z()],
                vector.gust_q22
            );
            assert_eq!(
                [sample.total.x(), sample.total.y(), sample.total.z()],
                vector.total_q22
            );
            assert!(status.is_clear());
        }
    }

    #[test]
    fn inertial_translation_and_crosswind_are_explicit() {
        let state = HobbySpatialState {
            velocity: EnuVelocity::new(10 << 19, -2 << 19, 1 << 19),
            ..HobbySpatialState::at_rest(EnuPosition::ZERO, QuaternionQ30::IDENTITY)
        };
        let inertial =
            step_free_translation(state, EnuAcceleration::ZERO, SpatialTime::from_raw(1 << 18))
                .unwrap();
        assert_eq!(inertial.velocity, state.velocity);
        assert_eq!(
            inertial.position,
            EnuPosition::new(10 << 13, -2 << 13, 1 << 13)
        );

        let still = HobbySpatialState::at_rest(EnuPosition::ZERO, QuaternionQ30::IDENTITY);
        let environment = evaluate_hobby_spatial_environment(still, &layered_gust(), 0).unwrap();
        assert_ne!(environment.air_velocity_enu.x(), 0);
        assert_ne!(environment.air_velocity_enu.y(), 0);
    }
    #[test]
    fn environment_uses_air_relative_velocity() {
        let state = HobbySpatialState {
            velocity: EnuVelocity::new(10 << 19, 0, 0),
            ..HobbySpatialState::at_rest(EnuPosition::ZERO, QuaternionQ30::IDENTITY)
        };
        let environment = evaluate_hobby_spatial_environment(state, &calm(), 1).unwrap();
        assert_eq!(environment.air_velocity_enu.x(), 10 << 19);
        assert_eq!(environment.air_speed_q19, 10 << 19);
        assert!(environment.density_q29 > 0);
        assert!(environment.gravity_q19 > 0);
    }
}

#[cfg(test)]
mod phase8_environment_prefix_tests {
    use super::*;
    use crate::phase7_environment::sample_hobby_environment;
    use crate::phase7_numeric::HobbyAltitude;
    #[test]
    fn compact_environment_is_exact_and_fails_above_its_declared_envelope() {
        for metres in [0, 125, 754, 3_000] {
            let mut expected_status = NumericStatus::CLEAR;
            let expected = sample_hobby_environment(
                HobbyAltitude::from_raw(metres << 13),
                &mut expected_status,
            );
            let mut actual_status = NumericStatus::CLEAR;
            let actual = sample_phase8_environment(metres << 13, &mut actual_status).unwrap();
            assert_eq!(
                actual,
                (
                    expected.density.raw(),
                    expected.sound_speed.unwrap().raw(),
                    expected.gravity.raw()
                )
            );
            assert!(expected_status.is_clear() && actual_status.is_clear());
        }
        let mut status = NumericStatus::CLEAR;
        assert_eq!(
            sample_phase8_environment((PHASE8_ENVIRONMENT_TOP_M + 1) << 13, &mut status),
            Err(Phase8WorldError::ModelEnvelopeExceeded)
        );
    }
}
