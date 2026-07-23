use ksa64_interface::phase5::{
    SENSOR_VALID_BAROMETER, SENSOR_VALID_GPS, SENSOR_VALID_STAR_TRACKER,
};
use ksa64_interface::EngineAction;
use ksa64_sim::phase5_sensors::{Phase5SensorFaults, Phase5SensorParameters, Phase5SensorSuite};
use ksa64_sim::phase5_vehicle::{Phase5VehicleCommand, Phase5VehicleMachine};
use ksa64_sim::sensors::StepWindow;

fn command(step: u32) -> Phase5VehicleCommand {
    Phase5VehicleCommand {
        engine_action: if step == 0 {
            EngineAction::Ignite
        } else {
            EngineAction::Hold
        },
        ..Phase5VehicleCommand::HOLD
    }
}

#[test]
fn spatial_sensors_are_deterministic_and_follow_rates_and_delays() {
    let mut vehicle = Phase5VehicleMachine::new_ksa5a().unwrap();
    let mut a = Phase5SensorSuite::new(0x5e05_0001, Phase5SensorFaults::default());
    let mut b = Phase5SensorSuite::new(0x5e05_0001, Phase5SensorFaults::default());
    let mut saw_baro = false;
    let mut saw_gps = false;
    let mut saw_star = false;
    for step in 0..16 {
        let snapshot = vehicle.step(command(step)).unwrap();
        let fa = a.sample(snapshot);
        let fb = b.sample(snapshot);
        assert_eq!(fa, fb);
        saw_baro |= fa.validity & SENSOR_VALID_BAROMETER != 0;
        saw_gps |= fa.validity & SENSOR_VALID_GPS != 0;
        saw_star |= fa.validity & SENSOR_VALID_STAR_TRACKER != 0;
    }
    assert!(saw_baro && saw_gps && saw_star);
    assert_eq!(a.checksum(), b.checksum());
    assert_eq!(a.prng_state(), b.prng_state());
}

#[test]
fn spatial_sensor_outages_remove_only_the_selected_aiding_source() {
    let faults = Phase5SensorFaults {
        gps_outage: Some(StepWindow { start: 0, end: 100 }),
        star_tracker_outage: Some(StepWindow { start: 0, end: 100 }),
        barometer_dropout: None,
    };
    let mut vehicle = Phase5VehicleMachine::new_ksa5a().unwrap();
    let mut sensors =
        Phase5SensorSuite::new_parameterized(7, faults, Phase5SensorParameters::DEFAULT);
    let mut saw_baro = false;
    for step in 0..16 {
        let frame = sensors.sample(vehicle.step(command(step)).unwrap());
        assert_eq!(frame.validity & SENSOR_VALID_GPS, 0);
        assert_eq!(frame.validity & SENSOR_VALID_STAR_TRACKER, 0);
        saw_baro |= frame.validity & SENSOR_VALID_BAROMETER != 0;
    }
    assert!(saw_baro);
}

fn quantize(value: i32, resolution: i32) -> i32 {
    let half = resolution / 2;
    if value >= 0 {
        ((value + half) / resolution) * resolution
    } else {
        ((value - half) / resolution) * resolution
    }
}

#[test]
fn transported_imu_is_the_exact_aggregate_of_four_fast_samples() {
    let mut vehicle = Phase5VehicleMachine::new_ksa5a().unwrap();
    let snapshot = vehicle.step(command(0)).unwrap();
    assert!(snapshot
        .imu_accel_body_q28
        .windows(2)
        .any(|pair| pair[0] != pair[1]));
    let parameters = Phase5SensorParameters {
        noise_scale_ppm: -1_000_000,
        clock_drift_ppm: 0,
        ..Phase5SensorParameters::DEFAULT
    };
    let mut sensors =
        Phase5SensorSuite::new_parameterized(99, Phase5SensorFaults::default(), parameters);
    let frame = sensors.sample(snapshot);
    let mut axis = 0;
    while axis < 3 {
        let mut accel_sum = 0i64;
        let mut gyro_sum = 0i64;
        for fast in 0..4 {
            accel_sum += quantize(
                snapshot.imu_accel_body_q28[fast][axis],
                ksa64_sim::phase5_sensors::ACCEL_RESOLUTION_Q28,
            ) as i64;
            gyro_sum += quantize(
                snapshot.imu_gyro_body_q24[fast][axis],
                ksa64_sim::phase5_sensors::GYRO_RESOLUTION_Q24,
            ) as i64;
        }
        assert_eq!(frame.accel_body_q28[axis], (accel_sum / 4) as i32);
        assert_eq!(frame.gyro_body_q24[axis], (gyro_sum / 4) as i32);
        axis += 1;
    }
}
