use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_quantities::{
    DownrangeAngle, DynamicPressure, Mach, PlanarVelocity, Radius, SpecificAngularMomentum,
};
use ksa64_core::planar::{PlanarTruthState, StagePhase};
use ksa64_core::quantities::{Mass, Time};
use ksa64_interface::{SENSOR_VALID_ALTIMETER, SENSOR_VALID_GPS};
use ksa64_sim::actuator::SteeringSnapshot;
use ksa64_sim::sensors::*;
use ksa64_sim::world::WorldSnapshot;

fn snapshot(step: u32, altitude_m: i32) -> WorldSnapshot {
    let truth = PlanarTruthState::new(
        step,
        Time::from_raw((step as i32) * 8_192),
        Radius::from_raw(EARTH_RADIUS_Q12 + altitude_m * 4096 / 1000),
        DownrangeAngle::from_raw((step as i32) * 10),
        PlanarVelocity::from_raw(0),
        SpecificAngularMomentum::from_raw(0),
        Mass::from_raw(1_000 * 4096),
        Mass::from_raw(500 * 4096),
        0,
        StagePhase::Burning,
    );
    WorldSnapshot {
        truth,
        pitch: ksa64_core::phase2_quantities::PitchAngle::RADIAL,
        mach: Mach::ZERO,
        dynamic_pressure: DynamicPressure::ZERO,
        events: 0,
        truth_checksum: 0,
    }
}
fn steering(pitch: u16) -> SteeringSnapshot {
    SteeringSnapshot {
        requested: pitch,
        lagged_target: pitch,
        applied: pitch,
        stuck: false,
    }
}

#[test]
fn xorshift_is_nonzero_and_repeatable() {
    let mut a = XorShift32::new(0);
    let mut b = XorShift32::new(0);
    assert_ne!(a.state(), 0);
    for _ in 0..64 {
        assert_eq!(a.next_u32(), b.next_u32());
        assert_ne!(a.state(), 0)
    }
}

#[test]
fn identical_seed_and_truth_produce_identical_frames_and_checksums() {
    let mut a = SensorSuite::new(0x1234_5678, SensorFaults::default());
    let mut b = SensorSuite::new(0x1234_5678, SensorFaults::default());
    for step in 0..1100 {
        let sa = a.sample(snapshot(step, 10_000), steering(1000));
        let sb = b.sample(snapshot(step, 10_000), steering(1000));
        assert_eq!(sa, sb)
    }
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(a.prng_state(), b.prng_state())
}

#[test]
fn rates_latency_quantization_bias_noise_and_clock_are_bounded() {
    let mut sensors = SensorSuite::new(7, SensorFaults::default());
    let mut alt_steps = Vec::new();
    let mut gps_steps = Vec::new();
    for step in 0..970 {
        let frame = sensors.sample(snapshot(step, 10_000), steering(0));
        assert_eq!(frame.accel_radial_q28 % ACCEL_RESOLUTION_Q28, 0);
        assert_eq!(frame.accel_tangential_q28 % ACCEL_RESOLUTION_Q28, 0);
        assert!(frame.accel_radial_q28.abs() <= 3_000_000);
        assert!(frame.accel_tangential_q28.abs() <= ACCEL_RESOLUTION_Q28 * 2);
        assert_eq!(frame.gyro_rate_q24 % GYRO_RESOLUTION_Q24, 0);
        let expected_clock =
            (step as i32) * 8_192 + (((step as i64) * 8_192 * 20) / 1_000_000) as i32;
        assert_eq!(frame.onboard_time_q16, expected_clock);
        if frame.validity & SENSOR_VALID_ALTIMETER != 0 {
            assert_eq!(frame.altitude_q12 % ALT_RESOLUTION_Q12, 0);
            alt_steps.push(step)
        }
        if frame.validity & SENSOR_VALID_GPS != 0 {
            assert_eq!(frame.gps_radius_q12 % GPS_POSITION_RESOLUTION_Q12, 0);
            assert_eq!(frame.gps_downrange_q32 % GPS_ANGLE_RESOLUTION_Q32, 0);
            assert_eq!(
                frame.gps_radial_velocity_q24 % GPS_VELOCITY_RESOLUTION_Q24,
                0
            );
            gps_steps.push(step)
        }
    }
    assert_eq!(&alt_steps[..4], &[1, 3, 5, 7]);
    assert_eq!(gps_steps[0], GPS_ACQUIRE_STEP + 2)
}

#[test]
fn dropout_windows_remove_measurements_and_recover() {
    let faults = SensorFaults {
        altimeter_dropout: Some(StepWindow {
            start: 360,
            end: 480,
        }),
        gps_outage: Some(StepWindow {
            start: 1000,
            end: 1040,
        }),
    };
    let mut sensors = SensorSuite::new(9, faults);
    let mut alt_before = false;
    let mut alt_after = false;
    let mut gps_before = false;
    let mut gps_after = false;
    for step in 0..1060 {
        let frame = sensors.sample(snapshot(step, 20_000), steering(0));
        if step < 360 && frame.validity & SENSOR_VALID_ALTIMETER != 0 {
            alt_before = true
        }
        if (360..480).contains(&step) {
            assert_eq!(frame.validity & SENSOR_VALID_ALTIMETER, 0)
        }
        if step >= 480 && frame.validity & SENSOR_VALID_ALTIMETER != 0 {
            alt_after = true
        }
        if (962..1000).contains(&step) && frame.validity & SENSOR_VALID_GPS != 0 {
            gps_before = true
        }
        if (1000..1040).contains(&step) {
            assert_eq!(frame.validity & SENSOR_VALID_GPS, 0)
        }
        if step >= 1040 && frame.validity & SENSOR_VALID_GPS != 0 {
            gps_after = true
        }
    }
    assert!(alt_before && alt_after && gps_before && gps_after)
}
