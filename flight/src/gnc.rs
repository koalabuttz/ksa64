//! Phase 3 navigation, guidance, control, sequencing, and abort logic.

use crate::navigation::{Navigation, NavigationState};
use ksa64_interface::{
    parse_sensor_frame, ActuatorCommand, EngineAction, FlightMode, FlightOutput, SensorFrame,
    StagePhase, ALARM_ABORT, ALARM_NAVIGATION, ALARM_SENSOR_FRAME, ALARM_STEERING,
};

pub const STAGE1_CUTOFF_STEP: u32 = 1240;
pub const STAGE2_HANDOVER_STEPS: u32 = 16;
pub const INSERTION_FEEDBACK_STEP: u32 = 1760; // T+220 s.
pub const STAGE2_FLIGHT_CUTOFF_STEP: u32 = 3171;
pub const TRACKING_LIMIT: u16 = 364; // 2 degrees.
pub const TRACKING_LIMIT_STEPS: u8 = 16;
pub const TARGET_RADIUS_Q12: i32 = 26_950_193;
pub const TARGET_TANGENTIAL_VELOCITY_Q24: i32 = 130_711_290;
const MIN_ORBIT_RADIUS_Q12: i32 = 26_862_129; // 180 km.
const PITCH_STEPS: [u32; 8] = [0, 80, 240, 560, 960, 1240, 1760, 3200];
const PITCH_ANGLES: [u16; 8] = [0, 0, 3160, 7133, 9065, 13266, 16220, 16384];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightStatus {
    pub mode: FlightMode,
    pub alarms: u16,
    pub abort_latched: bool,
    pub tracking_bad_steps: u8,
    pub stage2_ignition_step: u32,
    pub checksum: u32,
}

pub struct FlightComputer {
    navigation: Navigation,
    status: FlightStatus,
    last_desired_pitch: u16,
}
impl FlightComputer {
    pub const fn new() -> Self {
        Self {
            navigation: Navigation::new(),
            status: FlightStatus {
                mode: FlightMode::Boot,
                alarms: 0,
                abort_latched: false,
                tracking_bad_steps: 0,
                stage2_ignition_step: u32::MAX,
                checksum: 2_166_136_261,
            },
            last_desired_pitch: 0,
        }
    }
    pub const fn status(&self) -> FlightStatus {
        self.status
    }
    pub const fn navigation(&self) -> NavigationState {
        self.navigation.state()
    }

    pub fn step(&mut self, frame: &SensorFrame) -> FlightOutput {
        let nav = match self.navigation.update(frame) {
            Ok(state) => state,
            Err(_) => {
                self.latch_abort(ALARM_NAVIGATION);
                return self.safe_output(frame.sequence);
            }
        };
        if self.status.abort_latched {
            return self.safe_output(frame.sequence);
        }
        self.update_mode(frame);
        if self.status.mode == FlightMode::Insertion {
            self.monitor_steering(frame)
        }
        if self.status.abort_latched {
            return self.safe_output(frame.sequence);
        }
        let command = self.guidance_command(frame, nav);
        self.last_desired_pitch = command.desired_pitch;
        self.make_output(frame.sequence, nav, command)
    }

    pub fn step_serialized(&mut self, bytes: &[u8]) -> FlightOutput {
        match parse_sensor_frame(bytes) {
            Ok(frame) => self.step(&frame),
            Err(_) => {
                self.latch_abort(ALARM_SENSOR_FRAME);
                self.safe_output(self.navigation.state().sequence)
            }
        }
    }

    fn update_mode(&mut self, frame: &SensorFrame) {
        if frame.stage_phase == StagePhase::Complete {
            self.status.mode = FlightMode::Coast;
            return;
        }
        if frame.active_stage == 0 {
            self.status.mode = if frame.stage_phase == StagePhase::CoastBeforeSeparation {
                FlightMode::StageTransition
            } else {
                FlightMode::ProgrammedAscent
            };
            return;
        }
        if frame.active_stage == 1
            && frame.engine_on
            && self.status.stage2_ignition_step == u32::MAX
        {
            self.status.stage2_ignition_step = frame.sequence
        }
        self.status.mode = if self.status.stage2_ignition_step != u32::MAX
            && frame.sequence
                >= self
                    .status
                    .stage2_ignition_step
                    .saturating_add(STAGE2_HANDOVER_STEPS)
        {
            FlightMode::Insertion
        } else {
            FlightMode::StageTransition
        };
    }

    fn guidance_command(&mut self, frame: &SensorFrame, nav: NavigationState) -> ActuatorCommand {
        let sequence = frame.sequence;
        let mut command = ActuatorCommand {
            sequence,
            desired_pitch: pitch_program(sequence.saturating_add(11)),
            engine_action: EngineAction::Hold,
            separate: false,
            abort_safeing: false,
            recovery_requested: false,
        };
        if frame.active_stage == 0 {
            if frame.stage_phase == StagePhase::CoastBeforeIgnition {
                command.engine_action = EngineAction::Ignite
            }
            if sequence.saturating_add(1) >= STAGE1_CUTOFF_STEP
                && frame.stage_phase == StagePhase::Burning
            {
                command.engine_action = EngineAction::Cutoff
            }
            if frame.stage_phase == StagePhase::CoastBeforeSeparation {
                command.separate = true
            }
        } else if frame.active_stage == 1 {
            if frame.stage_phase == StagePhase::CoastBeforeIgnition {
                command.engine_action = EngineAction::Ignite
            }
            if self.status.mode == FlightMode::Insertion && sequence >= INSERTION_FEEDBACK_STEP {
                command.desired_pitch = insertion_pitch(sequence, nav)
            }
            if frame.stage_phase == StagePhase::Burning && should_cutoff(sequence, nav) {
                command.engine_action = EngineAction::Cutoff;
                self.status.mode = FlightMode::Coast
            }
        }
        command
    }

    fn monitor_steering(&mut self, frame: &SensorFrame) {
        let error = circular_error(self.last_desired_pitch, frame.steering_pitch);
        if error > TRACKING_LIMIT {
            self.status.tracking_bad_steps = self.status.tracking_bad_steps.saturating_add(1)
        } else {
            self.status.tracking_bad_steps = 0
        }
        if self.status.tracking_bad_steps >= TRACKING_LIMIT_STEPS {
            self.latch_abort(ALARM_STEERING)
        }
    }
    fn latch_abort(&mut self, alarm: u16) {
        self.status.abort_latched = true;
        self.status.mode = FlightMode::Abort;
        self.status.alarms |= alarm | ALARM_ABORT
    }
    fn safe_output(&mut self, sequence: u32) -> FlightOutput {
        let nav = self.navigation.state();
        let command = ActuatorCommand {
            sequence,
            desired_pitch: self.last_desired_pitch,
            engine_action: EngineAction::Cutoff,
            separate: false,
            abort_safeing: true,
            recovery_requested: true,
        };
        self.make_output(sequence, nav, command)
    }
    fn make_output(
        &mut self,
        sequence: u32,
        nav: NavigationState,
        command: ActuatorCommand,
    ) -> FlightOutput {
        self.status.checksum = hash_flight(self.status.checksum, self.status, command);
        FlightOutput {
            sequence,
            nav_time_q16: nav.time_q16,
            nav_radius_q12: nav.radius_q12,
            nav_downrange_q32: nav.downrange_q32,
            nav_radial_velocity_q24: nav.radial_velocity_q24,
            nav_tangential_velocity_q24: nav.tangential_velocity_q24,
            nav_pitch: nav.pitch,
            mode: self.status.mode,
            alarms: self.status.alarms,
            command,
            nav_checksum: nav.checksum,
            flight_checksum: self.status.checksum,
        }
    }
}
impl Default for FlightComputer {
    fn default() -> Self {
        Self::new()
    }
}

fn pitch_program(step: u32) -> u16 {
    if step <= PITCH_STEPS[0] {
        return PITCH_ANGLES[0];
    }
    let last = PITCH_STEPS.len() - 1;
    if step >= PITCH_STEPS[last] {
        return PITCH_ANGLES[last];
    }
    let mut i = 0;
    while i < last {
        if step < PITCH_STEPS[i + 1] {
            let span = PITCH_STEPS[i + 1] - PITCH_STEPS[i];
            let offset = step - PITCH_STEPS[i];
            let range = PITCH_ANGLES[i + 1] as i32 - PITCH_ANGLES[i] as i32;
            return (PITCH_ANGLES[i] as i32
                + (range * offset as i32 + span as i32 / 2) / span as i32)
                as u16;
        }
        i += 1
    }
    PITCH_ANGLES[last]
}
fn insertion_pitch(sequence: u32, nav: NavigationState) -> u16 {
    let base = pitch_program(sequence.saturating_add(11)) as i32;
    // Energy error biases the trusted program toward prograde without crossing
    // it. Near target altitude, a smaller radial term damps residual climb.
    let prograde_margin = (16_384 - base).max(0);
    let energy = ((TARGET_TANGENTIAL_VELOCITY_Q24 - nav.tangential_velocity_q24) >> 13)
        .clamp(0, 546)
        .min(prograde_margin);
    let radial_raw = ((nav.radial_velocity_q24 >> 10)
        + ((nav.radius_q12 - TARGET_RADIUS_Q12) >> 7))
        .clamp(-910, 910);
    let radial_ramp = (nav.radius_q12 - (TARGET_RADIUS_Q12 - 81_920)).clamp(0, 40_960);
    let radial_damping = (radial_raw * radial_ramp) / 40_960;
    (base + energy + radial_damping).clamp(12_743, 20_025) as u16
}
fn should_cutoff(sequence: u32, nav: NavigationState) -> bool {
    let in_radius = (MIN_ORBIT_RADIUS_Q12..=27_025_969).contains(&nav.radius_q12);
    let velocity_ready = nav.tangential_velocity_q24 >= TARGET_TANGENTIAL_VELOCITY_Q24 - 10_000;
    let radial_settled = nav.radial_velocity_q24.abs() <= 838_861;
    (in_radius && velocity_ready && radial_settled)
        || sequence.saturating_add(1) >= STAGE2_FLIGHT_CUTOFF_STEP
}
fn circular_error(a: u16, b: u16) -> u16 {
    let d = a.abs_diff(b);
    d.min(u16::MAX - d)
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
fn hash_flight(mut h: u32, s: FlightStatus, c: ActuatorCommand) -> u32 {
    h = hash_word(h, s.mode as u32);
    h = hash_word(h, s.alarms as u32);
    h = hash_word(h, c.sequence);
    h = hash_word(h, c.desired_pitch as u32);
    h = hash_word(h, c.engine_action as u32);
    hash_word(
        h,
        (c.separate as u32)
            | ((c.abort_safeing as u32) << 1)
            | ((c.recovery_requested as u32) << 2),
    )
}
