//! Target-executable acceptance checks for the Phase 2 numeric contract.

use crate::phase2_numeric::{sin_cos_pitch_q15, sqrt_floor_u32};
use crate::phase2_quantities::{DownrangeAngle, PitchAngle};

pub fn run_phase2_contract_self_tests() -> u8 {
    let mut failures = 0u8;
    for &(value, expected) in &[
        (0, 0),
        (1, 1),
        (2, 1),
        (3, 1),
        (4, 2),
        (15, 3),
        (16, 4),
        (17, 4),
        (65_535, 255),
        (65_536, 256),
        (0x7fff_ffff, 46_340),
        (0xffff_ffff, 65_535),
    ] {
        if sqrt_floor_u32(value) != expected {
            failures = failures.saturating_add(1);
        }
    }
    if sin_cos_pitch_q15(PitchAngle::RADIAL) != Some((0, 32767)) {
        failures = failures.saturating_add(1);
    }
    if sin_cos_pitch_q15(PitchAngle::PROGRADE) != Some((32767, 0)) {
        failures = failures.saturating_add(1);
    }
    if sin_cos_pitch_q15(PitchAngle::from_raw(16_385)).is_some() {
        failures = failures.saturating_add(1);
    }
    if DownrangeAngle::from_raw(i32::MAX).wrapping_add_raw(1).raw() != i32::MIN {
        failures = failures.saturating_add(1);
    }
    failures
}
