use ksa64_core::phase2_numeric::{sin_cos_phase3_pitch_q15, sin_cos_pitch_q15};
use ksa64_core::phase2_quantities::PitchAngle;

#[test]
fn phase3_trig_extends_without_changing_phase2_domain() {
    assert_eq!(
        sin_cos_phase3_pitch_q15(PitchAngle::RADIAL),
        Some((0, 32_767))
    );
    assert_eq!(
        sin_cos_phase3_pitch_q15(PitchAngle::PROGRADE),
        Some((32_767, 0))
    );
    let angle_110 = PitchAngle::from_raw(20_025);
    let (sine, cosine) = sin_cos_phase3_pitch_q15(angle_110).unwrap();
    assert!((30_790..=30_800).contains(&sine));
    assert!((-11_215..=-11_195).contains(&cosine));
    assert_eq!(sin_cos_pitch_q15(angle_110), None);
    assert_eq!(sin_cos_phase3_pitch_q15(PitchAngle::from_raw(20_026)), None);
}
