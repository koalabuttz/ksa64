//! Phase 2 target-safe numeric helpers and generated constants.

use crate::phase2_quantities::PitchAngle;

include!("../../phase2/generated/contract_v1.rs");

pub fn sqrt_floor_u32(value: u32) -> u32 {
    let mut remainder = value;
    let mut root = 0u32;
    let mut bit = 1u32 << 30;
    while bit > remainder {
        bit >>= 2;
    }
    while bit != 0 {
        if remainder >= root.wrapping_add(bit) {
            remainder -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    root
}

pub fn sin_cos_pitch_q15(angle: PitchAngle) -> Option<(i16, i16)> {
    if !angle.is_phase2_valid() {
        return None;
    }
    let raw = angle.raw();
    Some((
        quarter_wave_q15(raw),
        quarter_wave_q15(PitchAngle::PROGRADE.raw() - raw),
    ))
}

fn quarter_wave_q15(raw: u16) -> i16 {
    let index = (raw >> 6) as usize;
    if index >= 256 {
        return SIN_QUARTER_Q15[256];
    }
    let fraction = (raw & 0x3f) as i32;
    let left = SIN_QUARTER_Q15[index] as i32;
    let right = SIN_QUARTER_Q15[index + 1] as i32;
    (left + (((right - left) * fraction + 32) >> 6)) as i16
}
