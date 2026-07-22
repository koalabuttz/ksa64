//! Strong storage types for the accepted Phase 1 numeric contract.

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

fixed_i32!(Time, 16);
fixed_i32!(Altitude, 12);
fixed_i32!(Velocity, 24);
fixed_i32!(Acceleration, 28);
fixed_i32!(Mass, 12);
fixed_i32!(MassFlow, 16);
fixed_i32!(Force, 12);
fixed_i32!(NetForce, 12);
fixed_i32!(Density, 28);
fixed_i32!(Cda, 16);
fixed_i32!(SpeedSquared, 20);
fixed_i32!(DensitySpeedSquared, 20);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fraction(u16);

impl Fraction {
    pub const FRACTIONAL_BITS: u8 = 16;
    pub const ZERO: Self = Self(0);
    pub const ALMOST_ONE: Self = Self(u16::MAX);

    #[inline]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }
}
