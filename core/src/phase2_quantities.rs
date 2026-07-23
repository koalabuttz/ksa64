//! Strong storage types for the Phase 2 planar numeric contract.

macro_rules! fixed_i32 {
    ($name:ident, $fractional_bits:expr) => {
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

fixed_i32!(Radius, 12);
fixed_i32!(PlanarAltitude, 12);
fixed_i32!(PlanarVelocity, 24);
fixed_i32!(PlanarAcceleration, 28);
fixed_i32!(SpecificAngularMomentum, 14);
fixed_i32!(Mach, 16);
fixed_i32!(DynamicPressure, 16);
fixed_i32!(Coefficient, 14);
fixed_i32!(ReferenceArea, 16);
fixed_i32!(SpecificEnergy, 24);
fixed_i32!(Eccentricity, 16);
fixed_i32!(GravitationalParameter, 12);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DownrangeAngle(i32);

impl DownrangeAngle {
    pub const ZERO: Self = Self(0);
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> i32 {
        self.0
    }
    pub const fn wrapping_add_raw(self, delta: i32) -> Self {
        Self(self.0.wrapping_add(delta))
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PitchAngle(u16);

impl PitchAngle {
    pub const RADIAL: Self = Self(0);
    pub const PROGRADE: Self = Self(1 << 14);
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u16 {
        self.0
    }
    pub const fn is_phase2_valid(self) -> bool {
        self.0 <= Self::PROGRADE.0
    }
    pub const fn is_phase3_valid(self) -> bool {
        self.0 <= 20_025
    }
}
