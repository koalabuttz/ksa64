//! Phase 5 spatial guidance, attitude control, sequencing, and fail-closed logic.

use crate::phase5_navigation::{SpatialNavigation, SpatialNavigationError, SpatialNavigationState};
use ksa64_interface::phase5::{
    parse_spatial_sensor_frame, SpatialActuatorCommand, SpatialSensorFrame, SENSOR_VALID_ACTUATOR,
};
use ksa64_interface::{
    EngineAction, FlightMode, StagePhase, ALARM_ABORT, ALARM_COMMAND_REJECTED, ALARM_NAVIGATION,
    ALARM_SENSOR_FRAME, ALARM_STEERING,
};

pub const STAGE1_CUTOFF_STEP: u32 = 1240;
pub const STAGE2_FLIGHT_CUTOFF_STEP: u32 = 3171;
pub const GIMBAL_LIMIT_Q16: i32 = 6_863;
pub const RCS_LIMIT_Q15: i32 = 32_767;
pub const TRACKING_LIMIT_Q16: i32 = 2_288;
pub const TRACKING_LIMIT_STEPS: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpatialGuidanceTarget {
    pub attitude_q30: [i32; 4],
    pub angular_rate_q24: [i32; 3],
}
impl SpatialGuidanceTarget {
    pub const fn hold(attitude_q30: [i32; 4]) -> Self {
        Self {
            attitude_q30,
            angular_rate_q24: [0; 3],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpatialFlightStatus {
    pub mode: FlightMode,
    pub alarms: u16,
    pub abort_latched: bool,
    pub tracking_bad_steps: u8,
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpatialFlightOutput {
    pub sequence: u32,
    pub navigation: SpatialNavigationState,
    pub mode: FlightMode,
    pub alarms: u16,
    pub command: SpatialActuatorCommand,
    pub flight_checksum: u32,
}

pub struct SpatialFlightComputer {
    navigation: SpatialNavigation,
    status: SpatialFlightStatus,
    previous_command: SpatialActuatorCommand,
}

impl SpatialFlightComputer {
    pub const fn new() -> Self {
        Self {
            navigation: SpatialNavigation::new(),
            status: SpatialFlightStatus {
                mode: FlightMode::Boot,
                alarms: 0,
                abort_latched: false,
                tracking_bad_steps: 0,
                checksum: 2_166_136_261,
            },
            previous_command: SpatialActuatorCommand::SAFE,
        }
    }
    pub const fn status(&self) -> SpatialFlightStatus {
        self.status
    }
    pub const fn navigation(&self) -> SpatialNavigationState {
        self.navigation.state()
    }

    pub fn step(
        &mut self,
        frame: &SpatialSensorFrame,
        target: SpatialGuidanceTarget,
    ) -> SpatialFlightOutput {
        let navigation = match self.navigation.update(frame) {
            Ok(value) => value,
            Err(_) => {
                self.latch_abort(ALARM_NAVIGATION);
                return self.safe_output(frame.sequence);
            }
        };
        if self.status.abort_latched {
            return self.safe_output(frame.sequence);
        }
        self.update_mode(frame);
        self.monitor_actuator(frame);
        if self.status.abort_latched {
            return self.safe_output(frame.sequence);
        }
        let mut command = attitude_command(frame.sequence, navigation, target);
        self.apply_sequencer(frame, &mut command);
        self.previous_command = command;
        self.make_output(frame.sequence, navigation, command)
    }

    pub fn step_serialized(
        &mut self,
        bytes: &[u8],
        target: SpatialGuidanceTarget,
    ) -> SpatialFlightOutput {
        match parse_spatial_sensor_frame(bytes) {
            Ok(frame) => self.step(&frame, target),
            Err(_) => {
                self.latch_abort(ALARM_SENSOR_FRAME);
                self.safe_output(self.navigation.state().sequence)
            }
        }
    }

    fn update_mode(&mut self, frame: &SpatialSensorFrame) {
        self.status.mode = if frame.stage_phase == StagePhase::Complete {
            FlightMode::Coast
        } else if frame.stage_phase == StagePhase::CoastBeforeSeparation {
            FlightMode::StageTransition
        } else if frame.active_stage == 0 {
            FlightMode::ProgrammedAscent
        } else if frame.engine_on {
            FlightMode::Insertion
        } else {
            FlightMode::StageTransition
        };
    }

    fn monitor_actuator(&mut self, frame: &SpatialSensorFrame) {
        if frame.validity & SENSOR_VALID_ACTUATOR == 0 {
            self.latch_abort(ALARM_COMMAND_REJECTED);
            return;
        }
        let pitch_error = frame.gimbal_applied_q16[0]
            .saturating_sub(self.previous_command.gimbal_q16[0])
            .abs();
        let yaw_error = frame.gimbal_applied_q16[1]
            .saturating_sub(self.previous_command.gimbal_q16[1])
            .abs();
        if pitch_error > TRACKING_LIMIT_Q16 || yaw_error > TRACKING_LIMIT_Q16 {
            self.status.tracking_bad_steps = self.status.tracking_bad_steps.saturating_add(1);
        } else {
            self.status.tracking_bad_steps = 0;
        }
        if self.status.tracking_bad_steps >= TRACKING_LIMIT_STEPS {
            self.latch_abort(ALARM_STEERING);
        }
    }

    fn apply_sequencer(
        &mut self,
        frame: &SpatialSensorFrame,
        command: &mut SpatialActuatorCommand,
    ) {
        if frame.stage_phase == StagePhase::CoastBeforeIgnition {
            command.engine_action = EngineAction::Ignite;
        }
        if frame.active_stage == 0
            && frame.stage_phase == StagePhase::Burning
            && frame.sequence >= STAGE1_CUTOFF_STEP
        {
            command.engine_action = EngineAction::Cutoff;
        }
        if frame.stage_phase == StagePhase::CoastBeforeSeparation {
            command.separate = true;
        }
        if frame.active_stage == 1
            && frame.stage_phase == StagePhase::Burning
            && frame.sequence >= STAGE2_FLIGHT_CUTOFF_STEP
        {
            command.engine_action = EngineAction::Cutoff;
            self.status.mode = FlightMode::Coast;
        }
    }

    fn latch_abort(&mut self, alarm: u16) {
        self.status.abort_latched = true;
        self.status.mode = FlightMode::Abort;
        self.status.alarms |= alarm | ALARM_ABORT;
    }

    fn safe_output(&mut self, sequence: u32) -> SpatialFlightOutput {
        let navigation = self.navigation.state();
        let command = SpatialActuatorCommand {
            sequence,
            gimbal_q16: [0; 2],
            rcs_q15: [0; 3],
            engine_action: EngineAction::Cutoff,
            separate: false,
            abort_safeing: true,
        };
        self.previous_command = command;
        self.make_output(sequence, navigation, command)
    }

    fn make_output(
        &mut self,
        sequence: u32,
        navigation: SpatialNavigationState,
        command: SpatialActuatorCommand,
    ) -> SpatialFlightOutput {
        self.status.checksum = hash_flight(self.status.checksum, self.status, command);
        SpatialFlightOutput {
            sequence,
            navigation,
            mode: self.status.mode,
            alarms: self.status.alarms,
            command,
            flight_checksum: self.status.checksum,
        }
    }
}

impl Default for SpatialFlightComputer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn attitude_command(
    sequence: u32,
    navigation: SpatialNavigationState,
    target: SpatialGuidanceTarget,
) -> SpatialActuatorCommand {
    let mut error = quaternion_error(target.attitude_q30, navigation.attitude_q30);
    if error[0] < 0 {
        let mut component = 0;
        while component < 4 {
            error[component] = error[component].saturating_neg();
            component += 1;
        }
    }
    let roll_rate_error = target.angular_rate_q24[0].saturating_sub(navigation.angular_rate_q24[0]);
    let pitch_rate_error =
        target.angular_rate_q24[1].saturating_sub(navigation.angular_rate_q24[1]);
    let yaw_rate_error = target.angular_rate_q24[2].saturating_sub(navigation.angular_rate_q24[2]);
    // Positive pitch/yaw gimbal produces negative body Y/Z torque in KSA-5A.
    let pitch_raw = -(error[2] >> 13) - (pitch_rate_error >> 9);
    let yaw_raw = -(error[3] >> 13) - (yaw_rate_error >> 9);
    let pitch =
        if pitch_raw.abs() <= 1 { 0 } else { pitch_raw }.clamp(-GIMBAL_LIMIT_Q16, GIMBAL_LIMIT_Q16);
    let yaw =
        if yaw_raw.abs() <= 1 { 0 } else { yaw_raw }.clamp(-GIMBAL_LIMIT_Q16, GIMBAL_LIMIT_Q16);
    let roll_rcs = ((error[1] >> 14) + (roll_rate_error >> 9)).clamp(-RCS_LIMIT_Q15, RCS_LIMIT_Q15);
    let pitch_rcs =
        ((error[2] >> 14) + (pitch_rate_error >> 9)).clamp(-RCS_LIMIT_Q15, RCS_LIMIT_Q15);
    let yaw_rcs = ((error[3] >> 14) + (yaw_rate_error >> 9)).clamp(-RCS_LIMIT_Q15, RCS_LIMIT_Q15);
    SpatialActuatorCommand {
        sequence,
        gimbal_q16: [pitch, yaw],
        rcs_q15: [roll_rcs, pitch_rcs, yaw_rcs],
        engine_action: EngineAction::Hold,
        separate: false,
        abort_safeing: false,
    }
}

fn quaternion_error(desired: [i32; 4], current: [i32; 4]) -> [i32; 4] {
    let conjugate = [current[0], -current[1], -current[2], -current[3]];
    [
        product(desired[0], conjugate[0])
            - product(desired[1], conjugate[1])
            - product(desired[2], conjugate[2])
            - product(desired[3], conjugate[3]),
        product(desired[0], conjugate[1])
            + product(desired[1], conjugate[0])
            + product(desired[2], conjugate[3])
            - product(desired[3], conjugate[2]),
        product(desired[0], conjugate[2]) - product(desired[1], conjugate[3])
            + product(desired[2], conjugate[0])
            + product(desired[3], conjugate[1]),
        product(desired[0], conjugate[3]) + product(desired[1], conjugate[2])
            - product(desired[2], conjugate[1])
            + product(desired[3], conjugate[0]),
    ]
}
fn product(left: i32, right: i32) -> i32 {
    ((left as i64 * right as i64) >> 30) as i32
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
fn hash_flight(mut hash: u32, status: SpatialFlightStatus, command: SpatialActuatorCommand) -> u32 {
    hash = hash_word(hash, status.mode as u32);
    hash = hash_word(hash, status.alarms as u32);
    hash = hash_word(hash, command.sequence);
    hash = hash_word(hash, command.gimbal_q16[0] as u32);
    hash = hash_word(hash, command.gimbal_q16[1] as u32);
    hash = hash_word(hash, command.rcs_q15[0] as u32);
    hash = hash_word(hash, command.rcs_q15[1] as u32);
    hash = hash_word(hash, command.rcs_q15[2] as u32);
    hash = hash_word(hash, command.engine_action as u32);
    hash_word(
        hash,
        command.separate as u32 | ((command.abort_safeing as u32) << 1),
    )
}

#[allow(dead_code)]
fn _navigation_error_is_part_of_contract(error: SpatialNavigationError) -> SpatialNavigationError {
    error
}
