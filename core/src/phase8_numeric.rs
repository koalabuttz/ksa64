//! Generated fixed-point quantities for the Phase 8 hobby-spatial profile.

use crate::spatial_numeric::{FixedVec3, QuaternionQ30};

include!("../../phase8/generated/numeric_v1.rs");

macro_rules! spatial_fixed_i32 {
    ($name:ident, $fractional_bits:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(i32);

        impl $name {
            pub const FRACTIONAL_BITS: u8 = $fractional_bits;
            pub const ZERO: Self = Self(0);

            #[inline]
            pub const fn from_raw(raw: i32) -> Self {
                Self(raw)
            }

            #[inline]
            pub const fn raw(self) -> i32 {
                self.0
            }
        }
    };
}

spatial_fixed_i32!(SpatialTime, SPATIAL_TIME_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialPosition, SPATIAL_POSITION_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialVelocity, SPATIAL_VELOCITY_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialAcceleration, SPATIAL_ACCELERATION_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialMass, SPATIAL_MASS_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialForce, SPATIAL_FORCE_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialMomentArm, SPATIAL_MOMENT_ARM_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialInertia, SPATIAL_INERTIA_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialTorque, SPATIAL_TORQUE_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialAngularRate, SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialCoefficient, SPATIAL_COEFFICIENT_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialStaticMargin, SPATIAL_STATIC_MARGIN_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialAngle, SPATIAL_ANGLE_FRACTIONAL_BITS);
spatial_fixed_i32!(SpatialWind, SPATIAL_WIND_FRACTIONAL_BITS);

pub type EnuPosition = FixedVec3<SPATIAL_POSITION_FRACTIONAL_BITS>;
pub type EnuVelocity = FixedVec3<SPATIAL_VELOCITY_FRACTIONAL_BITS>;
pub type EnuAcceleration = FixedVec3<SPATIAL_ACCELERATION_FRACTIONAL_BITS>;
pub type EnuForce = FixedVec3<SPATIAL_FORCE_FRACTIONAL_BITS>;
pub type BodyTorque = FixedVec3<SPATIAL_TORQUE_FRACTIONAL_BITS>;
pub type BodyAngularRate = FixedVec3<SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS>;
pub type EnuWind = FixedVec3<SPATIAL_WIND_FRACTIONAL_BITS>;
pub type BodyToEnuQuaternion = QuaternionQ30;

pub const SPATIAL_POWERED_STEP: SpatialTime = SpatialTime::from_raw(SPATIAL_POWERED_STEP_RAW);
pub const SPATIAL_COAST_TRANSLATION_STEP: SpatialTime =
    SpatialTime::from_raw(SPATIAL_COAST_TRANSLATION_STEP_RAW);
pub const SPATIAL_COAST_ATTITUDE_STEP: SpatialTime =
    SpatialTime::from_raw(SPATIAL_COAST_ATTITUDE_STEP_RAW);
pub const SPATIAL_RECOVERY_STEP: SpatialTime = SpatialTime::from_raw(SPATIAL_RECOVERY_STEP_RAW);

pub const fn hobby_spatial_numeric_contract_is_valid() -> bool {
    SPATIAL_TIME_FRACTIONAL_BITS == 18
        && SPATIAL_POSITION_FRACTIONAL_BITS == 13
        && SPATIAL_VELOCITY_FRACTIONAL_BITS == 19
        && SPATIAL_ACCELERATION_FRACTIONAL_BITS == 19
        && SPATIAL_MASS_FRACTIONAL_BITS == 21
        && SPATIAL_FORCE_FRACTIONAL_BITS == 13
        && SPATIAL_MOMENT_ARM_FRACTIONAL_BITS == 28
        && SPATIAL_INERTIA_FRACTIONAL_BITS == 19
        && SPATIAL_TORQUE_FRACTIONAL_BITS == 12
        && SPATIAL_ANGULAR_RATE_FRACTIONAL_BITS == 24
        && SPATIAL_COEFFICIENT_FRACTIONAL_BITS == 24
        && SPATIAL_STATIC_MARGIN_FRACTIONAL_BITS == 24
        && SPATIAL_ANGLE_FRACTIONAL_BITS == 28
        && SPATIAL_WIND_FRACTIONAL_BITS == 22
        && SPATIAL_QUATERNION_FRACTIONAL_BITS == 30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_contract_and_cadences_are_frozen() {
        assert!(hobby_spatial_numeric_contract_is_valid());
        assert_eq!(SPATIAL_POWERED_STEP.raw(), 2_621);
        assert_eq!(SPATIAL_COAST_TRANSLATION_STEP.raw(), 5_243);
        assert_eq!(SPATIAL_COAST_ATTITUDE_STEP.raw(), 2_621);
        assert_eq!(SPATIAL_RECOVERY_STEP.raw(), 13_107);
    }

    #[test]
    fn scalar_wrappers_remain_one_word() {
        assert_eq!(core::mem::size_of::<SpatialPosition>(), 4);
        assert_eq!(core::mem::size_of::<SpatialInertia>(), 4);
        assert_eq!(core::mem::size_of::<SpatialTorque>(), 4);
        assert_eq!(core::mem::size_of::<SpatialAngularRate>(), 4);
    }

    #[test]
    fn vector_aliases_preserve_declared_scales() {
        assert_eq!(core::mem::size_of::<EnuPosition>(), 12);
        assert_eq!(core::mem::size_of::<BodyToEnuQuaternion>(), 16);
        assert_eq!(
            (
                EnuVelocity::new(1, 2, 3).x(),
                EnuVelocity::new(1, 2, 3).y(),
                EnuVelocity::new(1, 2, 3).z()
            ),
            (1, 2, 3)
        );
    }
}
