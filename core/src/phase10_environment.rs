//! Global Earth environment for Phase 10.
//!
//! All production paths are allocation-free and deterministic. Host floating
//! point is used only by the independent fixture generator.

use crate::numeric::{
    add, divide_scaled, magnitude3_floor, multiply_scaled, subtract, NumericFault, NumericStatus,
};
use crate::phase10_contract::{
    AtmosphereModelId, EarthModelPack, EARTH_ROTATION_Q30_RAD_S, PHASE10_CONTRACT_ID, WGS84_J2_Q30,
    WGS84_MU_Q8_KM3_S2, WGS84_SEMI_MAJOR_Q12_KM, WGS84_SEMI_MINOR_Q12_KM,
};
use crate::phase10_numeric::{
    interpolate_i32, GlobalAccelerationVec, GlobalPositionVec, GlobalVelocityVec,
};
use crate::scenario::crc32_ieee;
use crate::spatial_numeric::{cross_mixed_scaled, FixedVec3};

pub const KAT10_MAX_KNOTS: usize = 64;
pub const KAT10_HEADER_LENGTH: usize = 128;
pub const KAT10_KNOT_LENGTH: usize = 40;
pub const KAT10_LENGTH: usize = KAT10_HEADER_LENGTH + KAT10_MAX_KNOTS * KAT10_KNOT_LENGTH + 4;

pub const ATMOSPHERE_DENSITY_FRACTIONAL_BITS: u8 = 28;
pub const ATMOSPHERE_PRESSURE_FRACTIONAL_BITS: u8 = 14;
pub const ATMOSPHERE_TEMPERATURE_FRACTIONAL_BITS: u8 = 16;
pub const ATMOSPHERE_SOUND_SPEED_FRACTIONAL_BITS: u8 = 16;
pub const ATMOSPHERE_WIND_FRACTIONAL_BITS: u8 = 19;

const VERSION: u16 = 10;
const KIND_ATMOSPHERE: u16 = 3;
const WGS84_E2_Q30: i32 = 7_188_036;
const CORDIC_GAIN_Q30: i32 = 652_032_874;
const PI_Q28: i32 = 843_314_857;
const HALF_PI_Q28: i32 = 421_657_428;
const CORDIC_ATAN_Q28: [i32; 28] = [
    210_828_714,
    124_459_457,
    65_760_959,
    33_381_290,
    16_755_422,
    8_385_879,
    4_193_963,
    2_097_109,
    1_048_571,
    524_287,
    262_144,
    131_072,
    65_536,
    32_768,
    16_384,
    8_192,
    4_096,
    2_048,
    1_024,
    512,
    256,
    128,
    64,
    32,
    16,
    8,
    4,
    2,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct AtmosphereKnot {
    pub altitude_q12_km: i32,
    pub density_q28_kg_m3: i32,
    pub pressure_q14_pa: i32,
    pub temperature_q16_k: i32,
    pub speed_of_sound_q16_m_s: i32,
    pub wind_enu_q19_m_s: FixedVec3<ATMOSPHERE_WIND_FRACTIONAL_BITS>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct CompiledAtmospherePack {
    pub identity: u32,
    pub earth_identity: u32,
    pub source_hash: u32,
    pub count: u8,
    pub zero_above_last: bool,
    pub knots: [AtmosphereKnot; KAT10_MAX_KNOTS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentError {
    Length,
    Magic,
    Version,
    Contract,
    Identity,
    Reserved,
    Checksum,
    Range,
    Coverage,
    Numeric,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct AtmosphereSample {
    pub density_q28_kg_m3: i32,
    pub pressure_q14_pa: i32,
    pub temperature_q16_k: i32,
    pub speed_of_sound_q16_m_s: i32,
    pub wind_enu_q19_m_s: FixedVec3<ATMOSPHERE_WIND_FRACTIONAL_BITS>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct GeodeticState {
    pub latitude_q28_rad: i32,
    pub longitude_q28_rad: i32,
    pub height_q12_km: i32,
}

impl CompiledAtmospherePack {
    pub fn validate(&self) -> Result<(), EnvironmentError> {
        if self.identity == 0
            || self.earth_identity == 0
            || self.source_hash == 0
            || self.count < 2
            || self.count as usize > KAT10_MAX_KNOTS
        {
            return Err(EnvironmentError::Range);
        }
        let active = &self.knots[..self.count as usize];
        for (index, knot) in active.iter().enumerate() {
            if knot.density_q28_kg_m3 < 0
                || knot.pressure_q14_pa < 0
                || knot.temperature_q16_k <= 0
                || knot.speed_of_sound_q16_m_s <= 0
                || (index > 0 && knot.altitude_q12_km <= active[index - 1].altitude_q12_km)
            {
                return Err(EnvironmentError::Range);
            }
        }
        if self.knots[self.count as usize..]
            .iter()
            .any(|knot| *knot != AtmosphereKnot::default())
        {
            return Err(EnvironmentError::Reserved);
        }
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> Result<(), EnvironmentError> {
        self.validate()?;
        if output.len() != KAT10_LENGTH {
            return Err(EnvironmentError::Length);
        }
        output.fill(0);
        output[..5].copy_from_slice(b"KAT10");
        p16(output, 6, VERSION);
        p16(output, 8, KAT10_HEADER_LENGTH as u16);
        p16(output, 10, KIND_ATMOSPHERE);
        p32(output, 12, KAT10_LENGTH as u32);
        p32(output, 16, PHASE10_CONTRACT_ID);
        p32(output, 20, self.identity);
        p32(output, 32, self.earth_identity);
        p32(output, 36, self.source_hash);
        output[40] = AtmosphereModelId::CompiledProfileV1 as u8;
        output[41] = self.count;
        output[42] = self.zero_above_last as u8;
        pi32(output, 44, self.knots[0].altitude_q12_km);
        pi32(
            output,
            48,
            self.knots[self.count as usize - 1].altitude_q12_km,
        );
        for (index, knot) in self.knots.iter().enumerate() {
            let at = KAT10_HEADER_LENGTH + index * KAT10_KNOT_LENGTH;
            pi32(output, at, knot.altitude_q12_km);
            pi32(output, at + 4, knot.density_q28_kg_m3);
            pi32(output, at + 8, knot.pressure_q14_pa);
            pi32(output, at + 12, knot.temperature_q16_k);
            pi32(output, at + 16, knot.speed_of_sound_q16_m_s);
            pi32(output, at + 20, knot.wind_enu_q19_m_s.x());
            pi32(output, at + 24, knot.wind_enu_q19_m_s.y());
            pi32(output, at + 28, knot.wind_enu_q19_m_s.z());
        }
        let checksum_at = KAT10_LENGTH - 4;
        let checksum = crc32_ieee(&output[..checksum_at]);
        p32(output, checksum_at, checksum);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, EnvironmentError> {
        if bytes.len() != KAT10_LENGTH {
            return Err(EnvironmentError::Length);
        }
        if &bytes[..5] != b"KAT10" || bytes[5] != 0 {
            return Err(EnvironmentError::Magic);
        }
        if g16(bytes, 6) != VERSION
            || g16(bytes, 8) as usize != KAT10_HEADER_LENGTH
            || g16(bytes, 10) != KIND_ATMOSPHERE
            || g32(bytes, 12) as usize != KAT10_LENGTH
        {
            return Err(EnvironmentError::Version);
        }
        if g32(bytes, 16) != PHASE10_CONTRACT_ID {
            return Err(EnvironmentError::Contract);
        }
        let checksum_at = KAT10_LENGTH - 4;
        if g32(bytes, checksum_at) != crc32_ieee(&bytes[..checksum_at]) {
            return Err(EnvironmentError::Checksum);
        }
        if bytes[24..32].iter().any(|byte| *byte != 0)
            || bytes[43] != 0
            || bytes[52..KAT10_HEADER_LENGTH].iter().any(|byte| *byte != 0)
        {
            return Err(EnvironmentError::Reserved);
        }
        if bytes[40] != AtmosphereModelId::CompiledProfileV1 as u8 {
            return Err(EnvironmentError::Version);
        }
        let identity = g32(bytes, 20);
        let count = bytes[41];
        if identity == 0 || count < 2 || count as usize > KAT10_MAX_KNOTS || bytes[42] > 1 {
            return Err(EnvironmentError::Range);
        }
        let mut knots = [AtmosphereKnot::default(); KAT10_MAX_KNOTS];
        for (index, knot) in knots.iter_mut().enumerate() {
            let at = KAT10_HEADER_LENGTH + index * KAT10_KNOT_LENGTH;
            if bytes[at + 32..at + 40].iter().any(|byte| *byte != 0) {
                return Err(EnvironmentError::Reserved);
            }
            *knot = AtmosphereKnot {
                altitude_q12_km: gi32(bytes, at),
                density_q28_kg_m3: gi32(bytes, at + 4),
                pressure_q14_pa: gi32(bytes, at + 8),
                temperature_q16_k: gi32(bytes, at + 12),
                speed_of_sound_q16_m_s: gi32(bytes, at + 16),
                wind_enu_q19_m_s: FixedVec3::new(
                    gi32(bytes, at + 20),
                    gi32(bytes, at + 24),
                    gi32(bytes, at + 28),
                ),
            };
        }
        let pack = Self {
            identity,
            earth_identity: g32(bytes, 32),
            source_hash: g32(bytes, 36),
            count,
            zero_above_last: bytes[42] != 0,
            knots,
        };
        if gi32(bytes, 44) != pack.knots[0].altitude_q12_km
            || gi32(bytes, 48) != pack.knots[count as usize - 1].altitude_q12_km
        {
            return Err(EnvironmentError::Identity);
        }
        pack.validate()?;
        Ok(pack)
    }

    pub fn sample(&self, altitude_q12_km: i32) -> Result<AtmosphereSample, EnvironmentError> {
        self.validate()?;
        let active = &self.knots[..self.count as usize];
        if altitude_q12_km < active[0].altitude_q12_km {
            return Err(EnvironmentError::Coverage);
        }
        if altitude_q12_km > active[active.len() - 1].altitude_q12_km {
            return if self.zero_above_last {
                Ok(AtmosphereSample::default())
            } else {
                Err(EnvironmentError::Coverage)
            };
        }
        let mut upper = 1usize;
        while upper < active.len() && altitude_q12_km > active[upper].altitude_q12_km {
            upper += 1;
        }
        let lower = upper.saturating_sub(1);
        if active[lower].altitude_q12_km == altitude_q12_km || lower == upper {
            return Ok(sample_from_knot(active[lower]));
        }
        let lo = active[lower];
        let hi = active[upper];
        let numerator = (altitude_q12_km - lo.altitude_q12_km) as u32;
        let denominator = (hi.altitude_q12_km - lo.altitude_q12_km) as u32;
        let mut status = NumericStatus::CLEAR;
        let sample = AtmosphereSample {
            density_q28_kg_m3: interpolate_i32(
                lo.density_q28_kg_m3,
                hi.density_q28_kg_m3,
                numerator,
                denominator,
                &mut status,
            ),
            pressure_q14_pa: interpolate_i32(
                lo.pressure_q14_pa,
                hi.pressure_q14_pa,
                numerator,
                denominator,
                &mut status,
            ),
            temperature_q16_k: interpolate_i32(
                lo.temperature_q16_k,
                hi.temperature_q16_k,
                numerator,
                denominator,
                &mut status,
            ),
            speed_of_sound_q16_m_s: interpolate_i32(
                lo.speed_of_sound_q16_m_s,
                hi.speed_of_sound_q16_m_s,
                numerator,
                denominator,
                &mut status,
            ),
            wind_enu_q19_m_s: FixedVec3::new(
                interpolate_i32(
                    lo.wind_enu_q19_m_s.x(),
                    hi.wind_enu_q19_m_s.x(),
                    numerator,
                    denominator,
                    &mut status,
                ),
                interpolate_i32(
                    lo.wind_enu_q19_m_s.y(),
                    hi.wind_enu_q19_m_s.y(),
                    numerator,
                    denominator,
                    &mut status,
                ),
                interpolate_i32(
                    lo.wind_enu_q19_m_s.z(),
                    hi.wind_enu_q19_m_s.z(),
                    numerator,
                    denominator,
                    &mut status,
                ),
            ),
        };
        if status.is_clear() {
            Ok(sample)
        } else {
            Err(EnvironmentError::Numeric)
        }
    }
}

fn sample_from_knot(knot: AtmosphereKnot) -> AtmosphereSample {
    AtmosphereSample {
        density_q28_kg_m3: knot.density_q28_kg_m3,
        pressure_q14_pa: knot.pressure_q14_pa,
        temperature_q16_k: knot.temperature_q16_k,
        speed_of_sound_q16_m_s: knot.speed_of_sound_q16_m_s,
        wind_enu_q19_m_s: knot.wind_enu_q19_m_s,
    }
}

fn rounded_i64(value: i64, denominator: i64, status: &mut NumericStatus) -> i32 {
    if denominator <= 0 {
        status.record(NumericFault::InvalidInput);
        return 0;
    }
    let half = denominator / 2;
    let rounded = if value >= 0 {
        (value + half) / denominator
    } else {
        (value - half) / denominator
    };
    if rounded < i32::MIN as i64 || rounded > i32::MAX as i64 {
        status.record(NumericFault::Saturation);
        0
    } else {
        rounded as i32
    }
}

fn cordic_atan2_q28(y: i32, x: i32, status: &mut NumericStatus) -> i32 {
    if x == 0 && y == 0 {
        status.record(NumericFault::InvalidInput);
        return 0;
    }
    if x == 0 {
        return if y >= 0 { HALF_PI_Q28 } else { -HALF_PI_Q28 };
    }
    if y == 0 {
        return if x > 0 { 0 } else { PI_Q28 };
    }
    let mut vx = x as i64;
    let mut vy = y as i64;
    let mut angle = 0i64;
    if vx < 0 {
        let positive_y = vy >= 0;
        vx = -vx;
        vy = -vy;
        angle = if positive_y {
            PI_Q28 as i64
        } else {
            -(PI_Q28 as i64)
        };
    }
    for (index, increment) in CORDIC_ATAN_Q28.iter().enumerate() {
        let old_x = vx;
        if vy > 0 {
            vx += vy >> index;
            vy -= old_x >> index;
            angle += *increment as i64;
        } else {
            vx -= vy >> index;
            vy += old_x >> index;
            angle -= *increment as i64;
        }
    }
    rounded_i64(angle, 1, status)
}

pub(crate) fn cordic_sin_cos_q30(mut angle_q28: i32, status: &mut NumericStatus) -> (i32, i32) {
    while angle_q28 > PI_Q28 {
        angle_q28 -= PI_Q28 * 2;
    }
    while angle_q28 < -PI_Q28 {
        angle_q28 += PI_Q28 * 2;
    }
    let mut sign = 1i32;
    if angle_q28 > HALF_PI_Q28 {
        angle_q28 -= PI_Q28;
        sign = -1;
    } else if angle_q28 < -HALF_PI_Q28 {
        angle_q28 += PI_Q28;
        sign = -1;
    }
    let mut x = CORDIC_GAIN_Q30 as i64;
    let mut y = 0i64;
    let mut z = angle_q28 as i64;
    for (index, increment) in CORDIC_ATAN_Q28.iter().enumerate() {
        let old_x = x;
        if z >= 0 {
            x -= y >> index;
            y += old_x >> index;
            z -= *increment as i64;
        } else {
            x += y >> index;
            y -= old_x >> index;
            z += *increment as i64;
        }
    }
    (
        rounded_i64(y * sign as i64, 1, status),
        rounded_i64(x * sign as i64, 1, status),
    )
}

/// Bounded five-iteration WGS 84 geodetic conversion.
pub fn ecef_to_geodetic(position: GlobalPositionVec) -> Result<GeodeticState, EnvironmentError> {
    let mut status = NumericStatus::CLEAR;
    let p = magnitude3_floor(position.x(), position.y(), 0, &mut status);
    if p > i32::MAX as u32 {
        status.record(NumericFault::Saturation);
    }
    let p = p as i32;
    if !status.is_clear() {
        return Err(EnvironmentError::Numeric);
    }
    if p == 0 {
        if position.z() == 0 {
            return Err(EnvironmentError::Range);
        }
        return Ok(GeodeticState {
            latitude_q28_rad: if position.z() > 0 {
                HALF_PI_Q28
            } else {
                -HALF_PI_Q28
            },
            longitude_q28_rad: 0,
            height_q12_km: position.z().abs() - WGS84_SEMI_MINOR_Q12_KM,
        });
    }
    if position.z() == 0 {
        return Ok(GeodeticState {
            latitude_q28_rad: 0,
            longitude_q28_rad: cordic_atan2_q28(position.y(), position.x(), &mut status),
            height_q12_km: p - WGS84_SEMI_MAJOR_Q12_KM,
        });
    }
    let longitude = cordic_atan2_q28(position.y(), position.x(), &mut status);
    let one_minus_e2 = (1 << 30) - WGS84_E2_Q30;
    let initial_denominator = multiply_scaled(p, one_minus_e2, 30, &mut status);
    let mut latitude = cordic_atan2_q28(position.z(), initial_denominator, &mut status);
    let mut height = 0i32;
    for _ in 0..5 {
        let (sin_lat, cos_lat) = cordic_sin_cos_q30(latitude, &mut status);
        if cos_lat == 0 {
            let sign = if position.z() >= 0 { 1 } else { -1 };
            latitude = sign * HALF_PI_Q28;
            height = position.z().abs() - WGS84_SEMI_MINOR_Q12_KM;
            break;
        }
        let sin2 = multiply_scaled(sin_lat, sin_lat, 30, &mut status);
        let root_term = subtract(
            1 << 30,
            multiply_scaled(WGS84_E2_Q30, sin2, 30, &mut status),
            &mut status,
        );
        let root = crate::numeric::sqrt_floor_scaled_u32(root_term.max(0) as u32, 30, &mut status);
        if root == 0 || root > i32::MAX as u32 {
            status.record(NumericFault::InvalidInput);
            break;
        }
        let normal_radius = divide_scaled(WGS84_SEMI_MAJOR_Q12_KM, root as i32, 30, &mut status);
        let p_over_cos = divide_scaled(p, cos_lat, 30, &mut status);
        height = subtract(p_over_cos, normal_radius, &mut status);
        let normal_over_total = divide_scaled(
            normal_radius,
            add(normal_radius, height, &mut status),
            30,
            &mut status,
        );
        let latitude_denominator = multiply_scaled(
            p,
            subtract(
                1 << 30,
                multiply_scaled(WGS84_E2_Q30, normal_over_total, 30, &mut status),
                &mut status,
            ),
            30,
            &mut status,
        );
        latitude = cordic_atan2_q28(position.z(), latitude_denominator, &mut status);
    }
    if !status.is_clear() {
        return Err(EnvironmentError::Numeric);
    }
    Ok(GeodeticState {
        latitude_q28_rad: latitude,
        longitude_q28_rad: longitude,
        height_q12_km: height,
    })
}

/// Central plus J2 gravity, expressed in the Earth-aligned frame.
pub fn central_j2_gravity(
    earth: &EarthModelPack,
    position: GlobalPositionVec,
) -> Result<GlobalAccelerationVec, EnvironmentError> {
    if earth.mu_q8_km3_s2 != WGS84_MU_Q8_KM3_S2
        || earth.j2_q30 != WGS84_J2_Q30
        || earth.semi_major_q12_km != WGS84_SEMI_MAJOR_Q12_KM
    {
        return Err(EnvironmentError::Identity);
    }
    let mut status = NumericStatus::CLEAR;
    let radius = magnitude3_floor(position.x(), position.y(), position.z(), &mut status);
    if radius == 0 || radius > i32::MAX as u32 {
        return Err(EnvironmentError::Range);
    }
    let radius = radius as i32;
    let r2_q8 = (radius as i64 * radius as i64 + (1 << 15)) >> 16;
    let gravity_q28 = rounded_i64(earth.mu_q8_km3_s2 as i64 * (1i64 << 28), r2_q8, &mut status);
    let x_ratio = rounded_i64(
        position.x() as i64 * (1i64 << 30),
        radius as i64,
        &mut status,
    );
    let y_ratio = rounded_i64(
        position.y() as i64 * (1i64 << 30),
        radius as i64,
        &mut status,
    );
    let z_ratio = rounded_i64(
        position.z() as i64 * (1i64 << 30),
        radius as i64,
        &mut status,
    );
    let re_ratio = rounded_i64(
        earth.semi_major_q12_km as i64 * (1i64 << 30),
        radius as i64,
        &mut status,
    );
    let re2 = multiply_scaled(re_ratio, re_ratio, 30, &mut status);
    let z2 = multiply_scaled(z_ratio, z_ratio, 30, &mut status);
    let j2_re2 = multiply_scaled(earth.j2_q30, re2, 30, &mut status);
    let factor = rounded_i64(j2_re2 as i64 * 3, 2, &mut status);
    let five_z_factor = rounded_i64(
        multiply_scaled(factor, z2, 30, &mut status) as i64 * 5,
        1,
        &mut status,
    );
    let correction_xy = subtract(
        1 << 30,
        subtract(five_z_factor, factor, &mut status),
        &mut status,
    );
    let correction_z = subtract(
        1 << 30,
        subtract(
            five_z_factor,
            rounded_i64(factor as i64 * 3, 1, &mut status),
            &mut status,
        ),
        &mut status,
    );
    let axis = |ratio: i32, correction: i32, status: &mut NumericStatus| {
        -multiply_scaled(
            multiply_scaled(gravity_q28, ratio, 30, status),
            correction,
            30,
            status,
        )
    };
    let result = GlobalAccelerationVec::new(
        axis(x_ratio, correction_xy, &mut status),
        axis(y_ratio, correction_xy, &mut status),
        axis(z_ratio, correction_z, &mut status),
    );
    if status.is_clear() {
        Ok(result)
    } else {
        Err(EnvironmentError::Numeric)
    }
}

/// Apparent acceleration terms for propagation in rotating ECEF.
pub fn ecef_rotating_terms(
    position: GlobalPositionVec,
    velocity: GlobalVelocityVec,
    angular_acceleration_ecef_q28: GlobalAccelerationVec,
) -> Result<GlobalAccelerationVec, EnvironmentError> {
    let mut status = NumericStatus::CLEAR;
    let omega_q24 = ((EARTH_ROTATION_Q30_RAD_S + 32) >> 6).max(1);
    let omega = FixedVec3::<24>::new(0, 0, omega_q24);
    let coriolis =
        cross_mixed_scaled::<24, 24, 28>(omega, velocity, &mut status).scale::<0>(-2, &mut status);
    let sweep_velocity = cross_mixed_scaled::<24, 12, 24>(omega, position, &mut status);
    let centrifugal = cross_mixed_scaled::<24, 24, 28>(omega, sweep_velocity, &mut status)
        .scale::<0>(-1, &mut status);
    let euler =
        cross_mixed_scaled::<28, 12, 28>(angular_acceleration_ecef_q28, position, &mut status)
            .scale::<0>(-1, &mut status);
    let result = coriolis
        .checked_add(centrifugal, &mut status)
        .checked_add(euler, &mut status);
    if status.is_clear() {
        Ok(result)
    } else {
        Err(EnvironmentError::Numeric)
    }
}

fn p16(output: &mut [u8], at: usize, value: u16) {
    output[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn p32(output: &mut [u8], at: usize, value: u32) {
    output[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn pi32(output: &mut [u8], at: usize, value: i32) {
    p32(output, at, value as u32);
}

fn g16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn g32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn gi32(bytes: &[u8], at: usize) -> i32 {
    g32(bytes, at) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase10_contract::EarthModelPack;

    fn earth() -> EarthModelPack {
        EarthModelPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kem10")).unwrap()
    }

    fn atmosphere() -> CompiledAtmospherePack {
        CompiledAtmospherePack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kat10"))
            .unwrap()
    }

    #[test]
    fn generated_atmosphere_is_strict_and_interpolates() {
        let pack = atmosphere();
        assert_eq!(pack.count, 21);
        let sea = pack.sample(0).unwrap();
        assert!((sea.density_q28_kg_m3 - 328_833_433).abs() <= 1);
        let mid = pack.sample(5 << 11).unwrap();
        assert!(mid.density_q28_kg_m3 > 0);
        assert_eq!(pack.sample(201 << 12).unwrap(), AtmosphereSample::default());
    }

    #[test]
    fn geodetic_equator_and_pole_are_bounded() {
        let equator =
            ecef_to_geodetic(GlobalPositionVec::new(WGS84_SEMI_MAJOR_Q12_KM, 0, 0)).unwrap();
        assert_eq!(equator.latitude_q28_rad, 0);
        assert_eq!(equator.longitude_q28_rad, 0);
        assert!(equator.height_q12_km.abs() <= 2);
        let pole = ecef_to_geodetic(GlobalPositionVec::new(0, 0, WGS84_SEMI_MINOR_Q12_KM)).unwrap();
        assert!((pole.latitude_q28_rad - HALF_PI_Q28).abs() <= 2);
        assert!(pole.height_q12_km.abs() <= 2);
    }

    #[test]
    fn central_j2_surface_gravity_is_plausible() {
        let acceleration = central_j2_gravity(
            &earth(),
            GlobalPositionVec::new(WGS84_SEMI_MAJOR_Q12_KM, 0, 0),
        )
        .unwrap();
        // 9.780 m/s2 in km/s2 Q28.
        assert!(
            (acceleration.x() + 2_634_479).abs() < 2_000,
            "{}",
            acceleration.x()
        );
        assert_eq!(acceleration.y(), 0);
        assert_eq!(acceleration.z(), 0);
    }

    #[test]
    fn rotating_terms_show_equatorial_centrifugal_acceleration() {
        let terms = ecef_rotating_terms(
            GlobalPositionVec::new(WGS84_SEMI_MAJOR_Q12_KM, 0, 0),
            GlobalVelocityVec::ZERO,
            GlobalAccelerationVec::ZERO,
        )
        .unwrap();
        assert!(terms.x() > 8_000);
        assert_eq!(terms.y(), 0);
        assert_eq!(terms.z(), 0);
    }
}
