//! Deterministic multirate Phase 5 IMU, barometer, GPS, star tracker, clock,
//! and actuator-feedback sensors.

use crate::phase5_vehicle::{Phase5StagePhase, Phase5VehicleSnapshot};
use crate::sensors::{StepWindow, XorShift32};
use ksa64_core::numeric::NumericStatus;
use ksa64_core::spatial_numeric::QuaternionQ30;
use ksa64_core::spatial_world::evaluate_spatial_environment;
use ksa64_interface::phase5::{
    write_spatial_sensor_frame, SpatialSensorFrame, EVENT_GPS_ACQUIRED, EVENT_GPS_LOST,
    EVENT_STAR_ACQUIRED, EVENT_STAR_LOST, SENSOR_VALID_ACTUATOR, SENSOR_VALID_BAROMETER,
    SENSOR_VALID_CLOCK, SENSOR_VALID_GPS, SENSOR_VALID_IMU, SENSOR_VALID_STAR_TRACKER,
    SPATIAL_SENSOR_FRAME_LENGTH,
};
use ksa64_interface::StagePhase;

pub const SPATIAL_SENSOR_CONTRACT_ID: u32 = 0x0507_0001;
pub const ACCEL_RESOLUTION_Q28: i32 = 27;
pub const ACCEL_NOISE_Q28: i32 = 54;
pub const GYRO_RESOLUTION_Q24: i32 = 168;
pub const GYRO_NOISE_Q24: i32 = 336;
pub const BARO_RESOLUTION_Q12: i32 = 1;
pub const BARO_NOISE_Q12: i32 = 4;
pub const GPS_POSITION_RESOLUTION_Q12: i32 = 4;
pub const GPS_POSITION_NOISE_Q12: i32 = 20;
pub const GPS_VELOCITY_RESOLUTION_Q24: i32 = 168;
pub const GPS_VELOCITY_NOISE_Q24: i32 = 839;
pub const STAR_COMPONENT_RESOLUTION_Q30: i32 = 1_074;
pub const STAR_COMPONENT_NOISE_Q30: i32 = 2_148;
pub const BAROMETER_MAX_ALTITUDE_Q12: i32 = 80 * 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Phase5SensorFaults {
    pub barometer_dropout: Option<StepWindow>,
    pub gps_outage: Option<StepWindow>,
    pub star_tracker_outage: Option<StepWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5SensorParameters {
    pub accelerometer_bias_q28: [i32; 3],
    pub gyro_bias_q24: [i32; 3],
    pub barometer_bias_q12: i32,
    pub gps_position_bias_q12: [i32; 3],
    pub gps_velocity_bias_q24: [i32; 3],
    pub star_component_bias_q30: [i32; 4],
    pub noise_scale_ppm: i32,
    pub clock_drift_ppm: i32,
}

impl Phase5SensorParameters {
    pub const DEFAULT: Self = Self {
        accelerometer_bias_q28: [0; 3],
        gyro_bias_q24: [0; 3],
        barometer_bias_q12: 0,
        gps_position_bias_q12: [0; 3],
        gps_velocity_bias_q24: [0; 3],
        star_component_bias_q30: [0; 4],
        noise_scale_ppm: 0,
        clock_drift_ppm: 20,
    };
    pub const fn is_valid(self) -> bool {
        self.noise_scale_ppm >= -1_000_000
            && self.noise_scale_ppm <= 1_000_000
            && self.clock_drift_ppm >= -10_000
            && self.clock_drift_ppm <= 10_000
    }
}
impl Default for Phase5SensorParameters {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GpsSample {
    position: [i32; 3],
    velocity: [i32; 3],
}

fn triangular(prng: &mut XorShift32, amplitude: i32) -> i32 {
    if amplitude <= 0 {
        return 0;
    }
    let span = amplitude as u32 + 1;
    (prng.next_u32() % span) as i32 - (prng.next_u32() % span) as i32
}
fn scale_ppm(value: i32, delta_ppm: i32) -> i32 {
    ((value as i64 * (1_000_000i64 + delta_ppm as i64)) / 1_000_000i64) as i32
}
fn quantize(value: i32, resolution: i32) -> i32 {
    if resolution <= 1 {
        return value;
    }
    let half = resolution / 2;
    if value >= 0 {
        ((value + half) / resolution) * resolution
    } else {
        ((value - half) / resolution) * resolution
    }
}
fn map_phase(phase: Phase5StagePhase) -> StagePhase {
    match phase {
        Phase5StagePhase::CoastBeforeIgnition => StagePhase::CoastBeforeIgnition,
        Phase5StagePhase::Burning => StagePhase::Burning,
        Phase5StagePhase::CoastBeforeSeparation => StagePhase::CoastBeforeSeparation,
        Phase5StagePhase::Complete => StagePhase::Complete,
    }
}
fn in_window(window: Option<StepWindow>, step: u32) -> bool {
    window.map(|value| value.contains(step)).unwrap_or(false)
}
fn vector_to_array<const F: u8>(vector: ksa64_core::spatial_numeric::FixedVec3<F>) -> [i32; 3] {
    [vector.x(), vector.y(), vector.z()]
}
fn quat_to_array(quaternion: QuaternionQ30) -> [i32; 4] {
    [
        quaternion.w(),
        quaternion.x(),
        quaternion.y(),
        quaternion.z(),
    ]
}

pub struct Phase5SensorSuite {
    prng: XorShift32,
    faults: Phase5SensorFaults,
    parameters: Phase5SensorParameters,
    barometer_delay: Option<i32>,
    gps_delay: [Option<GpsSample>; 2],
    star_delay: Option<[i32; 4]>,
    gps_was_valid: bool,
    star_was_valid: bool,
    checksum: u32,
}

impl Phase5SensorSuite {
    pub const fn new(seed: u32, faults: Phase5SensorFaults) -> Self {
        Self::new_parameterized(seed, faults, Phase5SensorParameters::DEFAULT)
    }
    pub const fn new_parameterized(
        seed: u32,
        faults: Phase5SensorFaults,
        parameters: Phase5SensorParameters,
    ) -> Self {
        Self {
            prng: XorShift32::new(seed),
            faults,
            parameters,
            barometer_delay: None,
            gps_delay: [None, None],
            star_delay: None,
            gps_was_valid: false,
            star_was_valid: false,
            checksum: 2_166_136_261,
        }
    }
    pub const fn checksum(&self) -> u32 {
        self.checksum
    }
    pub const fn prng_state(&self) -> u32 {
        self.prng.state()
    }

    pub fn sample(&mut self, snapshot: Phase5VehicleSnapshot) -> SpatialSensorFrame {
        let truth = snapshot.truth;
        let step = truth.step();
        let mut status = NumericStatus::CLEAR;
        let environment = evaluate_spatial_environment(truth.spatial(), &mut status);
        let accel_noise = scale_ppm(ACCEL_NOISE_Q28, self.parameters.noise_scale_ppm);
        let gyro_noise = scale_ppm(GYRO_NOISE_Q24, self.parameters.noise_scale_ppm);
        let baro_noise = scale_ppm(BARO_NOISE_Q12, self.parameters.noise_scale_ppm);
        let gps_position_noise = scale_ppm(GPS_POSITION_NOISE_Q12, self.parameters.noise_scale_ppm);
        let gps_velocity_noise = scale_ppm(GPS_VELOCITY_NOISE_Q24, self.parameters.noise_scale_ppm);
        let star_noise = scale_ppm(STAR_COMPONENT_NOISE_Q30, self.parameters.noise_scale_ppm);
        let mut accel_sum = [0i64; 3];
        let mut gyro_sum = [0i64; 3];
        let mut fast = 0;
        while fast < snapshot.imu_accel_body_q28.len() {
            let mut axis = 0;
            while axis < 3 {
                let accel_sample = quantize(
                    snapshot.imu_accel_body_q28[fast][axis]
                        .saturating_add(self.parameters.accelerometer_bias_q28[axis])
                        .saturating_add(triangular(&mut self.prng, accel_noise)),
                    ACCEL_RESOLUTION_Q28,
                );
                let gyro_sample = quantize(
                    snapshot.imu_gyro_body_q24[fast][axis]
                        .saturating_add(self.parameters.gyro_bias_q24[axis])
                        .saturating_add(triangular(&mut self.prng, gyro_noise)),
                    GYRO_RESOLUTION_Q24,
                );
                accel_sum[axis] += accel_sample as i64;
                gyro_sum[axis] += gyro_sample as i64;
                axis += 1;
            }
            fast += 1;
        }
        let sample_count = snapshot.imu_accel_body_q28.len() as i64;
        let mut accel_body = [0; 3];
        let mut gyro_body = [0; 3];
        let mut axis = 0;
        while axis < 3 {
            accel_body[axis] = (accel_sum[axis] / sample_count) as i32;
            gyro_body[axis] = (gyro_sum[axis] / sample_count) as i32;
            axis += 1;
        }

        let barometer_new = if step & 1 == 0
            && environment.altitude_q12() >= 0
            && environment.altitude_q12() <= BAROMETER_MAX_ALTITUDE_Q12
        {
            Some(quantize(
                environment
                    .altitude_q12()
                    .saturating_add(self.parameters.barometer_bias_q12)
                    .saturating_add(triangular(&mut self.prng, baro_noise)),
                BARO_RESOLUTION_Q12,
            ))
        } else {
            None
        };
        let barometer = self.barometer_delay;
        self.barometer_delay = barometer_new;

        let gps_new = if step & 7 == 0 {
            let position = vector_to_array(truth.spatial().position());
            let velocity = vector_to_array(truth.spatial().velocity());
            let mut sample = GpsSample::default();
            axis = 0;
            while axis < 3 {
                sample.position[axis] = quantize(
                    position[axis]
                        .saturating_add(self.parameters.gps_position_bias_q12[axis])
                        .saturating_add(triangular(&mut self.prng, gps_position_noise)),
                    GPS_POSITION_RESOLUTION_Q12,
                );
                sample.velocity[axis] = quantize(
                    velocity[axis]
                        .saturating_add(self.parameters.gps_velocity_bias_q24[axis])
                        .saturating_add(triangular(&mut self.prng, gps_velocity_noise)),
                    GPS_VELOCITY_RESOLUTION_Q24,
                );
                axis += 1;
            }
            Some(sample)
        } else {
            None
        };
        let gps = self.gps_delay[0];
        self.gps_delay[0] = self.gps_delay[1];
        self.gps_delay[1] = gps_new;

        let star_new = if step & 3 == 0 {
            let truth_quaternion = quat_to_array(truth.rigid().attitude());
            let mut measured = [0; 4];
            let mut component = 0;
            while component < 4 {
                measured[component] = quantize(
                    truth_quaternion[component]
                        .saturating_add(self.parameters.star_component_bias_q30[component])
                        .saturating_add(triangular(&mut self.prng, star_noise)),
                    STAR_COMPONENT_RESOLUTION_Q30,
                );
                component += 1;
            }
            let mut normalize_status = NumericStatus::CLEAR;
            let normalized = QuaternionQ30::new(measured[0], measured[1], measured[2], measured[3])
                .normalized(&mut normalize_status);
            Some(if normalize_status.is_clear() {
                quat_to_array(normalized)
            } else {
                truth_quaternion
            })
        } else {
            None
        };
        let star = self.star_delay;
        self.star_delay = star_new;

        let barometer_valid =
            barometer.is_some() && !in_window(self.faults.barometer_dropout, step);
        let gps_valid = gps.is_some() && !in_window(self.faults.gps_outage, step);
        let star_valid = star.is_some() && !in_window(self.faults.star_tracker_outage, step);
        let mut events = snapshot.events;
        if gps_valid && !self.gps_was_valid {
            events |= EVENT_GPS_ACQUIRED;
        } else if !gps_valid && self.gps_was_valid {
            events |= EVENT_GPS_LOST;
        }
        if star_valid && !self.star_was_valid {
            events |= EVENT_STAR_ACQUIRED;
        } else if !star_valid && self.star_was_valid {
            events |= EVENT_STAR_LOST;
        }
        self.gps_was_valid = gps_valid;
        self.star_was_valid = star_valid;
        let gps_value = gps.unwrap_or_default();
        let clock_drift = ((truth.time_q16() as i64 * self.parameters.clock_drift_ppm as i64)
            / 1_000_000i64) as i32;
        let frame = SpatialSensorFrame {
            sequence: step,
            onboard_time_q16: truth.time_q16().saturating_add(clock_drift),
            validity: SENSOR_VALID_IMU
                | SENSOR_VALID_CLOCK
                | SENSOR_VALID_ACTUATOR
                | if barometer_valid {
                    SENSOR_VALID_BAROMETER
                } else {
                    0
                }
                | if gps_valid { SENSOR_VALID_GPS } else { 0 }
                | if star_valid {
                    SENSOR_VALID_STAR_TRACKER
                } else {
                    0
                },
            events,
            accel_body_q28: accel_body,
            gyro_body_q24: gyro_body,
            baro_altitude_q12: barometer.unwrap_or(0),
            gps_position_q12: gps_value.position,
            gps_velocity_q24: gps_value.velocity,
            star_attitude_q30: star.unwrap_or([0; 4]),
            gimbal_applied_q16: [snapshot.gimbal.applied.pitch, snapshot.gimbal.applied.yaw],
            rcs_propellant_q12: snapshot.rcs_propellant_q12,
            active_stage: truth.active_stage(),
            stage_phase: map_phase(truth.phase()),
            engine_on: truth.phase() == Phase5StagePhase::Burning,
        };
        let mut bytes = [0u8; SPATIAL_SENSOR_FRAME_LENGTH];
        if write_spatial_sensor_frame(&frame, &mut bytes).is_ok() {
            self.checksum = rolling_hash(self.checksum, &bytes);
        }
        frame
    }
}

fn rolling_hash(mut hash: u32, bytes: &[u8]) -> u32 {
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    hash
}
