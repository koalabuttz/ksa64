//! Allocation-free fixed-point vectors and Hamilton quaternions for Phase 5.

use crate::numeric::{
    add, divide_scaled, multiply_scaled, sqrt_floor_scaled_u32, subtract, NumericFault,
    NumericStatus,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FixedVec3<const FRACTIONAL_BITS: u8> {
    x: i32,
    y: i32,
    z: i32,
}

impl<const FRACTIONAL_BITS: u8> FixedVec3<FRACTIONAL_BITS> {
    pub const ZERO: Self = Self::new(0, 0, 0);

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn y(self) -> i32 {
        self.y
    }

    pub const fn z(self) -> i32 {
        self.z
    }

    pub fn checked_add(self, rhs: Self, status: &mut NumericStatus) -> Self {
        Self::new(
            add(self.x, rhs.x, status),
            add(self.y, rhs.y, status),
            add(self.z, rhs.z, status),
        )
    }

    pub fn checked_sub(self, rhs: Self, status: &mut NumericStatus) -> Self {
        Self::new(
            subtract(self.x, rhs.x, status),
            subtract(self.y, rhs.y, status),
            subtract(self.z, rhs.z, status),
        )
    }

    pub fn scale<const SCALAR_FRACTIONAL_BITS: u8>(
        self,
        scalar: i32,
        status: &mut NumericStatus,
    ) -> Self {
        Self::new(
            multiply_scaled(self.x, scalar, SCALAR_FRACTIONAL_BITS, status),
            multiply_scaled(self.y, scalar, SCALAR_FRACTIONAL_BITS, status),
            multiply_scaled(self.z, scalar, SCALAR_FRACTIONAL_BITS, status),
        )
    }

    pub fn dot_scaled<const OUTPUT_FRACTIONAL_BITS: u8>(
        self,
        rhs: Self,
        status: &mut NumericStatus,
    ) -> i32 {
        let combined = FRACTIONAL_BITS.saturating_mul(2);
        if combined < OUTPUT_FRACTIONAL_BITS || combined - OUTPUT_FRACTIONAL_BITS > 31 {
            status.record(NumericFault::InvalidShift);
            return 0;
        }
        let shift = combined - OUTPUT_FRACTIONAL_BITS;
        let xy = add(
            multiply_scaled(self.x, rhs.x, shift, status),
            multiply_scaled(self.y, rhs.y, shift, status),
            status,
        );
        add(xy, multiply_scaled(self.z, rhs.z, shift, status), status)
    }

    pub fn cross_scaled<const OUTPUT_FRACTIONAL_BITS: u8>(
        self,
        rhs: Self,
        status: &mut NumericStatus,
    ) -> FixedVec3<OUTPUT_FRACTIONAL_BITS> {
        cross_mixed_scaled::<FRACTIONAL_BITS, FRACTIONAL_BITS, OUTPUT_FRACTIONAL_BITS>(
            self, rhs, status,
        )
    }
}

pub fn cross_mixed_scaled<
    const LEFT_FRACTIONAL_BITS: u8,
    const RIGHT_FRACTIONAL_BITS: u8,
    const OUTPUT_FRACTIONAL_BITS: u8,
>(
    left: FixedVec3<LEFT_FRACTIONAL_BITS>,
    right: FixedVec3<RIGHT_FRACTIONAL_BITS>,
    status: &mut NumericStatus,
) -> FixedVec3<OUTPUT_FRACTIONAL_BITS> {
    let combined = LEFT_FRACTIONAL_BITS.saturating_add(RIGHT_FRACTIONAL_BITS);
    if combined < OUTPUT_FRACTIONAL_BITS || combined - OUTPUT_FRACTIONAL_BITS > 31 {
        status.record(NumericFault::InvalidShift);
        return FixedVec3::ZERO;
    }
    let shift = combined - OUTPUT_FRACTIONAL_BITS;
    FixedVec3::new(
        subtract(
            multiply_scaled(left.y, right.z, shift, status),
            multiply_scaled(left.z, right.y, shift, status),
            status,
        ),
        subtract(
            multiply_scaled(left.z, right.x, shift, status),
            multiply_scaled(left.x, right.z, shift, status),
            status,
        ),
        subtract(
            multiply_scaled(left.x, right.y, shift, status),
            multiply_scaled(left.y, right.x, shift, status),
            status,
        ),
    )
}

pub type PositionVec = FixedVec3<12>;
pub type VelocityVec = FixedVec3<24>;
pub type AccelerationVec = FixedVec3<28>;
pub type AngularRateVec = FixedVec3<24>;
pub type TorqueVec = FixedVec3<16>;
pub type ModalVec = FixedVec3<24>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct QuaternionQ30 {
    w: i32,
    x: i32,
    y: i32,
    z: i32,
}

impl QuaternionQ30 {
    pub const ONE: i32 = 1 << 30;
    pub const IDENTITY: Self = Self::new(Self::ONE, 0, 0, 0);

    pub const fn new(w: i32, x: i32, y: i32, z: i32) -> Self {
        Self { w, x, y, z }
    }

    pub const fn w(self) -> i32 {
        self.w
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn y(self) -> i32 {
        self.y
    }

    pub const fn z(self) -> i32 {
        self.z
    }

    pub const fn vector(self) -> FixedVec3<30> {
        FixedVec3::new(self.x, self.y, self.z)
    }

    pub const fn conjugate(self) -> Self {
        Self::new(self.w, -self.x, -self.y, -self.z)
    }

    pub fn norm_squared_q30(self, status: &mut NumericStatus) -> i32 {
        let wx = add(
            multiply_scaled(self.w, self.w, 30, status),
            multiply_scaled(self.x, self.x, 30, status),
            status,
        );
        let yz = add(
            multiply_scaled(self.y, self.y, 30, status),
            multiply_scaled(self.z, self.z, 30, status),
            status,
        );
        add(wx, yz, status)
    }

    pub fn normalized(self, status: &mut NumericStatus) -> Self {
        let norm_squared = self.norm_squared_q30(status);
        if norm_squared <= 0 {
            status.record(NumericFault::InvalidInput);
            return Self::IDENTITY;
        }
        let norm = sqrt_floor_scaled_u32(norm_squared as u32, 30, status);
        if norm == 0 || norm > i32::MAX as u32 {
            status.record(NumericFault::InvalidInput);
            return Self::IDENTITY;
        }
        let denominator = norm as i32;
        Self::new(
            divide_scaled(self.w, denominator, 30, status),
            divide_scaled(self.x, denominator, 30, status),
            divide_scaled(self.y, denominator, 30, status),
            divide_scaled(self.z, denominator, 30, status),
        )
    }

    pub fn hamilton(self, rhs: Self, status: &mut NumericStatus) -> Self {
        let w = subtract(
            subtract(
                subtract(
                    multiply_scaled(self.w, rhs.w, 30, status),
                    multiply_scaled(self.x, rhs.x, 30, status),
                    status,
                ),
                multiply_scaled(self.y, rhs.y, 30, status),
                status,
            ),
            multiply_scaled(self.z, rhs.z, 30, status),
            status,
        );
        let x = add(
            add(
                multiply_scaled(self.w, rhs.x, 30, status),
                multiply_scaled(self.x, rhs.w, 30, status),
                status,
            ),
            subtract(
                multiply_scaled(self.y, rhs.z, 30, status),
                multiply_scaled(self.z, rhs.y, 30, status),
                status,
            ),
            status,
        );
        let y = add(
            add(
                multiply_scaled(self.w, rhs.y, 30, status),
                multiply_scaled(self.y, rhs.w, 30, status),
                status,
            ),
            subtract(
                multiply_scaled(self.z, rhs.x, 30, status),
                multiply_scaled(self.x, rhs.z, 30, status),
                status,
            ),
            status,
        );
        let z = add(
            add(
                multiply_scaled(self.w, rhs.z, 30, status),
                multiply_scaled(self.z, rhs.w, 30, status),
                status,
            ),
            subtract(
                multiply_scaled(self.x, rhs.y, 30, status),
                multiply_scaled(self.y, rhs.x, 30, status),
                status,
            ),
            status,
        );
        Self::new(w, x, y, z)
    }

    /// Rotates a vector from body coordinates into ECI coordinates.
    pub fn rotate<const FRACTIONAL_BITS: u8>(
        self,
        vector: FixedVec3<FRACTIONAL_BITS>,
        status: &mut NumericStatus,
    ) -> FixedVec3<FRACTIONAL_BITS> {
        let cross = cross_mixed_scaled::<30, FRACTIONAL_BITS, FRACTIONAL_BITS>(
            self.vector(),
            vector,
            status,
        );
        let twice_cross = cross.checked_add(cross, status);
        let scalar_term = twice_cross.scale::<30>(self.w, status);
        let vector_term = cross_mixed_scaled::<30, FRACTIONAL_BITS, FRACTIONAL_BITS>(
            self.vector(),
            twice_cross,
            status,
        );
        vector
            .checked_add(scalar_term, status)
            .checked_add(vector_term, status)
    }
}
