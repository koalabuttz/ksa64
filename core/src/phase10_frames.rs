//! Deterministic local-ENU, ECEF, and GCRF state transport.

use crate::numeric::{NumericFault, NumericStatus};
use crate::phase10_contract::{Phase10ContractError, ReferenceFrameId, TransformPack};
use crate::phase10_numeric::{
    interpolate_i32, GlobalAccelerationVec, GlobalAngularRateVec, GlobalKinematicState,
    GlobalPositionVec, GlobalVelocityVec, MissionTimeQ16,
};
use crate::phase8_numeric::{BodyToEnuQuaternion, EnuPosition, EnuVelocity};
use crate::spatial_numeric::{cross_mixed_scaled, FixedVec3, QuaternionQ30};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct LocalAnchor {
    pub identity: u32,
    pub origin_ecef: GlobalPositionVec,
    pub enu_to_ecef: QuaternionQ30,
    /// Required at an exact pole; zero is the accepted default meridian.
    pub reference_meridian_q28_rad: i32,
}

impl LocalAnchor {
    pub fn validate(self) -> Result<Self, FrameError> {
        if self.identity == 0 {
            return Err(FrameError::Identity);
        }
        let mut status = NumericStatus::CLEAR;
        let normalized = self.enu_to_ecef.normalized(&mut status);
        if !status.is_clear() {
            return Err(FrameError::Numeric);
        }
        Ok(Self {
            enu_to_ecef: normalized,
            ..self
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct LocalKinematicState {
    pub position: EnuPosition,
    pub velocity: EnuVelocity,
    pub attitude: BodyToEnuQuaternion,
    pub angular_rate: GlobalAngularRateVec,
    pub time: MissionTimeQ16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct InterpolatedTransform {
    pub time: MissionTimeQ16,
    pub ecef_to_gcrf: QuaternionQ30,
    pub angular_velocity_gcrf: GlobalAngularRateVec,
    pub angular_acceleration_gcrf: GlobalAccelerationVec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    Identity,
    Coverage,
    Numeric,
    Scale,
    Contract(Phase10ContractError),
}

impl From<Phase10ContractError> for FrameError {
    fn from(value: Phase10ContractError) -> Self {
        Self::Contract(value)
    }
}

fn checked_i64_to_i32(value: i64, status: &mut NumericStatus) -> i32 {
    if value < i32::MIN as i64 || value > i32::MAX as i64 {
        status.record(NumericFault::Saturation);
        0
    } else {
        value as i32
    }
}

fn rounded_divide(value: i64, denominator: i64, status: &mut NumericStatus) -> i32 {
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
    checked_i64_to_i32(rounded, status)
}

fn enu_position_to_global(value: EnuPosition, status: &mut NumericStatus) -> GlobalPositionVec {
    // metres Q13 -> kilometres Q12 = raw / 2000.
    GlobalPositionVec::new(
        rounded_divide(value.x() as i64, 2_000, status),
        rounded_divide(value.y() as i64, 2_000, status),
        rounded_divide(value.z() as i64, 2_000, status),
    )
}

fn global_position_to_enu(value: GlobalPositionVec, status: &mut NumericStatus) -> EnuPosition {
    EnuPosition::new(
        checked_i64_to_i32(value.x() as i64 * 2_000, status),
        checked_i64_to_i32(value.y() as i64 * 2_000, status),
        checked_i64_to_i32(value.z() as i64 * 2_000, status),
    )
}

fn enu_velocity_to_global(value: EnuVelocity, status: &mut NumericStatus) -> GlobalVelocityVec {
    // m/s Q19 -> km/s Q24 = raw * 4 / 125.
    GlobalVelocityVec::new(
        rounded_divide(value.x() as i64 * 4, 125, status),
        rounded_divide(value.y() as i64 * 4, 125, status),
        rounded_divide(value.z() as i64 * 4, 125, status),
    )
}

fn global_velocity_to_enu(value: GlobalVelocityVec, status: &mut NumericStatus) -> EnuVelocity {
    // km/s Q24 -> m/s Q19 = raw * 125 / 4.
    EnuVelocity::new(
        rounded_divide(value.x() as i64 * 125, 4, status),
        rounded_divide(value.y() as i64 * 125, 4, status),
        rounded_divide(value.z() as i64 * 125, 4, status),
    )
}

fn choose_quaternion_sign(a: QuaternionQ30, b: QuaternionQ30) -> QuaternionQ30 {
    let dot = a.w() as i64 * b.w() as i64
        + a.x() as i64 * b.x() as i64
        + a.y() as i64 * b.y() as i64
        + a.z() as i64 * b.z() as i64;
    if dot < 0 {
        QuaternionQ30::new(-b.w(), -b.x(), -b.y(), -b.z())
    } else {
        b
    }
}

pub fn interpolate_transform(
    pack: &TransformPack,
    time: MissionTimeQ16,
) -> Result<InterpolatedTransform, FrameError> {
    pack.validate()?;
    if !pack.covers(time) {
        return Err(FrameError::Coverage);
    }
    let elapsed = time.raw() - pack.knots[0].time.raw();
    let mut index = (elapsed / pack.knot_spacing_q16) as usize;
    if index >= pack.count as usize - 1 {
        index = pack.count as usize - 2;
    }
    let first = pack.knots[index];
    let second = pack.knots[index + 1];
    let numerator = time.raw() - first.time.raw();
    let denominator = second.time.raw() - first.time.raw();
    let second_q = choose_quaternion_sign(first.ecef_to_gcrf, second.ecef_to_gcrf);
    let mut status = NumericStatus::CLEAR;
    let quaternion = QuaternionQ30::new(
        interpolate_i32(
            first.ecef_to_gcrf.w(),
            second_q.w(),
            numerator,
            denominator,
            &mut status,
        ),
        interpolate_i32(
            first.ecef_to_gcrf.x(),
            second_q.x(),
            numerator,
            denominator,
            &mut status,
        ),
        interpolate_i32(
            first.ecef_to_gcrf.y(),
            second_q.y(),
            numerator,
            denominator,
            &mut status,
        ),
        interpolate_i32(
            first.ecef_to_gcrf.z(),
            second_q.z(),
            numerator,
            denominator,
            &mut status,
        ),
    )
    .normalized(&mut status);
    let omega = interpolate_vec(
        first.angular_velocity_gcrf,
        second.angular_velocity_gcrf,
        numerator,
        denominator,
        &mut status,
    );
    let alpha = interpolate_vec(
        first.angular_acceleration_gcrf,
        second.angular_acceleration_gcrf,
        numerator,
        denominator,
        &mut status,
    );
    if !status.is_clear() {
        return Err(FrameError::Numeric);
    }
    Ok(InterpolatedTransform {
        time,
        ecef_to_gcrf: quaternion,
        angular_velocity_gcrf: omega,
        angular_acceleration_gcrf: alpha,
    })
}

fn interpolate_vec<const F: u8>(
    a: FixedVec3<F>,
    b: FixedVec3<F>,
    numerator: u32,
    denominator: u32,
    status: &mut NumericStatus,
) -> FixedVec3<F> {
    FixedVec3::new(
        interpolate_i32(a.x(), b.x(), numerator, denominator, status),
        interpolate_i32(a.y(), b.y(), numerator, denominator, status),
        interpolate_i32(a.z(), b.z(), numerator, denominator, status),
    )
}

pub fn local_to_ecef(
    anchor: LocalAnchor,
    state: LocalKinematicState,
) -> Result<GlobalKinematicState, FrameError> {
    let anchor = anchor.validate()?;
    let mut status = NumericStatus::CLEAR;
    let offset = anchor.enu_to_ecef.rotate(
        enu_position_to_global(state.position, &mut status),
        &mut status,
    );
    let position = anchor.origin_ecef.checked_add(offset, &mut status);
    let velocity = anchor.enu_to_ecef.rotate(
        enu_velocity_to_global(state.velocity, &mut status),
        &mut status,
    );
    let attitude = anchor
        .enu_to_ecef
        .hamilton(state.attitude, &mut status)
        .normalized(&mut status);
    if !status.is_clear() {
        return Err(FrameError::Numeric);
    }
    Ok(GlobalKinematicState::new(
        position,
        velocity,
        attitude,
        state.angular_rate,
        state.time,
    ))
}

pub fn ecef_to_local(
    anchor: LocalAnchor,
    state: GlobalKinematicState,
) -> Result<LocalKinematicState, FrameError> {
    let anchor = anchor.validate()?;
    let inverse = anchor.enu_to_ecef.conjugate();
    let mut status = NumericStatus::CLEAR;
    let offset_ecef = state.position.checked_sub(anchor.origin_ecef, &mut status);
    let position = global_position_to_enu(inverse.rotate(offset_ecef, &mut status), &mut status);
    let velocity = global_velocity_to_enu(inverse.rotate(state.velocity, &mut status), &mut status);
    let attitude = inverse
        .hamilton(state.attitude, &mut status)
        .normalized(&mut status);
    if !status.is_clear() {
        return Err(FrameError::Numeric);
    }
    Ok(LocalKinematicState {
        position,
        velocity,
        attitude,
        angular_rate: state.angular_rate,
        time: state.time,
    })
}

pub fn ecef_to_gcrf(
    transform: InterpolatedTransform,
    state: GlobalKinematicState,
) -> Result<GlobalKinematicState, FrameError> {
    if transform.time != state.time {
        return Err(FrameError::Identity);
    }
    let mut status = NumericStatus::CLEAR;
    let position = transform.ecef_to_gcrf.rotate(state.position, &mut status);
    let rotated_velocity = transform.ecef_to_gcrf.rotate(state.velocity, &mut status);
    let sweep =
        cross_mixed_scaled::<24, 12, 24>(transform.angular_velocity_gcrf, position, &mut status);
    let velocity = rotated_velocity.checked_add(sweep, &mut status);
    let attitude = transform
        .ecef_to_gcrf
        .hamilton(state.attitude, &mut status)
        .normalized(&mut status);
    if !status.is_clear() {
        return Err(FrameError::Numeric);
    }
    Ok(GlobalKinematicState::new(
        position,
        velocity,
        attitude,
        state.angular_rate,
        state.time,
    ))
}

pub fn gcrf_to_ecef(
    transform: InterpolatedTransform,
    state: GlobalKinematicState,
) -> Result<GlobalKinematicState, FrameError> {
    if transform.time != state.time {
        return Err(FrameError::Identity);
    }
    let inverse = transform.ecef_to_gcrf.conjugate();
    let mut status = NumericStatus::CLEAR;
    let sweep = cross_mixed_scaled::<24, 12, 24>(
        transform.angular_velocity_gcrf,
        state.position,
        &mut status,
    );
    let velocity_without_sweep = state.velocity.checked_sub(sweep, &mut status);
    let position = inverse.rotate(state.position, &mut status);
    let velocity = inverse.rotate(velocity_without_sweep, &mut status);
    let attitude = inverse
        .hamilton(state.attitude, &mut status)
        .normalized(&mut status);
    if !status.is_clear() {
        return Err(FrameError::Numeric);
    }
    Ok(GlobalKinematicState::new(
        position,
        velocity,
        attitude,
        state.angular_rate,
        state.time,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct FrameOwnedState {
    pub frame: ReferenceFrameId,
    pub state: GlobalKinematicState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase10_contract::{TransformPack, KFT10_LENGTH};

    fn transform_pack() -> TransformPack {
        TransformPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kft10")).unwrap()
    }

    #[test]
    fn generated_transform_pack_is_strict_and_covers_two_hours() {
        assert_eq!(
            include_bytes!("../../phase10/generated/ksa-g10r.kft10").len(),
            KFT10_LENGTH
        );
        let pack = transform_pack();
        assert_eq!(pack.count, 121);
        assert!(pack.covers(MissionTimeQ16::from_raw(7_200 << 16).unwrap()));
    }

    #[test]
    fn ecef_gcrf_round_trip_preserves_a_surface_state() {
        let time = MissionTimeQ16::from_raw(90 << 16).unwrap();
        let transform = interpolate_transform(&transform_pack(), time).unwrap();
        let state = GlobalKinematicState::new(
            GlobalPositionVec::new(26_124_849, 0, 0),
            GlobalVelocityVec::ZERO,
            QuaternionQ30::IDENTITY,
            GlobalAngularRateVec::new(1, -2, 3),
            time,
        );
        let inertial = ecef_to_gcrf(transform, state).unwrap();
        assert!(inertial.velocity.x().abs() + inertial.velocity.y().abs() > 1_000);
        let round_trip = gcrf_to_ecef(transform, inertial).unwrap();
        assert!((round_trip.position.x() - state.position.x()).abs() <= 2);
        assert!((round_trip.position.y() - state.position.y()).abs() <= 2);
        assert!((round_trip.position.z() - state.position.z()).abs() <= 2);
        assert!((round_trip.velocity.x() - state.velocity.x()).abs() <= 2);
        assert!((round_trip.velocity.y() - state.velocity.y()).abs() <= 2);
        assert!((round_trip.velocity.z() - state.velocity.z()).abs() <= 2);
        assert_eq!(round_trip.angular_rate, state.angular_rate);
        assert_eq!(round_trip.time, state.time);
    }

    #[test]
    fn local_round_trip_preserves_bounded_recovery_state() {
        let anchor = LocalAnchor {
            identity: 1,
            origin_ecef: GlobalPositionVec::new(26_124_849, 0, 0),
            enu_to_ecef: QuaternionQ30::new(759_250_125, 759_250_125, 0, 0),
            reference_meridian_q28_rad: 0,
        };
        let local = LocalKinematicState {
            position: EnuPosition::new(819_200, -409_600, 204_800),
            velocity: EnuVelocity::new(52_429, -104_858, 157_286),
            attitude: QuaternionQ30::IDENTITY,
            angular_rate: GlobalAngularRateVec::new(10, 20, -30),
            time: MissionTimeQ16::from_raw(10 << 16).unwrap(),
        };
        let global = local_to_ecef(anchor, local).unwrap();
        let round_trip = ecef_to_local(anchor, global).unwrap();
        assert!((round_trip.position.x() - local.position.x()).abs() <= 2_000);
        assert!((round_trip.position.y() - local.position.y()).abs() <= 2_000);
        assert!((round_trip.position.z() - local.position.z()).abs() <= 2_000);
        assert!((round_trip.velocity.x() - local.velocity.x()).abs() <= 32);
        assert!((round_trip.velocity.y() - local.velocity.y()).abs() <= 32);
        assert!((round_trip.velocity.z() - local.velocity.z()).abs() <= 32);
        assert_eq!(round_trip.angular_rate, local.angular_rate);
        assert_eq!(round_trip.time, local.time);
    }
}
