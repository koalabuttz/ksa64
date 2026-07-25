//! Generated and strongly typed Phase 9.5 advanced-effector numeric contract.

include!("../../phase9_5/generated/numeric_v1.rs");

macro_rules! fixed_i32 {
    ($name:ident, $bits:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(i32);
        impl $name {
            pub const FRACTIONAL_BITS: u8 = $bits;
            pub const ZERO: Self = Self(0);
            pub const fn from_raw(raw: i32) -> Self {
                Self(raw)
            }
            pub const fn raw(self) -> i32 {
                self.0
            }
        }
    };
}

fixed_i32!(EffectorForceQ23, ADVANCED_EFFECTOR_FORCE_FRACTIONAL_BITS);
fixed_i32!(HingeMomentQ24, ADVANCED_HINGE_MOMENT_FRACTIONAL_BITS);
fixed_i32!(SupplyPressureQ8, ADVANCED_SUPPLY_PRESSURE_FRACTIONAL_BITS);
fixed_i32!(PulseImpulseQ26, ADVANCED_PULSE_IMPULSE_FRACTIONAL_BITS);
fixed_i32!(MassFlowQ28, ADVANCED_MASS_FLOW_FRACTIONAL_BITS);
fixed_i32!(SupplyScaleQ30, ADVANCED_SUPPLY_SCALE_FRACTIONAL_BITS);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BodyTorqueDemandQ12([i32; 3]);
impl BodyTorqueDemandQ12 {
    pub const ZERO: Self = Self([0; 3]);
    pub const FRACTIONAL_BITS: u8 = BODY_TORQUE_DEMAND_FRACTIONAL_BITS;
    pub const fn from_raw(raw: [i32; 3]) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> [i32; 3] {
        self.0
    }
}

pub const fn advanced_numeric_contract_is_valid() -> bool {
    BODY_TORQUE_DEMAND_FRACTIONAL_BITS == 12
        && ADVANCED_TIME_FRACTIONAL_BITS == 18
        && RCS_PULSE_QUANTUM_Q18 == 1_024
        && ADVANCED_EFFECTOR_FORCE_FRACTIONAL_BITS == 23
        && ADVANCED_HINGE_MOMENT_FRACTIONAL_BITS == 24
        && ADVANCED_SUPPLY_PRESSURE_FRACTIONAL_BITS == 8
        && ADVANCED_PULSE_IMPULSE_FRACTIONAL_BITS == 26
        && ADVANCED_MASS_FLOW_FRACTIONAL_BITS == 28
        && ADVANCED_SUPPLY_SCALE_FRACTIONAL_BITS == 30
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_ranges_and_compatibility_scales_are_frozen() {
        assert!(advanced_numeric_contract_is_valid());
        assert_eq!(RCS_PULSE_QUANTUM_Q18 * 8, 8_192);
        assert_eq!(ADVANCED_MAX_SUPPLY_PRESSURE_RAW, 1_280_000_000);
    }
    #[test]
    fn wrappers_are_one_word_and_demand_is_three_words() {
        assert_eq!(core::mem::size_of::<EffectorForceQ23>(), 4);
        assert_eq!(core::mem::size_of::<SupplyScaleQ30>(), 4);
        assert_eq!(core::mem::size_of::<BodyTorqueDemandQ12>(), 12);
        assert_eq!(BodyTorqueDemandQ12::from_raw([1, -2, 3]).raw(), [1, -2, 3]);
    }
}
