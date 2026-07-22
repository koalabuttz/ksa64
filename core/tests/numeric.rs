use core::mem::size_of;

use ksa64_core::numeric::{
    add, divide_scaled, divide_scaled_reduced_u16, interpolate_clamped,
    interpolate_clamped_integral_q12, multiply_scaled, subtract, NumericFault, NumericStatus,
};
use ksa64_core::quantities::{
    Acceleration, Altitude, Cda, Density, DensitySpeedSquared, Force, Fraction, Mass, MassFlow,
    NetForce, SpeedSquared, Time, Velocity,
};

#[test]
fn generated_numeric_pack_passes() {
    assert_eq!(ksa64_core::run_numeric_self_tests(), 0);
}

#[test]
fn quantity_wrappers_are_one_word_and_keep_their_scales() {
    assert_eq!(size_of::<Time>(), 4);
    assert_eq!(size_of::<Altitude>(), 4);
    assert_eq!(size_of::<Velocity>(), 4);
    assert_eq!(size_of::<Acceleration>(), 4);
    assert_eq!(size_of::<Mass>(), 4);
    assert_eq!(size_of::<MassFlow>(), 4);
    assert_eq!(size_of::<Force>(), 4);
    assert_eq!(size_of::<NetForce>(), 4);
    assert_eq!(size_of::<Density>(), 4);
    assert_eq!(size_of::<Cda>(), 4);
    assert_eq!(size_of::<SpeedSquared>(), 4);
    assert_eq!(size_of::<DensitySpeedSquared>(), 4);
    assert_eq!(size_of::<Fraction>(), 2);
    assert_eq!(Time::FRACTIONAL_BITS, 16);
    assert_eq!(Altitude::FRACTIONAL_BITS, 12);
    assert_eq!(Velocity::FRACTIONAL_BITS, 24);
    assert_eq!(Acceleration::FRACTIONAL_BITS, 28);
    assert_eq!(SpeedSquared::FRACTIONAL_BITS, 20);
}

#[test]
fn faults_are_sticky_and_exact_minimum_is_not_saturation() {
    let mut status = NumericStatus::CLEAR;
    assert_eq!(multiply_scaled(i32::MIN, 1, 0, &mut status), i32::MIN);
    assert!(status.is_clear());

    assert_eq!(
        multiply_scaled(i32::MAX, i32::MAX, 0, &mut status),
        i32::MAX
    );
    assert!(status.contains(NumericFault::Saturation));
    assert_eq!(divide_scaled(1, 0, 0, &mut status), 0);
    assert!(status.contains(NumericFault::DivisionByZero));
    assert_eq!(multiply_scaled(1, 1, 32, &mut status), 0);
    assert!(status.contains(NumericFault::InvalidShift));
}

fn reference_signed(value: i64, negative: bool) -> (i32, bool) {
    let signed = if negative { -value } else { value };
    if signed > i32::MAX as i64 {
        (i32::MAX, true)
    } else if signed < i32::MIN as i64 {
        (i32::MIN, true)
    } else {
        (signed as i32, false)
    }
}

fn reference_rounded_ratio(numerator: i64, denominator: i64) -> i64 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder * 2 >= denominator {
        quotient + 1
    } else {
        quotient
    }
}

#[test]
fn two_word_primitives_match_host_widening_across_boundary_grid() {
    let values = [
        i32::MIN,
        -123_456_789,
        -65_536,
        -1,
        0,
        1,
        65_536,
        123_456_789,
        i32::MAX,
    ];
    let shifts = [0u8, 1, 4, 12, 16, 20, 24, 28, 31];

    for &a in &values {
        for &b in &values {
            for &shift in &shifts {
                let negative = (a < 0) ^ (b < 0);
                let magnitude = (a as i64).abs() * (b as i64).abs();
                let rounded = reference_rounded_ratio(magnitude, 1i64 << shift);
                let expected = reference_signed(rounded, negative);
                let mut status = NumericStatus::CLEAR;
                let actual = multiply_scaled(a, b, shift, &mut status);
                assert_eq!(
                    (actual, status.contains(NumericFault::Saturation)),
                    expected
                );

                if b != 0 {
                    let negative = (a < 0) ^ (b < 0);
                    let shifted = (a as i64).abs() << shift;
                    let rounded = reference_rounded_ratio(shifted, (b as i64).abs());
                    let expected = reference_signed(rounded, negative);
                    let mut status = NumericStatus::CLEAR;
                    let actual = divide_scaled(a, b, shift, &mut status);
                    assert_eq!(
                        (actual, status.contains(NumericFault::Saturation)),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn reduced_u16_division_matches_general_and_falls_back_exactly() {
    let numerators = [
        i32::MIN,
        -2_097_151,
        -2_048_000,
        -1_594_586,
        -4_820,
        -1,
        0,
        1,
        11_046,
        31_130,
        2_047_999,
        2_048_000,
        2_097_151,
        2_097_152,
        i32::MAX,
    ];
    let denominators = [
        128, 256, 384, 491_520, 491_521, 1_945_600, 2_048_000, 8_388_480, 8_388_608, 20_480_000,
        -491_520, 0,
    ];

    for numerator in numerators {
        for denominator in denominators {
            let mut general_status = NumericStatus::CLEAR;
            let general = divide_scaled(numerator, denominator, 28, &mut general_status);
            let mut specialized_status = NumericStatus::CLEAR;
            let specialized = divide_scaled_reduced_u16::<28, 7, 21>(
                numerator,
                denominator,
                &mut specialized_status,
            );
            assert_eq!(specialized, general, "{numerator} / {denominator}");
            assert_eq!(
                specialized_status, general_status,
                "{numerator} / {denominator}"
            );
        }
    }
}

#[test]
fn integral_q12_interpolation_matches_general_contract() {
    let xs = [0, 8_192, 20_480, 40_960, 81_920];
    let ys = [
        328_833_434,
        270_177_602,
        197_599_634,
        111_000_745,
        23_866_596,
    ];
    let probes = [
        -1, 0, 1, 4_095, 4_096, 8_191, 8_192, 14_336, 20_479, 20_480, 30_720, 40_959, 40_960,
        61_440, 81_919, 81_920, 81_921,
    ];

    for probe in probes {
        let mut general_status = NumericStatus::CLEAR;
        let general = interpolate_clamped(probe, &xs, &ys, &mut general_status);
        let mut specialized_status = NumericStatus::CLEAR;
        let specialized =
            interpolate_clamped_integral_q12(probe, &xs, &ys, &mut specialized_status);
        assert_eq!(specialized, general, "probe {probe}");
        assert_eq!(specialized_status, general_status, "probe {probe}");
    }
}

#[test]
fn integral_q12_interpolation_rejects_non_integral_spans() {
    let mut status = NumericStatus::CLEAR;
    assert_eq!(
        interpolate_clamped_integral_q12(4_096, &[0, 8_193], &[0, 100], &mut status),
        0
    );
    assert!(status.contains(NumericFault::InvalidInput));
}

#[test]
fn checked_add_subtract_and_table_validation_contain_bad_inputs() {
    let mut status = NumericStatus::CLEAR;
    assert_eq!(add(i32::MAX, 1, &mut status), i32::MAX);
    assert_eq!(subtract(i32::MIN, 1, &mut status), i32::MIN);
    assert!(status.contains(NumericFault::Saturation));

    let mut table_status = NumericStatus::CLEAR;
    assert_eq!(
        interpolate_clamped(1, &[0, 0, 2], &[10, 20, 30], &mut table_status),
        0
    );
    assert!(table_status.contains(NumericFault::InvalidInput));
}
