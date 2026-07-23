//! Deterministic imperfect Phase 3 sensors with bounded latency queues.

use crate::actuator::SteeringSnapshot;
use crate::world::WorldSnapshot;
use ksa64_core::phase2_mission::{
    EVENT_CUTOFF as WORLD_CUTOFF, EVENT_IGNITION as WORLD_IGNITION,
    EVENT_SEPARATION as WORLD_SEPARATION,
};
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::planar::{evaluate_vacuum, PlanarWorld};
use ksa64_interface::{
    SensorFrame, StagePhase, EVENT_CUTOFF, EVENT_GPS_ACQUIRED, EVENT_GPS_LOST, EVENT_IGNITION,
    EVENT_SEPARATION, SENSOR_VALID_ACCEL, SENSOR_VALID_ALTIMETER, SENSOR_VALID_CLOCK,
    SENSOR_VALID_GPS, SENSOR_VALID_GYRO, SENSOR_VALID_STEERING,
};

pub const ACCEL_RESOLUTION_Q28: i32 = 2_684;
pub const ACCEL_BIAS_Q28: i32 = 537;
pub const ACCEL_NOISE_Q28: i32 = 2_684;
pub const GYRO_RESOLUTION_Q24: i32 = 167_772; // 0.01 degree/s.
pub const GYRO_BIAS_Q24: i32 = 33_554;
pub const GYRO_NOISE_Q24: i32 = 83_886;
pub const ALT_RESOLUTION_Q12: i32 = 41;
pub const ALT_BIAS_Q12: i32 = 82;
pub const ALT_NOISE_Q12: i32 = 41;
pub const GPS_POSITION_RESOLUTION_Q12: i32 = 41;
pub const GPS_POSITION_NOISE_Q12: i32 = 82;
pub const GPS_ANGLE_RESOLUTION_Q32: i32 = 1_073; // about 10 m at the equator.
pub const GPS_ANGLE_NOISE_Q32: i32 = 2_146;
pub const GPS_VELOCITY_RESOLUTION_Q24: i32 = 1_678;
pub const GPS_VELOCITY_NOISE_Q24: i32 = 3_355;
pub const GPS_ACQUIRE_STEP: u32 = 960;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepWindow {
    pub start: u32,
    pub end: u32,
}
impl StepWindow {
    pub const fn contains(self, step: u32) -> bool {
        step >= self.start && step < self.end
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SensorFaults {
    pub altimeter_dropout: Option<StepWindow>,
    pub gps_outage: Option<StepWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensorParameters {
    pub accelerometer_bias_q28: i32,
    pub gyro_bias_q24: i32,
    pub altimeter_bias_q12: i32,
    pub gps_radial_position_bias_q12: i32,
    pub gps_downrange_bias_q32: i32,
    pub gps_radial_velocity_bias_q24: i32,
    pub gps_tangential_velocity_bias_q24: i32,
    pub noise_scale_ppm: i32,
}
impl SensorParameters {
    pub const DEFAULT: Self = Self {
        accelerometer_bias_q28: 0,
        gyro_bias_q24: 0,
        altimeter_bias_q12: 0,
        gps_radial_position_bias_q12: 0,
        gps_downrange_bias_q32: 0,
        gps_radial_velocity_bias_q24: 0,
        gps_tangential_velocity_bias_q24: 0,
        noise_scale_ppm: 0,
    };
    pub const fn is_valid(self) -> bool {
        self.noise_scale_ppm >= -1_000_000 && self.noise_scale_ppm <= 1_000_000
    }
}
impl Default for SensorParameters {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GpsSample {
    radius: i32,
    downrange: i32,
    radial_velocity: i32,
    tangential_velocity: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XorShift32 {
    state: u32,
}
impl XorShift32 {
    pub const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x6d2b_79f5 } else { seed },
        }
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
    pub const fn state(self) -> u32 {
        self.state
    }
}

fn triangular(prng: &mut XorShift32, amplitude: i32) -> i32 {
    if amplitude <= 0 {
        return 0;
    }
    let span = amplitude as u32 + 1;
    (prng.next_u32() % span) as i32 - (prng.next_u32() % span) as i32
}
fn scale_ppm(value: i32, delta_ppm: i32) -> i32 {
    if delta_ppm == 0 {
        value
    } else {
        ((value as i64 * (1_000_000i64 + delta_ppm as i64)) / 1_000_000i64) as i32
    }
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
fn map_phase(phase: ksa64_core::planar::StagePhase) -> StagePhase {
    match phase {
        ksa64_core::planar::StagePhase::CoastBeforeIgnition => StagePhase::CoastBeforeIgnition,
        ksa64_core::planar::StagePhase::Burning => StagePhase::Burning,
        ksa64_core::planar::StagePhase::CoastBeforeSeparation => StagePhase::CoastBeforeSeparation,
        ksa64_core::planar::StagePhase::Complete => StagePhase::Complete,
    }
}
fn map_events(events: u16) -> u16 {
    let mut out = 0;
    if events & WORLD_IGNITION != 0 {
        out |= EVENT_IGNITION
    }
    if events & WORLD_CUTOFF != 0 {
        out |= EVENT_CUTOFF
    }
    if events & WORLD_SEPARATION != 0 {
        out |= EVENT_SEPARATION
    }
    out
}

pub struct SensorSuite {
    prng: XorShift32,
    faults: SensorFaults,
    previous_pitch: u16,
    altitude_delay: [Option<i32>; 1],
    gps_delay: [Option<GpsSample>; 2],
    gps_was_valid: bool,
    checksum: u32,
    parameters: SensorParameters,
}

impl SensorSuite {
    pub const fn new(seed: u32, faults: SensorFaults) -> Self {
        Self::new_parameterized(seed, faults, SensorParameters::DEFAULT)
    }
    pub const fn new_parameterized(
        seed: u32,
        faults: SensorFaults,
        parameters: SensorParameters,
    ) -> Self {
        Self {
            prng: XorShift32::new(seed),
            faults,
            previous_pitch: 0,
            altitude_delay: [None],
            gps_delay: [None, None],
            gps_was_valid: false,
            checksum: 2_166_136_261,
            parameters,
        }
    }
    pub const fn checksum(&self) -> u32 {
        self.checksum
    }
    pub const fn prng_state(&self) -> u32 {
        self.prng.state()
    }

    pub fn sample(&mut self, world: WorldSnapshot, steering: SteeringSnapshot) -> SensorFrame {
        let truth = world.truth;
        let step = truth.step();
        let planar_world = PlanarWorld::simple_earth(ksa64_core::quantities::Time::from_raw(8_192));
        let mut status = ksa64_core::numeric::NumericStatus::CLEAR;
        let vacuum = evaluate_vacuum(planar_world, truth, &mut status);
        let accel_noise = scale_ppm(ACCEL_NOISE_Q28, self.parameters.noise_scale_ppm);
        let gyro_noise = scale_ppm(GYRO_NOISE_Q24, self.parameters.noise_scale_ppm);
        let alt_noise = scale_ppm(ALT_NOISE_Q12, self.parameters.noise_scale_ppm);
        let gps_position_noise = scale_ppm(GPS_POSITION_NOISE_Q12, self.parameters.noise_scale_ppm);
        let gps_angle_noise = scale_ppm(GPS_ANGLE_NOISE_Q32, self.parameters.noise_scale_ppm);
        let gps_velocity_noise = scale_ppm(GPS_VELOCITY_NOISE_Q24, self.parameters.noise_scale_ppm);
        let proper_radial = truth.radial_acceleration().raw() - vacuum.radial_acceleration().raw();
        let accel_radial = quantize(
            proper_radial
                + ACCEL_BIAS_Q28
                + self.parameters.accelerometer_bias_q28
                + triangular(&mut self.prng, accel_noise),
            ACCEL_RESOLUTION_Q28,
        );
        let accel_tangential = quantize(
            truth.tangential_acceleration().raw()
                + ACCEL_BIAS_Q28
                + self.parameters.accelerometer_bias_q28
                + triangular(&mut self.prng, accel_noise),
            ACCEL_RESOLUTION_Q28,
        );
        let pitch_delta = steering.applied as i32 - self.previous_pitch as i32;
        self.previous_pitch = steering.applied;
        let gyro_true_q24 = ((pitch_delta as i64 * 45i64 * (1i64 << 24)) / 1024i64) as i32;
        let gyro = quantize(
            gyro_true_q24
                + GYRO_BIAS_Q24
                + self.parameters.gyro_bias_q24
                + triangular(&mut self.prng, gyro_noise),
            GYRO_RESOLUTION_Q24,
        );
        let true_altitude = truth.radius().raw() - EARTH_RADIUS_Q12;
        let alt_new = if step & 1 == 0 && true_altitude <= 80 * 4096 {
            Some(quantize(
                true_altitude
                    + ALT_BIAS_Q12
                    + self.parameters.altimeter_bias_q12
                    + triangular(&mut self.prng, alt_noise),
                ALT_RESOLUTION_Q12,
            ))
        } else {
            None
        };
        let altitude = self.altitude_delay[0];
        self.altitude_delay[0] = alt_new;
        let gps_new = if step & 7 == 0 && step >= GPS_ACQUIRE_STEP {
            Some(GpsSample {
                radius: quantize(
                    truth.radius().raw()
                        + self.parameters.gps_radial_position_bias_q12
                        + triangular(&mut self.prng, gps_position_noise),
                    GPS_POSITION_RESOLUTION_Q12,
                ),
                downrange: quantize(
                    truth
                        .downrange()
                        .raw()
                        .wrapping_add(self.parameters.gps_downrange_bias_q32)
                        + triangular(&mut self.prng, gps_angle_noise),
                    GPS_ANGLE_RESOLUTION_Q32,
                ),
                radial_velocity: quantize(
                    truth.radial_velocity().raw()
                        + self.parameters.gps_radial_velocity_bias_q24
                        + triangular(&mut self.prng, gps_velocity_noise),
                    GPS_VELOCITY_RESOLUTION_Q24,
                ),
                tangential_velocity: quantize(
                    vacuum.tangential_velocity().raw()
                        + self.parameters.gps_tangential_velocity_bias_q24
                        + triangular(&mut self.prng, gps_velocity_noise),
                    GPS_VELOCITY_RESOLUTION_Q24,
                ),
            })
        } else {
            None
        };
        let gps = self.gps_delay[0];
        self.gps_delay[0] = self.gps_delay[1];
        self.gps_delay[1] = gps_new;
        let alt_valid = altitude.is_some()
            && !self
                .faults
                .altimeter_dropout
                .map(|w| w.contains(step))
                .unwrap_or(false);
        let gps_valid = gps.is_some()
            && !self
                .faults
                .gps_outage
                .map(|w| w.contains(step))
                .unwrap_or(false);
        let mut events = map_events(world.events);
        if gps_valid && !self.gps_was_valid {
            events |= EVENT_GPS_ACQUIRED
        }
        if !gps_valid && self.gps_was_valid {
            events |= EVENT_GPS_LOST
        }
        self.gps_was_valid = gps_valid;
        let gps_value = gps.unwrap_or_default();
        let clock_drift = ((truth.time().raw() as i64 * 20) / 1_000_000) as i32;
        let frame = SensorFrame {
            sequence: step,
            onboard_time_q16: truth.time().raw() + clock_drift,
            accel_radial_q28: accel_radial,
            accel_tangential_q28: accel_tangential,
            gyro_rate_q24: gyro,
            steering_pitch: steering.applied,
            validity: SENSOR_VALID_ACCEL
                | SENSOR_VALID_GYRO
                | SENSOR_VALID_STEERING
                | SENSOR_VALID_CLOCK
                | if alt_valid { SENSOR_VALID_ALTIMETER } else { 0 }
                | if gps_valid { SENSOR_VALID_GPS } else { 0 },
            altitude_q12: altitude.unwrap_or(0),
            gps_radius_q12: gps_value.radius,
            gps_downrange_q32: gps_value.downrange,
            gps_radial_velocity_q24: gps_value.radial_velocity,
            gps_tangential_velocity_q24: gps_value.tangential_velocity,
            events,
            active_stage: truth.active_stage(),
            stage_phase: map_phase(truth.stage_phase()),
            engine_on: truth.stage_phase() == ksa64_core::planar::StagePhase::Burning,
        };
        let mut bytes = [0u8; ksa64_interface::SENSOR_FRAME_LENGTH];
        let _ = ksa64_interface::write_sensor_frame(&frame, &mut bytes);
        self.checksum = rolling_hash(self.checksum, &bytes);
        frame
    }
}

fn rolling_hash(mut hash: u32, bytes: &[u8]) -> u32 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(16_777_619);
        i += 1
    }
    hash
}
