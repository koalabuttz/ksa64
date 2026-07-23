//! Deterministic kinematic steering actuator.

pub const MAX_PITCH: u16 = 20_025; // 110 degrees in binary-angle units.
pub const MAX_SLEW_PER_STEP: i32 = 228; // 10 deg/s at the 0.125 s Phase 2 step.
pub const LAG_SHIFT: u8 = 2; // dt/tau = 0.125/0.5 = 1/4.
pub const DEFAULT_LAG_STEPS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActuatorParameters {
    pub lag_steps: u8,
    pub max_slew_per_step: i32,
}
impl ActuatorParameters {
    pub const DEFAULT: Self = Self {
        lag_steps: DEFAULT_LAG_STEPS,
        max_slew_per_step: MAX_SLEW_PER_STEP,
    };
    pub const fn is_valid(self) -> bool {
        self.lag_steps >= 1 && self.lag_steps <= 16 && self.max_slew_per_step >= 1
    }
}
impl Default for ActuatorParameters {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SteeringSnapshot {
    pub requested: u16,
    pub lagged_target: u16,
    pub applied: u16,
    pub stuck: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SteeringActuator {
    requested: u16,
    lagged_target_q8: i32,
    applied: u16,
    stuck: bool,
    parameters: ActuatorParameters,
}

impl SteeringActuator {
    pub const fn new(initial_pitch: u16) -> Self {
        Self::new_parameterized(initial_pitch, ActuatorParameters::DEFAULT)
    }
    pub const fn new_parameterized(initial_pitch: u16, parameters: ActuatorParameters) -> Self {
        let pitch = if initial_pitch > MAX_PITCH {
            MAX_PITCH
        } else {
            initial_pitch
        };
        Self {
            requested: pitch,
            lagged_target_q8: (pitch as i32) << 8,
            applied: pitch,
            stuck: false,
            parameters,
        }
    }
    pub const fn parameters(self) -> ActuatorParameters {
        self.parameters
    }
    pub fn set_stuck(&mut self, stuck: bool) {
        self.stuck = stuck;
    }
    pub fn jam_at(&mut self, pitch: u16) {
        let jammed = pitch.min(MAX_PITCH);
        self.applied = jammed;
        self.lagged_target_q8 = (jammed as i32) << 8;
        self.stuck = true;
    }
    pub fn advance(&mut self, request: u16) -> SteeringSnapshot {
        self.requested = request.min(MAX_PITCH);
        let requested_q8 = (self.requested as i32) << 8;
        let lag_error = requested_q8 - self.lagged_target_q8;
        self.lagged_target_q8 += if self.parameters.lag_steps == DEFAULT_LAG_STEPS {
            lag_error >> LAG_SHIFT
        } else {
            lag_error / self.parameters.lag_steps.max(1) as i32
        };
        if !self.stuck {
            let target = (self.lagged_target_q8 + 128) >> 8;
            let current = self.applied as i32;
            let limit = self.parameters.max_slew_per_step.max(1);
            let delta = (target - current).clamp(-limit, limit);
            self.applied = (current + delta).clamp(0, MAX_PITCH as i32) as u16;
        }
        self.snapshot()
    }
    pub const fn snapshot(self) -> SteeringSnapshot {
        SteeringSnapshot {
            requested: self.requested,
            lagged_target: ((self.lagged_target_q8 + 128) >> 8) as u16,
            applied: self.applied,
            stuck: self.stuck,
        }
    }
}
