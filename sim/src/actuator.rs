//! Deterministic kinematic steering actuator.

pub const MAX_PITCH: u16 = 20_025; // 110 degrees in binary-angle units.
pub const MAX_SLEW_PER_STEP: i32 = 228; // 10 deg/s at the 0.125 s Phase 2 step.
pub const LAG_SHIFT: u8 = 2; // dt/tau = 0.125/0.5 = 1/4.

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
}

impl SteeringActuator {
    pub const fn new(initial_pitch: u16) -> Self {
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
        }
    }
    pub fn set_stuck(&mut self, stuck: bool) {
        self.stuck = stuck;
    }
    pub fn advance(&mut self, request: u16) -> SteeringSnapshot {
        self.requested = request.min(MAX_PITCH);
        let requested_q8 = (self.requested as i32) << 8;
        self.lagged_target_q8 += (requested_q8 - self.lagged_target_q8) >> LAG_SHIFT;
        if !self.stuck {
            let target = (self.lagged_target_q8 + 128) >> 8;
            let current = self.applied as i32;
            let delta = (target - current).clamp(-MAX_SLEW_PER_STEP, MAX_SLEW_PER_STEP);
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
