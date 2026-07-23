//! Phase 2 step-aligned open-loop pitch guidance.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus};
use crate::phase2_quantities::PitchAngle;
use crate::quantities::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitchKnot {
    time: Time,
    pitch: PitchAngle,
}

impl PitchKnot {
    pub const fn new(time: Time, pitch: PitchAngle) -> Self {
        Self { time, pitch }
    }
    pub const fn time(self) -> Time {
        self.time
    }
    pub const fn pitch(self) -> PitchAngle {
        self.pitch
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PitchProgram<'a> {
    knots: &'a [PitchKnot],
}

impl<'a> PitchProgram<'a> {
    pub const fn new(knots: &'a [PitchKnot]) -> Self {
        Self { knots }
    }

    pub fn is_valid(self, timestep: Time) -> bool {
        if self.knots.len() < 2 || self.knots.len() > 16 || timestep.raw() <= 0 {
            return false;
        }
        let mut index = 0;
        while index < self.knots.len() {
            let knot = self.knots[index];
            if !knot.pitch().is_phase2_valid()
                || knot.time().raw() < 0
                || knot.time().raw() % timestep.raw() != 0
            {
                return false;
            }
            if index != 0
                && (knot.time() <= self.knots[index - 1].time()
                    || knot.pitch() < self.knots[index - 1].pitch())
            {
                return false;
            }
            index += 1;
        }
        true
    }

    pub fn pitch_at(self, time: Time, status: &mut NumericStatus) -> PitchAngle {
        if self.knots.is_empty() {
            status.record(NumericFault::InvalidInput);
            return PitchAngle::RADIAL;
        }
        if time <= self.knots[0].time() {
            return self.knots[0].pitch();
        }
        let last = self.knots.len() - 1;
        if time >= self.knots[last].time() {
            return self.knots[last].pitch();
        }
        let mut index = 0;
        while index < last {
            let left = self.knots[index];
            let right = self.knots[index + 1];
            if time < right.time() {
                let numerator = subtract(time.raw(), left.time().raw(), status);
                let denominator = subtract(right.time().raw(), left.time().raw(), status);
                let fraction = divide_scaled(numerator, denominator, 16, status).clamp(0, 65535);
                let range = right.pitch().raw() as i32 - left.pitch().raw() as i32;
                let delta = multiply_scaled(range, fraction, 16, status);
                return PitchAngle::from_raw(add(left.pitch().raw() as i32, delta, status) as u16);
            }
            index += 1;
        }
        status.record(NumericFault::InvalidInput);
        PitchAngle::RADIAL
    }
}
