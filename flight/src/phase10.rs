//! Truth-blind global-frame flight software for Phase 10.

use ksa64_interface::phase10::{
    GlobalAidFrameCell, GlobalCommandCell, GlobalFastSensorCell, GlobalFrameId, GlobalStatusCell,
    GlobalTransitionCell, GLOBAL_AID_ATTITUDE, GLOBAL_AID_BAROMETER, GLOBAL_AID_CONTINUITY,
    GLOBAL_AID_DEPLOYMENT_FEEDBACK, GLOBAL_AID_FRAME_SERVICE, GLOBAL_AID_GNSS,
    GLOBAL_COMMAND_DROGUE, GLOBAL_COMMAND_HOLD, GLOBAL_COMMAND_MAIN, GLOBAL_COMMAND_SAFE,
    GLOBAL_FAST_ATTITUDE, GLOBAL_FAST_DELTA_ANGLE, GLOBAL_FAST_DELTA_V,
};

pub const GLOBAL_FLIGHT_CONTRACT_ID: u32 = 0x1053_0001;
pub const GLOBAL_ALARM_FAST_SENSOR: u16 = 1;
pub const GLOBAL_ALARM_AID: u16 = 2;
pub const GLOBAL_ALARM_FRAME: u16 = 4;
pub const GLOBAL_ALARM_LINK: u16 = 8;
pub const GLOBAL_ALARM_SAFE: u16 = 16;
pub const GLOBAL_ALARM_NAVIGATION: u16 = 32;
pub const GLOBAL_ALARM_RECOVERY: u16 = 64;
pub const GLOBAL_ALARM_DEADLINE: u16 = 128;

const Q30_ONE: i32 = 1 << 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlobalFlightMode {
    Boot = 0,
    Prelaunch = 1,
    Ascent = 2,
    Coast = 3,
    Entry = 4,
    Recovery = 5,
    Complete = 6,
    Safe = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalFlightConfig {
    pub session: u16,
    pub initial_frame: GlobalFrameId,
    pub initial_position_q12: [i32; 3],
    pub initial_attitude_q30: [i32; 4],
    pub launch_target_q30: [i32; 4],
    pub powered_target_q30: [i32; 4],
    pub entry_target_q30: [i32; 4],
    pub pitch_program_end_q16: u32,
    pub proportional_gain_q15: [i16; 3],
    pub derivative_gain_q15: [i16; 3],
    pub torque_limit_q12: [i32; 3],
    pub gimbal_limit_q15: i16,
    pub minimum_arming_time_q16: u32,
    pub drogue_backup_time_q16: u32,
    pub main_backup_time_q16: u32,
    pub main_altitude_q12_km: i32,
    pub minimum_deployment_separation_q16: u32,
}

impl GlobalFlightConfig {
    pub fn is_valid(self) -> bool {
        self.session != 0
            && self.initial_attitude_q30 != [0; 4]
            && self.pitch_program_end_q16 > 0
            && self.proportional_gain_q15.iter().all(|gain| *gain >= 0)
            && self.derivative_gain_q15.iter().all(|gain| *gain >= 0)
            && self.torque_limit_q12.iter().all(|limit| *limit > 0)
            && self.gimbal_limit_q15 >= 0
            && self.drogue_backup_time_q16 > self.minimum_arming_time_q16
            && self.main_backup_time_q16 > self.drogue_backup_time_q16
            && self.main_altitude_q12_km > 0
            && self.minimum_deployment_separation_q16 > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalNavigation {
    pub frame: GlobalFrameId,
    pub position_q12: [i32; 3],
    pub velocity_q24: [i32; 3],
    pub attitude_q30: [i32; 4],
    pub covariance_proxy_q16: [i32; 3],
    pub checksum: u32,
}

impl GlobalNavigation {
    fn new(config: GlobalFlightConfig) -> Self {
        Self {
            frame: config.initial_frame,
            position_q12: config.initial_position_q12,
            velocity_q24: [0; 3],
            attitude_q30: normalize_quaternion(config.initial_attitude_q30),
            covariance_proxy_q16: [1 << 12; 3],
            checksum: 0x811c_9dc5,
        }
    }

    fn transition(&mut self, cell: &GlobalTransitionCell) -> bool {
        if cell.from != self.frame {
            return false;
        }
        let old_position = self.position_q12;
        let rotated_position = rotate_vector(cell.rotation_q30, old_position);
        self.position_q12 = add_vector(rotated_position, cell.translation_q12);
        self.velocity_q24 = match (cell.from, cell.to) {
            (GlobalFrameId::EarthFixedEcefV1, GlobalFrameId::EarthInertialEciV1) => add_vector(
                rotate_vector(cell.rotation_q30, self.velocity_q24),
                cross_rate_position(cell.omega_q24, self.position_q12),
            ),
            (GlobalFrameId::EarthInertialEciV1, GlobalFrameId::EarthFixedEcefV1) => {
                let without_sweep = subtract_vector(
                    self.velocity_q24,
                    cross_rate_position(cell.omega_q24, old_position),
                );
                rotate_vector(cell.rotation_q30, without_sweep)
            }
            _ => add_vector(
                rotate_vector(cell.rotation_q30, self.velocity_q24),
                cell.velocity_bias_q24,
            ),
        };
        self.attitude_q30 =
            normalize_quaternion(quaternion_product(cell.rotation_q30, self.attitude_q30));
        self.frame = cell.to;
        self.checksum = hash_navigation(
            self.checksum,
            cell.effective_epoch,
            self.frame,
            &self.position_q12,
            &self.velocity_q24,
        );
        true
    }

    fn inertial(&mut self, cell: &GlobalFastSensorCell) {
        for axis in 0..3 {
            self.velocity_q24[axis] =
                self.velocity_q24[axis].saturating_add(i32::from(cell.delta_velocity_q24[axis]));
            self.position_q12[axis] =
                self.position_q12[axis].saturating_add(self.velocity_q24[axis] >> 17);
            self.covariance_proxy_q16[axis] = self.covariance_proxy_q16[axis].saturating_add(8);
        }
        self.attitude_q30 = integrate_small_angle(
            self.attitude_q30,
            [
                i32::from(cell.delta_angle_q24[0]),
                i32::from(cell.delta_angle_q24[1]),
                i32::from(cell.delta_angle_q24[2]),
            ],
        );
        self.checksum = hash_navigation(
            self.checksum,
            cell.measurement_epoch,
            self.frame,
            &self.position_q12,
            &self.velocity_q24,
        );
    }

    fn aid(
        &mut self,
        cell: &GlobalAidFrameCell,
        ecef_to_active_rotation: [i32; 4],
        ecef_to_active_translation: [i32; 3],
        ecef_to_active_velocity_bias: [i32; 3],
    ) {
        if cell.validity & GLOBAL_AID_GNSS != 0 {
            let position = add_vector(
                rotate_vector(ecef_to_active_rotation, cell.gnss_position_q12_km),
                ecef_to_active_translation,
            );
            let velocity = add_vector(
                rotate_vector(ecef_to_active_rotation, cell.gnss_velocity_q24_km_s),
                ecef_to_active_velocity_bias,
            );
            for axis in 0..3 {
                self.position_q12[axis] = blend(self.position_q12[axis], position[axis], 3);
                self.velocity_q24[axis] = blend(self.velocity_q24[axis], velocity[axis], 3);
                self.covariance_proxy_q16[axis] = (self.covariance_proxy_q16[axis] * 3 / 4).max(1);
            }
        }
        if cell.validity & GLOBAL_AID_BAROMETER != 0 && self.frame == GlobalFrameId::LocalEnuV1 {
            self.position_q12[2] = blend(self.position_q12[2], cell.barometer_q12_km, 2);
        }
        if cell.validity & GLOBAL_AID_ATTITUDE != 0 {
            self.attitude_q30 = normalize_quaternion(cell.attitude_q30);
        }
        self.checksum = hash_navigation(
            self.checksum,
            cell.measurement_epoch,
            self.frame,
            &self.position_q12,
            &self.velocity_q24,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalFlightEvidence {
    pub command: GlobalCommandCell,
    pub status: Option<GlobalStatusCell>,
    pub navigation: GlobalNavigation,
    pub mode: GlobalFlightMode,
    pub safe: bool,
    pub armed: bool,
    pub drogue_latched: bool,
    pub main_latched: bool,
    pub alarms: u16,
    pub sensor_checksum: u32,
    pub flight_checksum: u32,
    pub deadline_misses: u16,
}

pub struct GlobalFlightComputer {
    config: GlobalFlightConfig,
    epoch: u16,
    navigation: GlobalNavigation,
    mode: GlobalFlightMode,
    alarms: u16,
    missing_fast: u8,
    safe: bool,
    seen_powered: bool,
    powered: bool,
    armed: bool,
    descending_count: u8,
    drogue_latched: bool,
    main_latched: bool,
    drogue_time_q16: u32,
    ecef_to_active_rotation: [i32; 4],
    ecef_to_active_translation: [i32; 3],
    ecef_to_active_velocity_bias: [i32; 3],
    transition_count: u8,
    last_time_q16: u32,
    last_fast: Option<GlobalFastSensorCell>,
    last_aid: Option<GlobalAidFrameCell>,
    last_command: GlobalCommandCell,
    last_status: Option<GlobalStatusCell>,
    sensor_checksum: u32,
    command_checksum: u32,
    flight_checksum: u32,
    deadline_misses: u16,
}

impl GlobalFlightComputer {
    pub fn new(config: GlobalFlightConfig) -> Option<Self> {
        if !config.is_valid() {
            return None;
        }
        let navigation = GlobalNavigation::new(config);
        Some(Self {
            config,
            epoch: 0,
            navigation,
            mode: GlobalFlightMode::Boot,
            alarms: 0,
            missing_fast: 0,
            safe: false,
            seen_powered: false,
            powered: false,
            armed: false,
            descending_count: 0,
            drogue_latched: false,
            main_latched: false,
            drogue_time_q16: 0,
            ecef_to_active_rotation: [Q30_ONE, 0, 0, 0],
            ecef_to_active_translation: [0; 3],
            ecef_to_active_velocity_bias: [0; 3],
            transition_count: 0,
            last_time_q16: 0,
            last_fast: None,
            last_aid: None,
            last_command: GlobalCommandCell {
                session: config.session,
                source_epoch: 0,
                effective_epoch: 1,
                frame: config.initial_frame,
                flags: 0,
                discrete: 0,
                gimbal_q15: [0; 2],
                rcs_pulse_quanta: [0; 12],
                torque_demand_q12: [0; 3],
                status: 0,
                command_checksum: 0x811c_9dc5,
            },
            last_status: None,
            sensor_checksum: 0x811c_9dc5,
            command_checksum: 0x811c_9dc5,
            flight_checksum: 0x811c_9dc5,
            deadline_misses: 0,
        })
    }

    pub const fn navigation(&self) -> GlobalNavigation {
        self.navigation
    }

    pub const fn is_safe(&self) -> bool {
        self.safe
    }

    pub fn record_deadline_miss(&mut self) {
        self.deadline_misses = self.deadline_misses.saturating_add(1);
        self.alarms |= GLOBAL_ALARM_DEADLINE | GLOBAL_ALARM_SAFE;
        self.safe = true;
    }

    pub fn tick(
        &mut self,
        fast: Option<GlobalFastSensorCell>,
        aid: Option<GlobalAidFrameCell>,
        transition: Option<GlobalTransitionCell>,
    ) -> GlobalFlightEvidence {
        self.apply_transition(transition);
        let valid_fast = fast.filter(|cell| {
            cell.session == self.config.session
                && cell.measurement_epoch == self.epoch
                && cell.production_epoch == self.epoch
                && cell.frame == self.navigation.frame
                && cell.validity
                    & (GLOBAL_FAST_DELTA_V | GLOBAL_FAST_DELTA_ANGLE | GLOBAL_FAST_ATTITUDE)
                    == (GLOBAL_FAST_DELTA_V | GLOBAL_FAST_DELTA_ANGLE | GLOBAL_FAST_ATTITUDE)
        });
        if let Some(cell) = valid_fast {
            self.missing_fast = 0;
            self.last_time_q16 = cell.mission_time_q16;
            self.sensor_checksum = hash_fast(self.sensor_checksum, &cell);
            self.navigation.inertial(&cell);
            self.last_fast = Some(cell);
            self.powered = cell.vehicle_status & 2 != 0;
            self.seen_powered |= self.powered;
        } else {
            self.missing_fast = self.missing_fast.saturating_add(1);
            self.alarms |= GLOBAL_ALARM_FAST_SENSOR | GLOBAL_ALARM_LINK;
            if self.missing_fast >= 3 {
                self.safe = true;
                self.alarms |= GLOBAL_ALARM_SAFE;
            }
        }

        let valid_aid = aid.filter(|cell| {
            cell.session == self.config.session
                && cell.measurement_epoch <= cell.production_epoch
                && self.epoch.wrapping_sub(cell.production_epoch) <= 32
                && cell.frame == self.navigation.frame
        });
        if let Some(cell) = valid_aid {
            if cell.validity & GLOBAL_AID_FRAME_SERVICE != 0
                && self.navigation.frame == GlobalFrameId::EarthInertialEciV1
            {
                self.ecef_to_active_rotation = cell.frame_rotation_q30;
            }
            self.navigation.aid(
                &cell,
                self.ecef_to_active_rotation,
                self.ecef_to_active_translation,
                self.ecef_to_active_velocity_bias,
            );
            self.last_aid = Some(cell);
        } else if aid.is_some() {
            self.alarms |= GLOBAL_ALARM_AID;
        }

        self.update_mode_and_recovery(valid_fast.as_ref(), valid_aid.as_ref());
        let mut discrete = self.recovery_command(valid_aid.as_ref());
        let target = self.guidance_target();
        let mut torque = [0; 3];
        let mut normalized_demand = [0i16; 3];
        if let Some(cell) = valid_fast {
            let error_quaternion = attitude_error(self.navigation.attitude_q30, target);
            for axis in 0..3 {
                let error = error_quaternion[axis + 1] >> 15;
                let rate = i32::from(cell.angular_rate_q15[axis]);
                let proportional =
                    (error * i32::from(self.config.proportional_gain_q15[axis])) >> 15;
                let derivative = (-rate * i32::from(self.config.derivative_gain_q15[axis])) >> 15;
                normalized_demand[axis] = clamp_i16(proportional.saturating_add(derivative));
                torque[axis] = ((i64::from(normalized_demand[axis])
                    * i64::from(self.config.torque_limit_q12[axis]))
                    >> 15) as i32;
            }
        }

        let held = self.missing_fast != 0 && self.missing_fast <= 2;
        if held {
            torque = self.last_command.torque_demand_q12;
            normalized_demand[1] = self.last_command.gimbal_q15[0];
            normalized_demand[2] = self.last_command.gimbal_q15[1];
            discrete = 0;
        }
        let mut flags = if held { GLOBAL_COMMAND_HOLD } else { 0 };
        if self.safe {
            torque = [0; 3];
            normalized_demand = [0; 3];
            discrete = GLOBAL_COMMAND_SAFE;
        }
        let gimbal = if self.powered && !self.safe {
            [
                normalized_demand[1]
                    .clamp(-self.config.gimbal_limit_q15, self.config.gimbal_limit_q15),
                normalized_demand[2]
                    .clamp(-self.config.gimbal_limit_q15, self.config.gimbal_limit_q15),
            ]
        } else {
            [0; 2]
        };
        let mut pulses = [0; 12];
        if !self.powered
            && !self.safe
            && matches!(self.mode, GlobalFlightMode::Coast | GlobalFlightMode::Entry)
        {
            for (axis, demand) in torque.iter().copied().enumerate() {
                let demand = if axis == 0 && descending_global(&self.navigation) {
                    0
                } else {
                    demand
                };
                let pair_authority_q12 = if axis == 0 { 1_638u32 } else { 29_900u32 };
                let magnitude = demand.unsigned_abs();
                let quantum = if magnitude == 0 {
                    0
                } else {
                    (((magnitude * 8 + pair_authority_q12 / 2) / pair_authority_q12).clamp(1, 8))
                        as u8
                };
                if quantum != 0 {
                    let channel = axis * 4 + usize::from(demand < 0) * 2;
                    pulses[channel] = quantum;
                    pulses[channel + 1] = quantum;
                }
            }
        }
        if self
            .last_fast
            .map(|cell| cell.rcs_propellant_q21 <= 0)
            .unwrap_or(false)
        {
            pulses = [0; 12];
            flags |= ksa64_interface::phase10::GLOBAL_COMMAND_RCS_RESERVED;
        }

        let mut command = GlobalCommandCell {
            session: self.config.session,
            source_epoch: self.epoch,
            effective_epoch: self.epoch.wrapping_add(1),
            frame: self.navigation.frame,
            flags,
            discrete,
            gimbal_q15: gimbal,
            rcs_pulse_quanta: pulses,
            torque_demand_q12: torque,
            status: self.alarms,
            command_checksum: 0,
        };
        self.command_checksum = hash_command(self.command_checksum, &command);
        command.command_checksum = self.command_checksum;
        self.flight_checksum = hash_flight(
            self.flight_checksum,
            self.epoch,
            self.navigation.checksum,
            self.command_checksum,
            self.alarms,
        );
        self.last_command = command;
        self.last_status = if self.epoch & 3 == 0 {
            Some(GlobalStatusCell {
                session: self.config.session,
                source_epoch: self.epoch,
                production_epoch: self.epoch,
                frame: self.navigation.frame,
                mode: self.mode as u8,
                flags: u16::from(self.safe),
                alarms: self.alarms,
                navigation_position_q12: self.navigation.position_q12,
                navigation_velocity_q24: self.navigation.velocity_q24,
                navigation_attitude_q30: self.navigation.attitude_q30,
                covariance_proxy_q16: self.navigation.covariance_proxy_q16,
                sensor_checksum: self.sensor_checksum,
                navigation_checksum: self.navigation.checksum,
                command_checksum: self.command_checksum,
                flight_checksum: self.flight_checksum,
                deadline_misses: self.deadline_misses,
                transition_count: self.transition_count,
            })
        } else {
            None
        };
        self.epoch = self.epoch.wrapping_add(1);
        GlobalFlightEvidence {
            command,
            status: self.last_status,
            navigation: self.navigation,
            mode: self.mode,
            safe: self.safe,
            armed: self.armed,
            drogue_latched: self.drogue_latched,
            main_latched: self.main_latched,
            alarms: self.alarms,
            sensor_checksum: self.sensor_checksum,
            flight_checksum: self.flight_checksum,
            deadline_misses: self.deadline_misses,
        }
    }

    fn apply_transition(&mut self, transition: Option<GlobalTransitionCell>) {
        let Some(cell) = transition else {
            return;
        };
        let valid = cell.session == self.config.session
            && cell.source_epoch == self.epoch
            && cell.effective_epoch == self.epoch
            && cell.from == self.navigation.frame;
        if !valid || !self.navigation.transition(&cell) {
            self.alarms |= GLOBAL_ALARM_FRAME | GLOBAL_ALARM_SAFE;
            self.safe = true;
            return;
        }
        self.transition_count = self.transition_count.saturating_add(1);
        self.config.launch_target_q30 = normalize_quaternion(quaternion_product(
            cell.rotation_q30,
            self.config.launch_target_q30,
        ));
        self.config.powered_target_q30 = normalize_quaternion(quaternion_product(
            cell.rotation_q30,
            self.config.powered_target_q30,
        ));
        self.config.entry_target_q30 = normalize_quaternion(quaternion_product(
            cell.rotation_q30,
            self.config.entry_target_q30,
        ));
        match cell.to {
            GlobalFrameId::EarthFixedEcefV1 => {
                self.ecef_to_active_rotation = [Q30_ONE, 0, 0, 0];
                self.ecef_to_active_translation = [0; 3];
                self.ecef_to_active_velocity_bias = [0; 3];
            }
            GlobalFrameId::EarthInertialEciV1 | GlobalFrameId::LocalEnuV1 => {
                self.ecef_to_active_rotation = cell.rotation_q30;
                self.ecef_to_active_translation = cell.translation_q12;
                self.ecef_to_active_velocity_bias = cell.velocity_bias_q24;
            }
        }
    }

    fn update_mode_and_recovery(
        &mut self,
        fast: Option<&GlobalFastSensorCell>,
        aid: Option<&GlobalAidFrameCell>,
    ) {
        self.mode = if self.safe {
            GlobalFlightMode::Safe
        } else {
            match self.navigation.frame {
                GlobalFrameId::LocalEnuV1 if self.transition_count == 0 => {
                    if self.powered {
                        GlobalFlightMode::Ascent
                    } else {
                        GlobalFlightMode::Prelaunch
                    }
                }
                GlobalFrameId::EarthFixedEcefV1 => {
                    if self.seen_powered && !self.powered {
                        GlobalFlightMode::Entry
                    } else {
                        GlobalFlightMode::Ascent
                    }
                }
                GlobalFrameId::EarthInertialEciV1 => GlobalFlightMode::Coast,
                GlobalFrameId::LocalEnuV1 => GlobalFlightMode::Recovery,
            }
        };
        self.armed |= self.seen_powered
            && !self.powered
            && self.last_time_q16 >= self.config.minimum_arming_time_q16;
        if self.mode == GlobalFlightMode::Recovery && self.navigation.velocity_q24[2] < 0 {
            self.descending_count = self.descending_count.saturating_add(1);
        } else {
            self.descending_count = 0;
        }
        if aid
            .map(|value| value.events & (1 << 9) != 0)
            .unwrap_or(false)
        {
            self.mode = GlobalFlightMode::Complete;
        }
        if fast.is_none() && self.missing_fast >= 3 {
            self.mode = GlobalFlightMode::Safe;
        }
    }

    fn recovery_command(&mut self, aid: Option<&GlobalAidFrameCell>) -> u8 {
        if self.safe || !self.armed {
            return 0;
        }
        let continuity = aid
            .filter(|cell| cell.validity & GLOBAL_AID_CONTINUITY != 0)
            .map(|cell| cell.continuity & 1 != 0)
            .unwrap_or(false);
        let feedback = aid
            .filter(|cell| cell.validity & GLOBAL_AID_DEPLOYMENT_FEEDBACK != 0)
            .map(|cell| cell.deployment_feedback)
            .unwrap_or(0);
        let mach_subsonic = self
            .last_fast
            .map(|cell| cell.mach_q12 < 3_277)
            .unwrap_or(false);
        let drogue_primary =
            self.mode == GlobalFlightMode::Recovery && self.descending_count >= 2 && mach_subsonic;
        let drogue_backup = self.last_time_q16 >= self.config.drogue_backup_time_q16;
        let mut command = 0;
        if !self.drogue_latched && (drogue_primary || drogue_backup) {
            if continuity {
                self.drogue_latched = true;
                self.drogue_time_q16 = self.last_time_q16;
                command |= GLOBAL_COMMAND_DROGUE;
            } else {
                self.alarms |= GLOBAL_ALARM_RECOVERY;
            }
        }
        let separation = self.last_time_q16.saturating_sub(self.drogue_time_q16)
            >= self.config.minimum_deployment_separation_q16;
        let main_primary = self.drogue_latched
            && feedback & 1 != 0
            && separation
            && self.mode == GlobalFlightMode::Recovery
            && self.navigation.velocity_q24[2] < 0
            && self.navigation.position_q12[2] <= self.config.main_altitude_q12_km;
        let main_backup = self.drogue_latched
            && separation
            && self.last_time_q16 >= self.config.main_backup_time_q16;
        if !self.main_latched && (main_primary || main_backup) {
            if continuity {
                self.main_latched = true;
                command |= GLOBAL_COMMAND_MAIN;
            } else {
                self.alarms |= GLOBAL_ALARM_RECOVERY;
            }
        }
        command
    }

    fn guidance_target(&self) -> [i32; 4] {
        let quaternion = match self.mode {
            GlobalFlightMode::Ascent => {
                if self.last_time_q16 >= self.config.pitch_program_end_q16 {
                    self.config.powered_target_q30
                } else {
                    let fraction_q16 = ((u64::from(self.last_time_q16) << 16)
                        / u64::from(self.config.pitch_program_end_q16))
                        as i32;
                    let mut target = [0; 4];
                    for (component, value) in target.iter_mut().enumerate() {
                        let low = self.config.launch_target_q30[component];
                        let high = self.config.powered_target_q30[component];
                        *value = low.saturating_add(
                            (((i64::from(high) - i64::from(low)) * i64::from(fraction_q16)) >> 16)
                                as i32,
                        );
                    }
                    normalize_quaternion(target)
                }
            }
            GlobalFlightMode::Coast if descending_global(&self.navigation) => {
                body_x_attitude_from_velocity(self.navigation.velocity_q24)
            }
            GlobalFlightMode::Entry => body_x_attitude_from_velocity(self.navigation.velocity_q24),
            _ => self.config.powered_target_q30,
        };
        quaternion
    }
}

fn blend(current: i32, measurement: i32, shift: u8) -> i32 {
    current.saturating_add(measurement.saturating_sub(current) >> shift)
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn rounded_shift(value: i64, shift: u8) -> i32 {
    let magnitude = value.unsigned_abs();
    let rounded = (magnitude + (1u64 << (shift - 1))) >> shift;
    let signed = if value < 0 {
        -(rounded as i64)
    } else {
        rounded as i64
    };
    signed.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn add_vector(left: [i32; 3], right: [i32; 3]) -> [i32; 3] {
    [
        left[0].saturating_add(right[0]),
        left[1].saturating_add(right[1]),
        left[2].saturating_add(right[2]),
    ]
}

fn subtract_vector(left: [i32; 3], right: [i32; 3]) -> [i32; 3] {
    [
        left[0].saturating_sub(right[0]),
        left[1].saturating_sub(right[1]),
        left[2].saturating_sub(right[2]),
    ]
}

fn cross_rate_position(rate_q24: [i32; 3], position_q12: [i32; 3]) -> [i32; 3] {
    [
        rounded_shift(
            i64::from(rate_q24[1]) * i64::from(position_q12[2])
                - i64::from(rate_q24[2]) * i64::from(position_q12[1]),
            12,
        ),
        rounded_shift(
            i64::from(rate_q24[2]) * i64::from(position_q12[0])
                - i64::from(rate_q24[0]) * i64::from(position_q12[2]),
            12,
        ),
        rounded_shift(
            i64::from(rate_q24[0]) * i64::from(position_q12[1])
                - i64::from(rate_q24[1]) * i64::from(position_q12[0]),
            12,
        ),
    ]
}

fn rotate_vector(rotation: [i32; 4], vector: [i32; 3]) -> [i32; 3] {
    let [w, x, y, z] = rotation.map(i64::from);
    let matrix = [
        [
            (1i64 << 30) - ((2 * (y * y + z * z)) >> 30),
            (2 * (x * y - w * z)) >> 30,
            (2 * (x * z + w * y)) >> 30,
        ],
        [
            (2 * (x * y + w * z)) >> 30,
            (1i64 << 30) - ((2 * (x * x + z * z)) >> 30),
            (2 * (y * z - w * x)) >> 30,
        ],
        [
            (2 * (x * z - w * y)) >> 30,
            (2 * (y * z + w * x)) >> 30,
            (1i64 << 30) - ((2 * (x * x + y * y)) >> 30),
        ],
    ];
    let mut output = [0; 3];
    for row in 0..3 {
        output[row] = rounded_shift(
            matrix[row][0] * i64::from(vector[0])
                + matrix[row][1] * i64::from(vector[1])
                + matrix[row][2] * i64::from(vector[2]),
            30,
        );
    }
    output
}

fn quaternion_product(left: [i32; 4], right: [i32; 4]) -> [i32; 4] {
    let [aw, ax, ay, az] = left.map(i64::from);
    let [bw, bx, by, bz] = right.map(i64::from);
    [
        rounded_shift(aw * bw - ax * bx - ay * by - az * bz, 30),
        rounded_shift(aw * bx + ax * bw + ay * bz - az * by, 30),
        rounded_shift(aw * by - ax * bz + ay * bw + az * bx, 30),
        rounded_shift(aw * bz + ax * by - ay * bx + az * bw, 30),
    ]
}

fn descending_global(navigation: &GlobalNavigation) -> bool {
    let radial = navigation
        .position_q12
        .iter()
        .zip(navigation.velocity_q24)
        .map(|(position, velocity)| i64::from(*position) * i64::from(velocity))
        .sum::<i64>();
    radial < 0
}
fn attitude_error(current: [i32; 4], target: [i32; 4]) -> [i32; 4] {
    let conjugate = [current[0], -current[1], -current[2], -current[3]];
    let mut error = normalize_quaternion(quaternion_product(conjugate, target));
    if error[0] < 0 {
        for component in &mut error {
            *component = component.saturating_neg();
        }
    }
    error
}
fn body_x_attitude_from_velocity(velocity: [i32; 3]) -> [i32; 4] {
    let sum = velocity
        .iter()
        .map(|component| i64::from(*component) * i64::from(*component))
        .sum::<i64>();
    if sum <= 0 {
        return [Q30_ONE, 0, 0, 0];
    }
    let magnitude = integer_sqrt(sum as u64).min(i32::MAX as u64) as i32;
    normalize_quaternion([
        magnitude.saturating_add(velocity[0]),
        0,
        velocity[2].saturating_neg(),
        velocity[1],
    ])
}
fn normalize_quaternion(value: [i32; 4]) -> [i32; 4] {
    let sum = value
        .iter()
        .map(|component| i64::from(*component) * i64::from(*component))
        .sum::<i64>();
    if sum <= 0 {
        return [Q30_ONE, 0, 0, 0];
    }
    let magnitude = integer_sqrt(sum as u64).max(1);
    value.map(|component| {
        let numerator = i64::from(component) * i64::from(Q30_ONE);
        (numerator / magnitude as i64).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    })
}

fn integrate_small_angle(attitude: [i32; 4], delta_angle_q24: [i32; 3]) -> [i32; 4] {
    let half = [
        Q30_ONE,
        delta_angle_q24[0].saturating_mul(32),
        delta_angle_q24[1].saturating_mul(32),
        delta_angle_q24[2].saturating_mul(32),
    ];
    normalize_quaternion(quaternion_product(attitude, half))
}

fn integer_sqrt(value: u64) -> u64 {
    if value == 0 {
        return 0;
    }
    let mut result = 0u64;
    let mut bit = 1u64 << 62;
    while bit > value {
        bit >>= 2;
    }
    let mut remainder = value;
    while bit != 0 {
        if remainder >= result + bit {
            remainder -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result
}

fn hash_word(mut checksum: u32, value: u32) -> u32 {
    for byte in value.to_le_bytes() {
        checksum ^= u32::from(byte);
        checksum = checksum.wrapping_mul(0x0100_0193);
    }
    checksum
}

fn hash_navigation(
    mut checksum: u32,
    epoch: u16,
    frame: GlobalFrameId,
    position: &[i32; 3],
    velocity: &[i32; 3],
) -> u32 {
    checksum = hash_word(checksum, u32::from(epoch));
    checksum = hash_word(checksum, frame as u32);
    for value in position.iter().chain(velocity.iter()) {
        checksum = hash_word(checksum, *value as u32);
    }
    checksum
}

fn hash_fast(mut checksum: u32, cell: &GlobalFastSensorCell) -> u32 {
    checksum = hash_word(checksum, u32::from(cell.measurement_epoch));
    checksum = hash_word(checksum, cell.mission_time_q16);
    checksum = hash_word(checksum, cell.frame as u32);
    for value in cell
        .delta_velocity_q24
        .iter()
        .chain(cell.delta_angle_q24.iter())
    {
        checksum = hash_word(checksum, *value as u16 as u32);
    }
    checksum
}

fn hash_command(mut checksum: u32, command: &GlobalCommandCell) -> u32 {
    checksum = hash_word(checksum, u32::from(command.source_epoch));
    checksum = hash_word(checksum, command.frame as u32);
    checksum = hash_word(checksum, u32::from(command.discrete));
    for value in command.torque_demand_q12 {
        checksum = hash_word(checksum, value as u32);
    }
    for quantum in command.rcs_pulse_quanta {
        checksum = hash_word(checksum, u32::from(quantum));
    }
    checksum
}

fn hash_flight(mut checksum: u32, epoch: u16, navigation: u32, command: u32, alarms: u16) -> u32 {
    checksum = hash_word(checksum, u32::from(epoch));
    checksum = hash_word(checksum, navigation);
    checksum = hash_word(checksum, command);
    hash_word(checksum, u32::from(alarms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_interface::phase10::{GLOBAL_AID_VALID_MASK, GLOBAL_FAST_VALID_MASK};

    fn config() -> GlobalFlightConfig {
        GlobalFlightConfig {
            session: 10,
            initial_frame: GlobalFrameId::LocalEnuV1,
            initial_position_q12: [0; 3],
            initial_attitude_q30: [Q30_ONE, 0, 0, 0],
            launch_target_q30: [759_250_125, 0, 759_250_125, 0],
            powered_target_q30: [900_258_487, 0, 584_993_064, 0],
            entry_target_q30: [Q30_ONE, 0, 0, 0],
            pitch_program_end_q16: 60 << 16,
            proportional_gain_q15: [16_384; 3],
            derivative_gain_q15: [8_192; 3],
            torque_limit_q12: [1 << 12; 3],
            gimbal_limit_q15: 455,
            minimum_arming_time_q16: 60 << 16,
            drogue_backup_time_q16: 600 << 16,
            main_backup_time_q16: 900 << 16,
            main_altitude_q12_km: 1 << 12,
            minimum_deployment_separation_q16: 2 << 16,
        }
    }

    fn fast(epoch: u16, frame: GlobalFrameId, time: u32) -> GlobalFastSensorCell {
        GlobalFastSensorCell {
            session: 10,
            measurement_epoch: epoch,
            production_epoch: epoch,
            frame,
            validity: GLOBAL_FAST_VALID_MASK,
            mission_time_q16: time,
            delta_velocity_q24: [0; 3],
            delta_angle_q24: [0; 3],
            attitude_vector_q15: [0, 8_192, 0],
            angular_rate_q15: [0; 3],
            dynamic_pressure_q10: 0,
            mach_q12: 0,
            gimbal_applied_q15: [0; 2],
            rcs_propellant_q21: 5 << 21,
            actuator_feedback: 0,
            vehicle_status: 2,
            sensor_checksum: epoch,
        }
    }

    fn aid(epoch: u16, frame: GlobalFrameId, time: u32) -> GlobalAidFrameCell {
        GlobalAidFrameCell {
            session: 10,
            measurement_epoch: epoch,
            production_epoch: epoch,
            frame,
            validity: GLOBAL_AID_VALID_MASK,
            mission_time_q16: time,
            barometer_q12_km: 0,
            gnss_position_q12_km: [0; 3],
            gnss_velocity_q24_km_s: [0; 3],
            attitude_q30: [Q30_ONE, 0, 0, 0],
            frame_rotation_q30: [Q30_ONE, 0, 0, 0],
            frame_omega_q24: [0; 3],
            events: 0,
            continuity: 1,
            deployment_feedback: 0,
        }
    }

    #[test]
    fn transition_changes_estimate_without_resetting_it_to_truth() {
        let mut computer = GlobalFlightComputer::new(config()).unwrap();
        let first = computer.tick(
            Some(fast(0, GlobalFrameId::LocalEnuV1, 0)),
            Some(aid(0, GlobalFrameId::LocalEnuV1, 0)),
            None,
        );
        assert!(!first.safe);
        let transition = GlobalTransitionCell {
            session: 10,
            source_epoch: 1,
            effective_epoch: 1,
            from: GlobalFrameId::LocalEnuV1,
            to: GlobalFrameId::EarthFixedEcefV1,
            flags: 0,
            mission_time_q16: 2_048,
            transform_identity: 1,
            rotation_q30: [Q30_ONE, 0, 0, 0],
            omega_q24: [0; 3],
            pre_position_q12: [999; 3],
            post_position_q12: [888; 3],
            pre_velocity_q24: [777; 3],
            post_velocity_q24: [666; 3],
            pre_attitude_q30: [Q30_ONE, 0, 0, 0],
            post_attitude_q30: [Q30_ONE, 0, 0, 0],
            pre_rate_q24: [0; 3],
            post_rate_q24: [0; 3],
            translation_q12: [100, 200, 300],
            velocity_bias_q24: [10, 20, 30],
            transition_checksum: 1,
        };
        let evidence = computer.tick(
            Some(fast(1, GlobalFrameId::EarthFixedEcefV1, 2_048)),
            None,
            Some(transition),
        );
        assert_eq!(evidence.navigation.frame, GlobalFrameId::EarthFixedEcefV1);
        assert_eq!(evidence.navigation.position_q12, [100, 200, 300]);
        assert_ne!(
            evidence.navigation.position_q12,
            transition.post_position_q12
        );
        assert!(!evidence.safe);
    }

    #[test]
    fn two_missing_epochs_hold_and_third_safes_without_replaying_discrete() {
        let mut computer = GlobalFlightComputer::new(config()).unwrap();
        let first = computer.tick(Some(fast(0, GlobalFrameId::LocalEnuV1, 0)), None, None);
        let one = computer.tick(None, None, None);
        let two = computer.tick(None, None, None);
        let three = computer.tick(None, None, None);
        assert_eq!(one.command.flags & GLOBAL_COMMAND_HOLD, GLOBAL_COMMAND_HOLD);
        assert_eq!(
            two.command.torque_demand_q12,
            first.command.torque_demand_q12
        );
        assert_eq!(one.command.discrete, 0);
        assert_eq!(two.command.discrete, 0);
        assert!(three.safe);
        assert_eq!(three.command.discrete, GLOBAL_COMMAND_SAFE);
    }
}
