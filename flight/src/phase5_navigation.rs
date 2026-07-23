//! Phase 5 three-dimensional aided inertial navigation.
//!
//! This module depends only on transported measurements. Simulator truth and
//! environment internals are deliberately unavailable to the flight crate.

use ksa64_interface::phase5::{
    SpatialSensorFrame, SENSOR_VALID_BAROMETER, SENSOR_VALID_CLOCK, SENSOR_VALID_GPS,
    SENSOR_VALID_IMU, SENSOR_VALID_STAR_TRACKER,
};

pub const EARTH_RADIUS_Q12: i32 = 26_124_849;
pub const EARTH_MU_Q12: i32 = 1_632_667_410;
pub const INITIAL_POSITION_Q12: [i32; 3] = [22_958_965, 0, 12_465_701];
pub const INITIAL_VELOCITY_Q24: [i32; 3] = [0, 6_857_499, 0];
pub const INITIAL_ATTITUDE_Q30: [i32; 4] = [1_040_703_765, 0, -264_305_086, 0];
pub const GPS_POSITION_SHIFT: u8 = 1;
pub const GPS_VELOCITY_SHIFT: u8 = 3;
pub const BAROMETER_POSITION_SHIFT: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpatialNavigationError {
    Sequence,
    MissingInertial,
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpatialNavigationState {
    pub sequence: u32,
    pub time_q16: i32,
    pub position_q12: [i32; 3],
    pub velocity_q24: [i32; 3],
    pub attitude_q30: [i32; 4],
    pub angular_rate_q24: [i32; 3],
    pub gps_aided: bool,
    pub star_aided: bool,
    pub barometer_aided: bool,
    pub checksum: u32,
}

pub struct SpatialNavigation {
    state: SpatialNavigationState,
    initialized: bool,
}

impl SpatialNavigation {
    pub const fn new() -> Self {
        Self {
            state: SpatialNavigationState {
                sequence: 0,
                time_q16: 0,
                position_q12: INITIAL_POSITION_Q12,
                velocity_q24: INITIAL_VELOCITY_Q24,
                attitude_q30: INITIAL_ATTITUDE_Q30,
                angular_rate_q24: [0; 3],
                gps_aided: false,
                star_aided: false,
                barometer_aided: false,
                checksum: 2_166_136_261,
            },
            initialized: false,
        }
    }
    pub const fn state(&self) -> SpatialNavigationState {
        self.state
    }

    pub fn update(
        &mut self,
        frame: &SpatialSensorFrame,
    ) -> Result<SpatialNavigationState, SpatialNavigationError> {
        let required = SENSOR_VALID_IMU | SENSOR_VALID_CLOCK;
        if frame.validity & required != required {
            return Err(SpatialNavigationError::MissingInertial);
        }
        if self.initialized && frame.sequence != self.state.sequence.wrapping_add(1) {
            return Err(SpatialNavigationError::Sequence);
        }
        let dt_q16 = if self.initialized {
            frame.onboard_time_q16.saturating_sub(self.state.time_q16)
        } else {
            frame.onboard_time_q16
        };
        if !(0..=16_384).contains(&dt_q16) {
            return Err(SpatialNavigationError::Numeric);
        }
        if self.initialized && dt_q16 > 0 {
            self.propagate(frame, dt_q16)?;
        }
        self.state.sequence = frame.sequence;
        self.state.time_q16 = frame.onboard_time_q16;
        self.state.angular_rate_q24 = frame.gyro_body_q24;
        self.state.gps_aided = false;
        self.state.star_aided = false;
        self.state.barometer_aided = false;
        if frame.validity & SENSOR_VALID_BAROMETER != 0 {
            self.apply_barometer(frame.baro_altitude_q12)?;
            self.state.barometer_aided = true;
        }
        if frame.validity & SENSOR_VALID_GPS != 0 {
            self.apply_gps(frame);
            self.state.gps_aided = true;
        }
        if frame.validity & SENSOR_VALID_STAR_TRACKER != 0 {
            self.state.attitude_q30 = normalize_quaternion(frame.star_attitude_q30)
                .ok_or(SpatialNavigationError::Numeric)?;
            self.state.star_aided = true;
        }
        self.initialized = true;
        self.state.checksum = hash_state(self.state.checksum, self.state);
        Ok(self.state)
    }

    fn propagate(
        &mut self,
        frame: &SpatialSensorFrame,
        dt_q16: i32,
    ) -> Result<(), SpatialNavigationError> {
        let radius_q12 =
            magnitude3(self.state.position_q12).ok_or(SpatialNavigationError::Numeric)?;
        if radius_q12 <= 0 {
            return Err(SpatialNavigationError::Numeric);
        }
        let mu_over_r_q24 = ((EARTH_MU_Q12 as i64) << 24) / radius_q12 as i64;
        let gravity_magnitude_q28 = (mu_over_r_q24 << 16) / radius_q12 as i64;
        let mut gravity = [0i32; 3];
        let mut axis = 0;
        while axis < 3 {
            let unit_q30 = ((self.state.position_q12[axis] as i64) << 30) / radius_q12 as i64;
            gravity[axis] = -((unit_q30 * gravity_magnitude_q28) >> 30) as i32;
            axis += 1;
        }
        let proper_eci = rotate_vector(self.state.attitude_q30, frame.accel_body_q28)
            .ok_or(SpatialNavigationError::Numeric)?;
        axis = 0;
        while axis < 3 {
            let acceleration = proper_eci[axis] as i64 + gravity[axis] as i64;
            let velocity =
                self.state.velocity_q24[axis] as i64 + ((acceleration * dt_q16 as i64) >> 20);
            let position =
                self.state.position_q12[axis] as i64 + ((velocity * dt_q16 as i64) >> 28);
            if velocity < i32::MIN as i64
                || velocity > i32::MAX as i64
                || position < i32::MIN as i64
                || position > i32::MAX as i64
            {
                return Err(SpatialNavigationError::Numeric);
            }
            self.state.velocity_q24[axis] = velocity as i32;
            self.state.position_q12[axis] = position as i32;
            axis += 1;
        }
        self.state.attitude_q30 =
            integrate_quaternion(self.state.attitude_q30, frame.gyro_body_q24, dt_q16)
                .ok_or(SpatialNavigationError::Numeric)?;
        Ok(())
    }

    fn apply_gps(&mut self, frame: &SpatialSensorFrame) {
        let mut axis = 0;
        while axis < 3 {
            // GPS is transported with two 0.125 s frames of latency. Project
            // its position to the current epoch before applying bounded gains.
            let projected = frame.gps_position_q12[axis]
                .saturating_add(((frame.gps_velocity_q24[axis] as i64 * 16_384i64) >> 28) as i32);
            self.state.position_q12[axis] = self.state.position_q12[axis].saturating_add(
                projected.saturating_sub(self.state.position_q12[axis]) >> GPS_POSITION_SHIFT,
            );
            self.state.velocity_q24[axis] = self.state.velocity_q24[axis].saturating_add(
                frame.gps_velocity_q24[axis].saturating_sub(self.state.velocity_q24[axis])
                    >> GPS_VELOCITY_SHIFT,
            );
            axis += 1;
        }
    }

    fn apply_barometer(&mut self, altitude_q12: i32) -> Result<(), SpatialNavigationError> {
        let radius = magnitude3(self.state.position_q12).ok_or(SpatialNavigationError::Numeric)?;
        if radius <= 0 {
            return Err(SpatialNavigationError::Numeric);
        }
        let measured_radius = EARTH_RADIUS_Q12.saturating_add(altitude_q12);
        let correction = measured_radius.saturating_sub(radius) >> BAROMETER_POSITION_SHIFT;
        let mut axis = 0;
        while axis < 3 {
            let delta =
                ((self.state.position_q12[axis] as i64 * correction as i64) / radius as i64) as i32;
            self.state.position_q12[axis] = self.state.position_q12[axis].saturating_add(delta);
            axis += 1;
        }
        Ok(())
    }
}

impl Default for SpatialNavigation {
    fn default() -> Self {
        Self::new()
    }
}

fn rotate_vector(quaternion: [i32; 4], vector: [i32; 3]) -> Option<[i32; 3]> {
    let q = quaternion;
    let cross = [
        ((q[2] as i64 * vector[2] as i64 - q[3] as i64 * vector[1] as i64) >> 30),
        ((q[3] as i64 * vector[0] as i64 - q[1] as i64 * vector[2] as i64) >> 30),
        ((q[1] as i64 * vector[1] as i64 - q[2] as i64 * vector[0] as i64) >> 30),
    ];
    let twice = [cross[0] * 2, cross[1] * 2, cross[2] * 2];
    let second_cross = [
        (q[2] as i64 * twice[2] - q[3] as i64 * twice[1]) >> 30,
        (q[3] as i64 * twice[0] - q[1] as i64 * twice[2]) >> 30,
        (q[1] as i64 * twice[1] - q[2] as i64 * twice[0]) >> 30,
    ];
    let mut out = [0; 3];
    let mut axis = 0;
    while axis < 3 {
        let value = vector[axis] as i64 + ((q[0] as i64 * twice[axis]) >> 30) + second_cross[axis];
        if value < i32::MIN as i64 || value > i32::MAX as i64 {
            return None;
        }
        out[axis] = value as i32;
        axis += 1;
    }
    Some(out)
}

fn integrate_quaternion(q: [i32; 4], rate: [i32; 3], dt_q16: i32) -> Option<[i32; 4]> {
    let products_q24 = [
        -(((q[1] as i64 * rate[0] as i64) >> 30)
            + ((q[2] as i64 * rate[1] as i64) >> 30)
            + ((q[3] as i64 * rate[2] as i64) >> 30)),
        ((q[0] as i64 * rate[0] as i64) >> 30) + ((q[2] as i64 * rate[2] as i64) >> 30)
            - ((q[3] as i64 * rate[1] as i64) >> 30),
        ((q[0] as i64 * rate[1] as i64) >> 30) + ((q[3] as i64 * rate[0] as i64) >> 30)
            - ((q[1] as i64 * rate[2] as i64) >> 30),
        ((q[0] as i64 * rate[2] as i64) >> 30) + ((q[1] as i64 * rate[1] as i64) >> 30)
            - ((q[2] as i64 * rate[0] as i64) >> 30),
    ];
    let mut next = [0; 4];
    let mut component = 0;
    while component < 4 {
        let value = q[component] as i64 + ((products_q24[component] * dt_q16 as i64) >> 11);
        if value < i32::MIN as i64 || value > i32::MAX as i64 {
            return None;
        }
        next[component] = value as i32;
        component += 1;
    }
    normalize_quaternion(next)
}

fn normalize_quaternion(q: [i32; 4]) -> Option<[i32; 4]> {
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
    let mut normalized = [0; 4];
    component = 0;
    while component < 4 {
        let value = ((q[component] as i64) << 30) / norm_q30;
        if value < i32::MIN as i64 || value > i32::MAX as i64 {
            return None;
        }
        normalized[component] = value as i32;
        component += 1;
    }
    Some(normalized)
}

fn magnitude3(vector: [i32; 3]) -> Option<i32> {
    let sum = (vector[0] as i64 * vector[0] as i64) as u64
        + (vector[1] as i64 * vector[1] as i64) as u64
        + (vector[2] as i64 * vector[2] as i64) as u64;
    let root = isqrt(sum);
    if root > i32::MAX as u64 {
        None
    } else {
        Some(root as i32)
    }
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

fn hash_word(mut hash: u32, word: u32) -> u32 {
    let mut shift = 0;
    while shift < 32 {
        hash ^= (word >> shift) & 0xff;
        hash = hash.wrapping_mul(16_777_619);
        shift += 8;
    }
    hash
}
fn hash_state(mut hash: u32, state: SpatialNavigationState) -> u32 {
    hash = hash_word(hash, state.sequence);
    hash = hash_word(hash, state.time_q16 as u32);
    let mut axis = 0;
    while axis < 3 {
        hash = hash_word(hash, state.position_q12[axis] as u32);
        hash = hash_word(hash, state.velocity_q24[axis] as u32);
        hash = hash_word(hash, state.angular_rate_q24[axis] as u32);
        axis += 1;
    }
    let mut component = 0;
    while component < 4 {
        hash = hash_word(hash, state.attitude_q30[component] as u32);
        component += 1;
    }
    hash_word(
        hash,
        state.gps_aided as u32
            | ((state.star_aided as u32) << 1)
            | ((state.barometer_aided as u32) << 2),
    )
}
