//! Deterministic aided inertial navigation. No simulator truth is available here.

use ksa64_interface::{
    SensorFrame, SENSOR_VALID_ACCEL, SENSOR_VALID_ALTIMETER, SENSOR_VALID_CLOCK, SENSOR_VALID_GPS,
    SENSOR_VALID_GYRO, SENSOR_VALID_STEERING,
};

pub const EARTH_RADIUS_Q12: i32 = 26_124_849;
pub const EARTH_MU_Q12: i32 = 1_632_667_410;
pub const EARTH_ROTATION_RAD_Q30: i32 = 78_298;
pub const INITIAL_TANGENTIAL_VELOCITY_Q24: i32 = 7_803_689;
pub const ALT_ALPHA_SHIFT: u8 = 3;
pub const ALT_BETA_SHIFT: u8 = 1;
pub const GPS_POSITION_SHIFT: u8 = 1;
pub const GPS_VELOCITY_SHIFT: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationError {
    Sequence,
    MissingInertial,
    Numeric,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigationState {
    pub sequence: u32,
    pub time_q16: i32,
    pub radius_q12: i32,
    pub downrange_q32: i32,
    pub radial_velocity_q24: i32,
    pub tangential_velocity_q24: i32,
    pub pitch: u16,
    pub gps_aided: bool,
    pub altitude_aided: bool,
    pub checksum: u32,
}

pub struct Navigation {
    state: NavigationState,
    initialized: bool,
}
impl Navigation {
    pub const fn new() -> Self {
        Self {
            state: NavigationState {
                sequence: 0,
                time_q16: 0,
                radius_q12: EARTH_RADIUS_Q12,
                downrange_q32: 0,
                radial_velocity_q24: 0,
                tangential_velocity_q24: INITIAL_TANGENTIAL_VELOCITY_Q24,
                pitch: 0,
                gps_aided: false,
                altitude_aided: false,
                checksum: 2_166_136_261,
            },
            initialized: false,
        }
    }
    pub const fn state(&self) -> NavigationState {
        self.state
    }
    pub fn update(&mut self, frame: &SensorFrame) -> Result<NavigationState, NavigationError> {
        let required =
            SENSOR_VALID_ACCEL | SENSOR_VALID_GYRO | SENSOR_VALID_STEERING | SENSOR_VALID_CLOCK;
        if frame.validity & required != required {
            return Err(NavigationError::MissingInertial);
        }
        if self.initialized && frame.sequence != self.state.sequence.wrapping_add(1) {
            return Err(NavigationError::Sequence);
        }
        let dt = if self.initialized {
            frame.onboard_time_q16 - self.state.time_q16
        } else {
            frame.onboard_time_q16
        };
        if !(0..=16_384).contains(&dt) {
            return Err(NavigationError::Numeric);
        }
        if self.initialized && dt > 0 {
            self.propagate(frame, dt)?
        }
        self.state.sequence = frame.sequence;
        self.state.time_q16 = frame.onboard_time_q16;
        self.state.pitch = frame.steering_pitch;
        self.state.altitude_aided = false;
        self.state.gps_aided = false;
        if frame.validity & SENSOR_VALID_ALTIMETER != 0 {
            self.apply_altimeter(frame.altitude_q12);
            self.state.altitude_aided = true
        }
        if frame.validity & SENSOR_VALID_GPS != 0 {
            self.apply_gps(frame);
            self.state.gps_aided = true
        }
        self.initialized = true;
        self.state.checksum = hash_state(self.state.checksum, self.state);
        Ok(self.state)
    }
    fn propagate(&mut self, frame: &SensorFrame, dt: i32) -> Result<(), NavigationError> {
        let radius = self.state.radius_q12 as i64;
        if radius <= 0 {
            return Err(NavigationError::Numeric);
        }
        let mu_over_r_q24 = ((EARTH_MU_Q12 as i64) << 24) / radius;
        let gravity_q28 = (mu_over_r_q24 << 16) / radius;
        let vt = self.state.tangential_velocity_q24 as i64;
        let vt2_q20 = (vt * vt) >> 28;
        let centrifugal_q28 = (vt2_q20 << 20) / radius;
        let radial_accel = frame.accel_radial_q28 as i64 + centrifugal_q28 - gravity_q28;
        let tangential_accel = frame.accel_tangential_q28 as i64;
        let vr = self.state.radial_velocity_q24 as i64;
        let geometry_accel_q28 = -(((vr * vt) >> 8) / radius);
        let new_vr = vr + ((radial_accel * dt as i64) >> 20);
        let new_vt = vt + (((tangential_accel + geometry_accel_q28) * dt as i64) >> 20);
        let new_radius = radius + ((new_vr * dt as i64) >> 28);
        let atmosphere_vt = (EARTH_ROTATION_RAD_Q30 as i64 * new_radius) >> 18;
        let relative_vt = new_vt - atmosphere_vt;
        let angle_rad_q28 = (relative_vt * dt as i64) / new_radius;
        let delta_turns_q32 = (angle_rad_q28 * 170_891_319i64) >> 26;
        let gyro_delta = (((frame.gyro_rate_q24 as i64 * dt as i64) >> 24) / 360) as i32;
        if new_radius <= 0
            || new_vr < i32::MIN as i64
            || new_vr > i32::MAX as i64
            || new_vt < i32::MIN as i64
            || new_vt > i32::MAX as i64
        {
            return Err(NavigationError::Numeric);
        }
        self.state.radius_q12 = new_radius as i32;
        self.state.radial_velocity_q24 = new_vr as i32;
        self.state.tangential_velocity_q24 = new_vt as i32;
        self.state.downrange_q32 = self
            .state
            .downrange_q32
            .wrapping_add(delta_turns_q32 as i32);
        self.state.pitch = self.state.pitch.wrapping_add(gyro_delta as u16);
        Ok(())
    }
    fn apply_altimeter(&mut self, altitude: i32) {
        let measured = EARTH_RADIUS_Q12.saturating_add(altitude);
        let error = measured.saturating_sub(self.state.radius_q12);
        self.state.radius_q12 = self
            .state
            .radius_q12
            .saturating_add(error >> ALT_ALPHA_SHIFT);
        self.state.radial_velocity_q24 = self
            .state
            .radial_velocity_q24
            .saturating_add((error << 12) >> (ALT_BETA_SHIFT + 2));
    }
    fn apply_gps(&mut self, frame: &SensorFrame) {
        // GPS PVT is delivered through a fixed two-step (0.25 s) transport
        // delay. Project the position components to the current measurement
        // epoch before applying the aiding correction.
        let measured_radius = frame
            .gps_radius_q12
            .saturating_add(frame.gps_radial_velocity_q24 >> 14);
        let radius = frame.gps_radius_q12 as i64;
        let atmosphere_vt = (EARTH_ROTATION_RAD_Q30 as i64 * radius) >> 18;
        let relative_vt = frame.gps_tangential_velocity_q24 as i64 - atmosphere_vt;
        let angle_rad_q28 = (relative_vt * 16_384i64) / radius;
        let delayed_turns = ((angle_rad_q28 * 170_891_319i64) >> 26) as i32;
        let measured_downrange = frame.gps_downrange_q32.wrapping_add(delayed_turns);
        self.state.radius_q12 = self
            .state
            .radius_q12
            .saturating_add((measured_radius - self.state.radius_q12) >> GPS_POSITION_SHIFT);
        let angle_error = measured_downrange.wrapping_sub(self.state.downrange_q32);
        self.state.downrange_q32 = self
            .state
            .downrange_q32
            .wrapping_add(angle_error >> GPS_POSITION_SHIFT);
        self.state.radial_velocity_q24 = self.state.radial_velocity_q24.saturating_add(
            (frame.gps_radial_velocity_q24 - self.state.radial_velocity_q24) >> GPS_VELOCITY_SHIFT,
        );
        self.state.tangential_velocity_q24 = self.state.tangential_velocity_q24.saturating_add(
            (frame.gps_tangential_velocity_q24 - self.state.tangential_velocity_q24)
                >> GPS_VELOCITY_SHIFT,
        );
    }
}
impl Default for Navigation {
    fn default() -> Self {
        Self::new()
    }
}
fn hash_word(mut h: u32, w: u32) -> u32 {
    let mut s = 0;
    while s < 32 {
        h ^= (w >> s) & 0xff;
        h = h.wrapping_mul(16_777_619);
        s += 8
    }
    h
}
fn hash_state(mut h: u32, s: NavigationState) -> u32 {
    h = hash_word(h, s.sequence);
    h = hash_word(h, s.time_q16 as u32);
    h = hash_word(h, s.radius_q12 as u32);
    h = hash_word(h, s.downrange_q32 as u32);
    h = hash_word(h, s.radial_velocity_q24 as u32);
    h = hash_word(h, s.tangential_velocity_q24 as u32);
    h = hash_word(h, s.pitch as u32);
    hash_word(h, (s.gps_aided as u32) | ((s.altitude_aided as u32) << 1))
}
