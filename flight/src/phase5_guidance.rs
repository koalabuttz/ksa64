//! KSA-5A reference launch-plane and ascent attitude guidance.

use crate::phase5_gnc::SpatialGuidanceTarget;

mod generated {
    include!("../../phase5/generated/guidance_v1.rs");
}

pub const GUIDANCE_POINT_COUNT: usize = generated::GUIDANCE_STEPS.len();
pub const GUIDANCE_SIGNATURE: u32 = generated::GUIDANCE_SIGNATURE;

pub fn reference_guidance_target(step: u32) -> SpatialGuidanceTarget {
    let last = generated::GUIDANCE_STEPS.len() - 1;
    if step >= generated::GUIDANCE_STEPS[last] {
        return SpatialGuidanceTarget {
            attitude_q30: generated::GUIDANCE_ATTITUDE_Q30[last],
            angular_rate_q24: generated::GUIDANCE_RATE_Q24[last],
        };
    }
    let mut segment = 0;
    while segment + 1 < generated::GUIDANCE_STEPS.len()
        && step >= generated::GUIDANCE_STEPS[segment + 1]
    {
        segment += 1;
    }
    let start = generated::GUIDANCE_STEPS[segment];
    let end = generated::GUIDANCE_STEPS[segment + 1];
    let span = end - start;
    let offset = step - start;
    let mut attitude = [0; 4];
    let mut component = 0;
    while component < 4 {
        let a = generated::GUIDANCE_ATTITUDE_Q30[segment][component] as i64;
        let b = generated::GUIDANCE_ATTITUDE_Q30[segment + 1][component] as i64;
        attitude[component] = (a + ((b - a) * offset as i64) / span as i64) as i32;
        component += 1;
    }
    SpatialGuidanceTarget {
        attitude_q30: normalize(attitude).unwrap_or(generated::GUIDANCE_ATTITUDE_Q30[segment]),
        angular_rate_q24: generated::GUIDANCE_RATE_Q24[segment],
    }
}

pub fn reference_guidance_target_scaled(
    step: u32,
    numerator: u32,
    denominator: u32,
) -> SpatialGuidanceTarget {
    if denominator == 0 {
        return reference_guidance_target(step);
    }
    let scaled_step =
        ((step as u64 * numerator as u64) / denominator as u64).min(u32::MAX as u64) as u32;
    let mut target = reference_guidance_target(scaled_step);
    let mut axis = 0;
    while axis < 3 {
        target.angular_rate_q24[axis] =
            ((target.angular_rate_q24[axis] as i64 * numerator as i64) / denominator as i64) as i32;
        axis += 1;
    }
    target
}
pub const fn reference_guidance_point(index: usize) -> Option<(u32, SpatialGuidanceTarget)> {
    if index >= generated::GUIDANCE_STEPS.len() {
        None
    } else {
        Some((
            generated::GUIDANCE_STEPS[index],
            SpatialGuidanceTarget {
                attitude_q30: generated::GUIDANCE_ATTITUDE_Q30[index],
                angular_rate_q24: generated::GUIDANCE_RATE_Q24[index],
            },
        ))
    }
}

fn normalize(q: [i32; 4]) -> Option<[i32; 4]> {
    let mut norm_squared_q30 = 0u64;
    let mut component = 0;
    while component < 4 {
        norm_squared_q30 = norm_squared_q30
            .checked_add(((q[component] as i64 * q[component] as i64) >> 30) as u64)?;
        component += 1;
    }
    let norm_q30 = isqrt(norm_squared_q30.checked_shl(30)?) as i64;
    if norm_q30 == 0 {
        return None;
    }
    let mut result = [0; 4];
    component = 0;
    while component < 4 {
        result[component] = (((q[component] as i64) << 30) / norm_q30) as i32;
        component += 1;
    }
    Some(result)
}

#[allow(clippy::manual_div_ceil)]
fn isqrt(value: u64) -> u64 {
    if value < 2 {
        return value;
    }
    let mut x = 1u64 << ((64 - value.leading_zeros() as u64 + 1) / 2);
    loop {
        let next = (x + value / x) >> 1;
        if next >= x {
            return x;
        }
        x = next;
    }
}
