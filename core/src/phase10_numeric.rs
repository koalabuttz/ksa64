//! Fixed-point contract for Phase 10 global Earth flight.
//!
//! Global translation reuses the Phase 5 operation order and scales. Body
//! geometry, forces, torques, and effectors retain their Phase 8/9.5 units.

use crate::numeric::{add, subtract, NumericFault, NumericStatus};
use crate::spatial_numeric::{FixedVec3, QuaternionQ30};

pub const GLOBAL_POSITION_FRACTIONAL_BITS: u8 = 12;
pub const GLOBAL_VELOCITY_FRACTIONAL_BITS: u8 = 24;
pub const GLOBAL_ACCELERATION_FRACTIONAL_BITS: u8 = 28;
pub const GLOBAL_QUATERNION_FRACTIONAL_BITS: u8 = 30;
pub const GLOBAL_ANGULAR_RATE_FRACTIONAL_BITS: u8 = 24;
pub const GLOBAL_TIME_FRACTIONAL_BITS: u8 = 16;

pub const GLOBAL_MAX_ALTITUDE_KM: i32 = 2_000;
pub const GLOBAL_MIN_ALTITUDE_KM: i32 = -1;
pub const GLOBAL_MAX_SPEED_RAW: i32 = 201_326_592;
pub const GLOBAL_MAX_ACCELERATION_RAW: i32 = 53_687_091;
pub const GLOBAL_MAX_DURATION_RAW: u32 = 943_718_400;
pub const GLOBAL_MAX_WET_MASS_KG: i32 = 800;

pub const GLOBAL_POWERED_STEP_Q16: u32 = 512;
pub const GLOBAL_COAST_STEP_Q16: u32 = 2_048;
pub const GLOBAL_RCS_QUANTUM_Q16: u32 = 256;
pub const GLOBAL_AVIONICS_PERIOD_Q16: u32 = 2_048;

pub type GlobalPositionVec = FixedVec3<GLOBAL_POSITION_FRACTIONAL_BITS>;
pub type GlobalVelocityVec = FixedVec3<GLOBAL_VELOCITY_FRACTIONAL_BITS>;
pub type GlobalAccelerationVec = FixedVec3<GLOBAL_ACCELERATION_FRACTIONAL_BITS>;
pub type GlobalAngularRateVec = FixedVec3<GLOBAL_ANGULAR_RATE_FRACTIONAL_BITS>;
pub type GlobalAttitude = QuaternionQ30;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct MissionTimeQ16(u32);

impl MissionTimeQ16 {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(GLOBAL_MAX_DURATION_RAW);

    pub const fn from_raw(raw: u32) -> Option<Self> {
        if raw <= GLOBAL_MAX_DURATION_RAW {
            Some(Self(raw))
        } else {
            None
        }
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub fn checked_add(self, delta_q16: u32, status: &mut NumericStatus) -> Self {
        match self.0.checked_add(delta_q16) {
            Some(raw) if raw <= GLOBAL_MAX_DURATION_RAW => Self(raw),
            _ => {
                status.record(NumericFault::Saturation);
                self
            }
        }
    }

    pub fn checked_sub(self, rhs: Self, status: &mut NumericStatus) -> u32 {
        match self.0.checked_sub(rhs.0) {
            Some(raw) => raw,
            None => {
                status.record(NumericFault::InvalidInput);
                0
            }
        }
    }

    /// Converts an exact Phase 8.5/9.5 Q18 event time into global Q16.
    pub const fn from_q18_exact(raw_q18: u32) -> Option<Self> {
        if raw_q18 & 3 != 0 {
            return None;
        }
        Self::from_raw(raw_q18 >> 2)
    }

    pub const fn to_q18_exact(self) -> u32 {
        self.0 << 2
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GlobalKinematicState {
    pub position: GlobalPositionVec,
    pub velocity: GlobalVelocityVec,
    pub attitude: GlobalAttitude,
    /// Physical body-relative-inertial angular rate expressed in body axes.
    pub angular_rate: GlobalAngularRateVec,
    pub time: MissionTimeQ16,
}

impl GlobalKinematicState {
    pub const fn new(
        position: GlobalPositionVec,
        velocity: GlobalVelocityVec,
        attitude: GlobalAttitude,
        angular_rate: GlobalAngularRateVec,
        time: MissionTimeQ16,
    ) -> Self {
        Self {
            position,
            velocity,
            attitude,
            angular_rate,
            time,
        }
    }
}

pub fn interpolate_i32(
    a: i32,
    b: i32,
    numerator: u32,
    denominator: u32,
    status: &mut NumericStatus,
) -> i32 {
    if denominator == 0 || numerator > denominator {
        status.record(NumericFault::InvalidInput);
        return a;
    }
    let delta = subtract(b, a, status) as i64;
    let scaled = delta * numerator as i64;
    let half = (denominator / 2) as i64;
    let rounded = if scaled >= 0 {
        (scaled + half) / denominator as i64
    } else {
        (scaled - half) / denominator as i64
    };
    if rounded < i32::MIN as i64 || rounded > i32::MAX as i64 {
        status.record(NumericFault::Saturation);
        return a;
    }
    add(a, rounded as i32, status)
}

/// Integrates one fixed-point rate with exact signed residual carry.
///
/// The returned value is an increment in the destination scale. `residual`
/// retains the sub-cell numerator left after rounding, preventing coherent
/// long-duration drift while leaving the public state representation unchanged.
pub fn integrate_with_residual(
    rate_raw: i32,
    dt_raw: u32,
    shift: u8,
    residual: &mut i64,
    status: &mut NumericStatus,
) -> i32 {
    if shift == 0 || shift >= 63 {
        status.record(NumericFault::InvalidInput);
        return 0;
    }
    let denominator = 1i64 << shift;
    let numerator = match i64::from(rate_raw)
        .checked_mul(i64::from(dt_raw))
        .and_then(|value| value.checked_add(*residual))
    {
        Some(value) => value,
        None => {
            status.record(NumericFault::Saturation);
            return 0;
        }
    };
    let half = denominator / 2;
    let quotient = if numerator >= 0 {
        (numerator + half) / denominator
    } else {
        (numerator - half) / denominator
    };
    if quotient < i64::from(i32::MIN) || quotient > i64::from(i32::MAX) {
        status.record(NumericFault::Saturation);
        return 0;
    }
    *residual = numerator - quotient * denominator;
    quotient as i32
}
pub const fn global_numeric_contract_is_valid() -> bool {
    GLOBAL_POSITION_FRACTIONAL_BITS == 12
        && GLOBAL_VELOCITY_FRACTIONAL_BITS == 24
        && GLOBAL_ACCELERATION_FRACTIONAL_BITS == 28
        && GLOBAL_QUATERNION_FRACTIONAL_BITS == 30
        && GLOBAL_ANGULAR_RATE_FRACTIONAL_BITS == 24
        && GLOBAL_TIME_FRACTIONAL_BITS == 16
        && GLOBAL_POWERED_STEP_Q16 * 128 == 65_536
        && GLOBAL_COAST_STEP_Q16 * 32 == 65_536
        && GLOBAL_RCS_QUANTUM_Q16 * 256 == 65_536
        && GLOBAL_AVIONICS_PERIOD_Q16 * 32 == 65_536
        && GLOBAL_MAX_DURATION_RAW >= 14_400 * 65_536
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_carry_prevents_long_duration_cell_loss() {
        let mut status = NumericStatus::CLEAR;
        let mut residual = 0i64;
        let mut total = 0i64;
        for _ in 0..3_200 {
            total += i64::from(integrate_with_residual(
                -66_327_286,
                2_048,
                28,
                &mut residual,
                &mut status,
            ));
        }
        let exact = i64::from(-66_327_286) * 2_048 * 3_200 / (1i64 << 28);
        assert!((total - exact).abs() <= 1);
        assert!(status.is_clear());
    }

    #[test]
    fn scales_and_exact_event_divisors_are_frozen() {
        assert!(global_numeric_contract_is_valid());
        assert_eq!(
            MissionTimeQ16::from_q18_exact(1_024).unwrap().raw(),
            GLOBAL_RCS_QUANTUM_Q16
        );
        assert_eq!(
            MissionTimeQ16::from_q18_exact(8_192).unwrap().raw(),
            GLOBAL_AVIONICS_PERIOD_Q16
        );
        assert_eq!(
            MissionTimeQ16::from_raw(GLOBAL_MAX_DURATION_RAW)
                .unwrap()
                .to_q18_exact(),
            GLOBAL_MAX_DURATION_RAW << 2
        );
        assert!(MissionTimeQ16::from_q18_exact(1_025).is_none());
    }

    #[test]
    fn interpolation_rounds_ties_away_from_zero() {
        let mut status = NumericStatus::CLEAR;
        assert_eq!(interpolate_i32(0, 3, 1, 2, &mut status), 2);
        assert_eq!(interpolate_i32(0, -3, 1, 2, &mut status), -2);
        assert!(status.is_clear());
    }
}
