use ksa64_flight::phase5_guidance::{
    reference_guidance_point, reference_guidance_target, GUIDANCE_POINT_COUNT, GUIDANCE_SIGNATURE,
};

#[inline]
fn hash(mut value: u32, word: u32) -> u32 {
    for byte in word.to_le_bytes() {
        value = (value ^ byte as u32).wrapping_mul(16_777_619);
    }
    value
}

pub fn phase5_guidance_signature() -> u32 {
    let mut signature = 2_166_136_261u32;
    let mut index = 0;
    while index < GUIDANCE_POINT_COUNT {
        let Some((step, target)) = reference_guidance_point(index) else {
            return 0;
        };
        let sampled = reference_guidance_target(step);
        if sampled.attitude_q30 == [0; 4] {
            return 0;
        }
        signature = hash(signature, step);
        for value in target.attitude_q30 {
            signature = hash(signature, value as u32);
        }
        for value in target.angular_rate_q24 {
            signature = hash(signature, value as u32);
        }
        index += 1;
    }
    signature
}

pub fn run_phase5_guidance_self_tests() -> u32 {
    if phase5_guidance_signature() == GUIDANCE_SIGNATURE {
        0
    } else {
        1
    }
}
