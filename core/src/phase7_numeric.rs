//! Generated strong fixed-point types for the hobby vertical profile.

include!("../../phase7/generated/numeric_v1.rs");

macro_rules! hobby_fixed_i32 {
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

hobby_fixed_i32!(HobbyTime, HOBBY_TIME_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyAltitude, HOBBY_ALTITUDE_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyVelocity, HOBBY_VELOCITY_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyAcceleration, HOBBY_ACCELERATION_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyMass, HOBBY_MASS_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyForce, HOBBY_FORCE_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyMassFlow, HOBBY_MASS_FLOW_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyDynamicPressure, HOBBY_DYNAMIC_PRESSURE_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyDensity, HOBBY_DENSITY_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyArea, HOBBY_AREA_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyRecoveryCda, HOBBY_RECOVERY_CDA_FRACTIONAL_BITS);
hobby_fixed_i32!(HobbyMach, HOBBY_MACH_FRACTIONAL_BITS);

pub const HOBBY_POWERED_STEP: HobbyTime = HobbyTime::from_raw(HOBBY_POWERED_STEP_RAW);
pub const HOBBY_COAST_STEP: HobbyTime = HobbyTime::from_raw(HOBBY_COAST_STEP_RAW);
pub const HOBBY_RECOVERY_STEP: HobbyTime = HobbyTime::from_raw(HOBBY_RECOVERY_STEP_RAW);

pub const fn hobby_numeric_contract_is_valid() -> bool {
    HOBBY_TIME_FRACTIONAL_BITS == 18
        && HOBBY_ALTITUDE_FRACTIONAL_BITS == 13
        && HOBBY_VELOCITY_FRACTIONAL_BITS == 19
        && HOBBY_ACCELERATION_FRACTIONAL_BITS == 19
        && HOBBY_MASS_FRACTIONAL_BITS == 21
        && HOBBY_FORCE_FRACTIONAL_BITS == 13
        && HOBBY_MASS_FLOW_FRACTIONAL_BITS == 23
        && HOBBY_DYNAMIC_PRESSURE_FRACTIONAL_BITS == 7
        && HOBBY_DENSITY_FRACTIONAL_BITS == 29
        && HOBBY_AREA_FRACTIONAL_BITS == 28
        && HOBBY_RECOVERY_CDA_FRACTIONAL_BITS == 23
        && HOBBY_MACH_FRACTIONAL_BITS == 27
        && HOBBY_MAX_TIME_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_ALTITUDE_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_VELOCITY_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_ACCELERATION_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_MASS_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_FORCE_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_MASS_FLOW_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_DYNAMIC_PRESSURE_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_DENSITY_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_AREA_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_RECOVERY_CDA_RAW <= i32::MAX - i32::MAX / 4
        && HOBBY_MAX_MACH_RAW <= i32::MAX - i32::MAX / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_contract_keeps_declared_headroom() {
        assert!(hobby_numeric_contract_is_valid());
        assert_eq!(HOBBY_MIN_ALTITUDE_RAW, -1_000 * (1 << 13));
        assert_eq!(HOBBY_MAX_ALTITUDE_RAW, 150_000 * (1 << 13));
    }

    #[test]
    fn wrappers_remain_exactly_one_word() {
        assert_eq!(core::mem::size_of::<HobbyTime>(), 4);
        assert_eq!(core::mem::size_of::<HobbyAltitude>(), 4);
        assert_eq!(core::mem::size_of::<HobbyVelocity>(), 4);
        assert_eq!(core::mem::size_of::<HobbyAcceleration>(), 4);
        assert_eq!(core::mem::size_of::<HobbyMass>(), 4);
        assert_eq!(core::mem::size_of::<HobbyForce>(), 4);
    }

    #[test]
    fn cadence_rounding_is_frozen() {
        assert_eq!(HOBBY_POWERED_STEP.raw(), 2_621);
        assert_eq!(HOBBY_COAST_STEP.raw(), 5_243);
        assert_eq!(HOBBY_RECOVERY_STEP.raw(), 13_107);
    }
}
