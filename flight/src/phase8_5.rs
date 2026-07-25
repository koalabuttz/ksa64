//! Truth-blind local-ENU avionics for Phase 8.5.

use ksa64_interface::phase8_5::{
    LocalAidCell, LocalCommandCell, LocalInertialCell, LocalStatusCell, LOCAL_AID_BAROMETER,
    LOCAL_AID_CONTINUITY, LOCAL_AID_DEPLOYMENT_FEEDBACK, LOCAL_AID_GPS, LOCAL_COMMAND_DROGUE,
    LOCAL_COMMAND_MAIN, LOCAL_COMMAND_SAFE,
};

pub const LOCAL_FLIGHT_CONTRACT_ID: u32 = 0x0853_0001;
pub const LOCAL_ALARM_INERTIAL: u16 = 1;
pub const LOCAL_ALARM_AID: u16 = 2;
pub const LOCAL_ALARM_LINK: u16 = 4;
pub const LOCAL_ALARM_SAFE: u16 = 8;
pub const LOCAL_ALARM_RECOVERY: u16 = 16;
pub const LOCAL_ALARM_DEADLINE: u16 = 32;
const GRAVITY_DELTA_V_Q19: i32 = 160_671;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalControlCapability {
    MonitorOnly,
    TwoAxisGimbal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalFlightConfig {
    pub session: u16,
    pub capability: LocalControlCapability,
    pub minimum_arming_time_q18: i32,
    pub minimum_arming_altitude_q13: i32,
    pub burnout_qualification_time_q18: i32,
    pub drogue_backup_time_q18: i32,
    pub main_backup_time_q18: i32,
    pub main_altitude_q13: i32,
    pub minimum_deployment_separation_q18: i32,
    pub proportional_gain_q15: i16,
    pub derivative_gain_q15: i16,
    pub gimbal_limit_q15: i16,
}
impl LocalFlightConfig {
    pub const fn is_valid(self) -> bool {
        self.session != 0
            && self.minimum_arming_time_q18 >= 0
            && self.minimum_arming_altitude_q13 >= 0
            && self.burnout_qualification_time_q18 > self.minimum_arming_time_q18
            && self.drogue_backup_time_q18 > self.burnout_qualification_time_q18
            && self.main_backup_time_q18 > self.drogue_backup_time_q18
            && self.main_altitude_q13 > 0
            && self.minimum_deployment_separation_q18 > 0
            && self.proportional_gain_q15 >= 0
            && self.derivative_gain_q15 >= 0
            && self.gimbal_limit_q15 >= 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalNavigation {
    pub position_q13: [i32; 3],
    pub velocity_q19: [i32; 3],
    pub attitude_vector: [i16; 3],
    pub checksum: u32,
}
impl LocalNavigation {
    pub const fn new(position_q13: [i32; 3], attitude_vector: [i16; 3]) -> Self {
        Self {
            position_q13,
            velocity_q19: [0; 3],
            attitude_vector,
            checksum: 0x811c_9dc5,
        }
    }
    fn inertial(&mut self, cell: &LocalInertialCell) {
        let mut a = 0;
        while a < 3 {
            let dv = (cell.delta_velocity[a] as i32) << 7;
            self.velocity_q19[a] = self.velocity_q19[a].saturating_add(dv);
            if a == 2 {
                self.velocity_q19[a] = self.velocity_q19[a].saturating_sub(GRAVITY_DELTA_V_Q19)
            }
            self.position_q13[a] = self.position_q13[a].saturating_add(self.velocity_q19[a] >> 11);
            self.attitude_vector[a] = cell.platform_angle[a];
            a += 1
        }
        self.checksum = hash_nav(
            self.checksum,
            cell.measurement_epoch,
            &self.position_q13,
            &self.velocity_q19,
        )
    }
    fn aid(&mut self, cell: &LocalAidCell) {
        if cell.validity & LOCAL_AID_BAROMETER != 0 {
            self.position_q13[2] = blend(self.position_q13[2], cell.barometer_q13, 2)
        }
        if cell.validity & LOCAL_AID_GPS != 0 {
            let mut a = 0;
            while a < 3 {
                self.position_q13[a] = blend(self.position_q13[a], cell.gps_position_q13[a], 3);
                self.velocity_q19[a] = blend(self.velocity_q19[a], cell.gps_velocity_q19[a], 3);
                a += 1
            }
        }
        self.checksum = hash_nav(
            self.checksum,
            cell.measurement_epoch,
            &self.position_q13,
            &self.velocity_q19,
        )
    }
}
fn blend(current: i32, measurement: i32, shift: u8) -> i32 {
    current.saturating_add(measurement.saturating_sub(current) >> shift)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalFlightEvidence {
    pub command: LocalCommandCell,
    pub status: Option<LocalStatusCell>,
    pub navigation: LocalNavigation,
    pub safe: bool,
    pub armed: bool,
    pub drogue_latched: bool,
    pub main_latched: bool,
    pub alarms: u16,
    pub flight_checksum: u32,
    pub deadline_misses: u16,
}

pub struct LocalFlightComputer {
    config: LocalFlightConfig,
    epoch: u16,
    navigation: LocalNavigation,
    alarms: u16,
    missing: u8,
    safe: bool,
    armed: bool,
    launched: bool,
    seen_powered: bool,
    last_powered: bool,
    burnout_qualified: bool,
    descending_count: u8,
    drogue_latched: bool,
    main_latched: bool,
    drogue_time_q18: i32,
    last_aid: Option<LocalAidCell>,
    last_command: LocalCommandCell,
    last_status: Option<LocalStatusCell>,
    flight_checksum: u32,
    deadline_misses: u16,
}
impl LocalFlightComputer {
    pub fn new(
        config: LocalFlightConfig,
        position_q13: [i32; 3],
        attitude_vector: [i16; 3],
    ) -> Option<Self> {
        if !config.is_valid() {
            return None;
        }
        Some(Self {
            config,
            epoch: 0,
            navigation: LocalNavigation::new(position_q13, attitude_vector),
            alarms: 0,
            missing: 0,
            safe: false,
            armed: false,
            launched: false,
            seen_powered: false,
            last_powered: false,
            burnout_qualified: false,
            descending_count: 0,
            drogue_latched: false,
            main_latched: false,
            drogue_time_q18: 0,
            last_aid: None,
            last_command: LocalCommandCell {
                session: config.session,
                source_epoch: 0,
                effective_epoch: 1,
                flags: 0,
                discrete: 0,
                gimbal: [0; 2],
                control_demand: [0; 2],
                status: 0,
            },
            last_status: None,
            flight_checksum: 0x811c_9dc5,
            deadline_misses: 0,
        })
    }
    pub const fn navigation(&self) -> LocalNavigation {
        self.navigation
    }
    pub const fn is_safe(&self) -> bool {
        self.safe
    }
    pub fn record_deadline_miss(&mut self) {
        self.deadline_misses = self.deadline_misses.saturating_add(1);
        self.alarms |= LOCAL_ALARM_DEADLINE;
        if self.deadline_misses >= 1 {
            self.safe = true;
            self.alarms |= LOCAL_ALARM_SAFE
        }
    }
    pub fn tick_in_place(
        &mut self,
        inertial: &LocalInertialCell,
        inertial_present: bool,
        aid: &LocalAidCell,
        aid_present: bool,
    ) {
        let epoch = self.epoch;
        let valid_inertial = inertial_present
            && inertial.session == self.config.session
            && inertial.measurement_epoch == epoch
            && inertial.production_epoch == epoch;
        if valid_inertial {
            self.navigation.inertial(inertial);
            self.missing = 0
        } else {
            self.missing = self.missing.saturating_add(1);
            self.alarms |= LOCAL_ALARM_INERTIAL | LOCAL_ALARM_LINK;
            if self.missing >= 3 {
                self.safe = true;
                self.alarms |= LOCAL_ALARM_SAFE
            }
        }
        let valid_aid = aid_present
            && aid.session == self.config.session
            && aid.measurement_epoch <= aid.production_epoch
            && aid.production_epoch <= epoch
            && epoch.wrapping_sub(aid.production_epoch) <= 32;
        if valid_aid {
            self.navigation.aid(aid);
            self.last_aid = Some(*aid)
        } else if aid_present {
            self.alarms |= LOCAL_ALARM_AID
        }
        let time_q18 = (epoch as i32).saturating_mul(8192);
        let measured_powered = if inertial_present {
            inertial.vehicle_status & 2 != 0
        } else {
            self.last_powered
        };
        if inertial_present {
            self.last_powered = measured_powered;
        }
        self.seen_powered |= measured_powered;
        if inertial_present {
            let acceleration_indicator = inertial.delta_velocity[0].unsigned_abs() as u32
                + inertial.delta_velocity[1].unsigned_abs() as u32
                + inertial.delta_velocity[2].unsigned_abs() as u32;
            if time_q18 >= (1 << 16) && acceleration_indicator > 1_800 {
                self.launched = true
            }
        }
        if self.launched
            && time_q18 >= self.config.minimum_arming_time_q18
            && self.navigation.position_q13[2] >= self.config.minimum_arming_altitude_q13
        {
            self.armed = true
        }
        if self.seen_powered
            && !measured_powered
            && time_q18 >= self.config.burnout_qualification_time_q18
        {
            self.burnout_qualified = true
        }
        let mut discrete = 0u8;
        if epoch & 3 == 0 {
            let continuity = self
                .last_aid
                .map(|a| a.validity & LOCAL_AID_CONTINUITY != 0 && a.continuity & 1 != 0)
                .unwrap_or(false);
            let feedback = self
                .last_aid
                .map(|a| {
                    if a.validity & LOCAL_AID_DEPLOYMENT_FEEDBACK != 0 {
                        a.deployment_feedback
                    } else {
                        0
                    }
                })
                .unwrap_or(0);
            if self.burnout_qualified && self.navigation.velocity_q19[2] < 0 {
                self.descending_count = self.descending_count.saturating_add(1)
            } else {
                self.descending_count = 0
            }
            let drogue_primary = self.armed && self.burnout_qualified && self.descending_count >= 2;
            let drogue_backup = self.armed && time_q18 >= self.config.drogue_backup_time_q18;
            if !self.drogue_latched && (drogue_primary || drogue_backup) {
                if continuity {
                    self.drogue_latched = true;
                    self.drogue_time_q18 = time_q18;
                    discrete |= LOCAL_COMMAND_DROGUE
                } else {
                    self.alarms |= LOCAL_ALARM_RECOVERY
                }
            }
            let drogue_ack = feedback & 1 != 0;
            let separation = time_q18.saturating_sub(self.drogue_time_q18)
                >= self.config.minimum_deployment_separation_q18;
            let main_primary = self.drogue_latched
                && drogue_ack
                && separation
                && self.navigation.velocity_q19[2] < 0
                && self.navigation.position_q13[2] <= self.config.main_altitude_q13;
            let main_backup =
                self.drogue_latched && separation && time_q18 >= self.config.main_backup_time_q18;
            if !self.main_latched && (main_primary || main_backup) {
                if continuity {
                    self.main_latched = true;
                    discrete |= LOCAL_COMMAND_MAIN
                } else {
                    self.alarms |= LOCAL_ALARM_RECOVERY
                }
            }
        }
        let mut demand = self.last_command.control_demand;
        if inertial_present {
            let value = inertial;
            let mut a = 0;
            while a < 2 {
                let angle = value.platform_angle[a + 1] as i32;
                let rate = value.angular_rate[a + 1] as i32;
                let p = (-angle * i32::from(self.config.proportional_gain_q15)) >> 15;
                let d = (-rate * i32::from(self.config.derivative_gain_q15)) >> 15;
                demand[a] = clamp_i16(p.saturating_add(d));
                a += 1
            }
        }
        let powered = measured_powered;
        let gimbal = if !self.safe
            && self.missing <= 2
            && powered
            && self.config.capability == LocalControlCapability::TwoAxisGimbal
        {
            if self.missing == 0 {
                [
                    demand[0].clamp(-self.config.gimbal_limit_q15, self.config.gimbal_limit_q15),
                    demand[1].clamp(-self.config.gimbal_limit_q15, self.config.gimbal_limit_q15),
                ]
            } else {
                self.last_command.gimbal
            }
        } else {
            [0; 2]
        };
        if self.safe {
            discrete = LOCAL_COMMAND_SAFE
        }
        let command = LocalCommandCell {
            session: self.config.session,
            source_epoch: epoch,
            effective_epoch: epoch.wrapping_add(1),
            flags: u8::from(self.safe),
            discrete,
            gimbal,
            control_demand: demand,
            status: self.alarms,
        };
        self.flight_checksum = hash_command(self.flight_checksum, command);
        self.last_command = command;
        let status = if epoch & 3 == 0 {
            Some(LocalStatusCell {
                session: self.config.session,
                source_epoch: epoch,
                production_epoch: epoch,
                mode: if self.safe {
                    7
                } else if self.main_latched {
                    5
                } else if self.drogue_latched {
                    4
                } else if self.armed {
                    2
                } else {
                    1
                },
                flags: 0,
                alarms: self.alarms,
                navigation_position_q13: self.navigation.position_q13,
                navigation_velocity_q19: self.navigation.velocity_q19,
                flight_checksum: self.flight_checksum,
                deadline_misses: self.deadline_misses,
                navigation_checksum: self.navigation.checksum as u16,
            })
        } else {
            None
        };
        self.last_status = status;
        self.epoch = self.epoch.wrapping_add(1);
    }
    pub const fn command(&self) -> LocalCommandCell {
        self.last_command
    }
    pub const fn status(&self) -> Option<LocalStatusCell> {
        self.last_status
    }
    pub const fn evidence(&self) -> LocalFlightEvidence {
        LocalFlightEvidence {
            command: self.last_command,
            status: self.last_status,
            navigation: self.navigation,
            safe: self.safe,
            armed: self.armed,
            drogue_latched: self.drogue_latched,
            main_latched: self.main_latched,
            alarms: self.alarms,
            flight_checksum: self.flight_checksum,
            deadline_misses: self.deadline_misses,
        }
    }
    pub fn tick(
        &mut self,
        inertial: Option<LocalInertialCell>,
        aid: Option<LocalAidCell>,
    ) -> LocalFlightEvidence {
        let missing_inertial = LocalInertialCell {
            session: 0,
            measurement_epoch: 0,
            production_epoch: 0,
            validity: 0,
            flags: 0,
            platform_angle: [0; 3],
            angular_rate: [0; 3],
            delta_velocity: [0; 3],
            gimbal_applied: [0; 2],
            vehicle_status: 0,
            actuator_feedback: 0,
        };
        let missing_aid = LocalAidCell {
            session: 0,
            measurement_epoch: 0,
            production_epoch: 0,
            validity: 0,
            events: 0,
            onboard_time_q18: 0,
            barometer_q13: 0,
            gps_position_q13: [0; 3],
            gps_velocity_q19: [0; 3],
            attitude_vector: [0; 3],
            continuity: 0,
            deployment_feedback: 0,
            vehicle_status: 0,
            clock_flags: 0,
        };
        self.tick_in_place(
            inertial.as_ref().unwrap_or(&missing_inertial),
            inertial.is_some(),
            aid.as_ref().unwrap_or(&missing_aid),
            aid.is_some(),
        );
        self.evidence()
    }
}
fn clamp_i16(v: i32) -> i16 {
    v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}
fn hw(h: u32, value: u32) -> u32 {
    h.rotate_left(5).wrapping_add(0x9e37_79b9) ^ value
}
fn hash_nav(mut h: u32, e: u16, p: &[i32; 3], v: &[i32; 3]) -> u32 {
    h = hw(h, e as u32);
    for x in p {
        h = hw(h, *x as u32)
    }
    for x in v {
        h = hw(h, *x as u32)
    }
    h
}
fn hash_command(mut h: u32, c: LocalCommandCell) -> u32 {
    h = hw(h, c.source_epoch as u32);
    h = hw(h, c.effective_epoch as u32);
    h = hw(h, c.gimbal[0] as u16 as u32);
    h = hw(h, c.gimbal[1] as u16 as u32);
    hw(h, c.discrete as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_interface::phase8_5::{LOCAL_AID_VALID_MASK, LOCAL_INERTIAL_VALID_MASK};
    fn config(capability: LocalControlCapability) -> LocalFlightConfig {
        LocalFlightConfig {
            session: 0x8501,
            capability,
            minimum_arming_time_q18: 1 << 18,
            minimum_arming_altitude_q13: 10 << 13,
            burnout_qualification_time_q18: 3 << 18,
            drogue_backup_time_q18: 15 << 18,
            main_backup_time_q18: 65 << 18,
            main_altitude_q13: 200 << 13,
            minimum_deployment_separation_q18: 2 << 18,
            proportional_gain_q15: 8192,
            derivative_gain_q15: 4096,
            gimbal_limit_q15: 455,
        }
    }
    fn inertial(e: u16, powered: bool) -> LocalInertialCell {
        LocalInertialCell {
            session: 0x8501,
            measurement_epoch: e,
            production_epoch: e,
            validity: LOCAL_INERTIAL_VALID_MASK,
            flags: 0,
            platform_angle: [0, 100, -100],
            angular_rate: [0, 0, 0],
            delta_velocity: [0, 0, if powered { 2_500 } else { 1_255 }],
            gimbal_applied: [0; 2],
            vehicle_status: if powered { 2 } else { 0 },
            actuator_feedback: 0,
        }
    }
    fn aid(e: u16, alt: i32, vel: i32, feedback: u16) -> LocalAidCell {
        LocalAidCell {
            session: 0x8501,
            measurement_epoch: e,
            production_epoch: e,
            validity: LOCAL_AID_VALID_MASK,
            events: 0,
            onboard_time_q18: e as i32 * 8192,
            barometer_q13: alt,
            gps_position_q13: [0, 0, alt],
            gps_velocity_q19: [0, 0, vel],
            attitude_vector: [0; 3],
            continuity: 1,
            deployment_feedback: feedback,
            vehicle_status: 0,
            clock_flags: 0,
        }
    }
    #[test]
    fn monitor_only_never_commands_gimbal() {
        let mut f =
            LocalFlightComputer::new(config(LocalControlCapability::MonitorOnly), [0; 3], [0; 3])
                .unwrap();
        for e in 0..16 {
            let out = f.tick(
                Some(inertial(e, true)),
                if e & 3 == 0 {
                    Some(aid(e, 100 << 13, 1 << 19, 0))
                } else {
                    None
                },
            );
            assert_eq!(out.command.gimbal, [0; 2]);
            assert_ne!(out.command.control_demand, [0; 2]);
        }
    }
    #[test]
    fn two_missing_hold_and_third_safe() {
        let mut f = LocalFlightComputer::new(
            config(LocalControlCapability::TwoAxisGimbal),
            [0; 3],
            [0; 3],
        )
        .unwrap();
        let a = f.tick(Some(inertial(0, true)), Some(aid(0, 100 << 13, 1 << 19, 0)));
        assert_ne!(a.command.gimbal, [0; 2]);
        let hold_one = f.tick(None, None);
        assert!(!hold_one.safe);
        assert_eq!(hold_one.command.gimbal, a.command.gimbal);
        let hold_two = f.tick(None, None);
        assert!(!hold_two.safe);
        assert_eq!(hold_two.command.gimbal, a.command.gimbal);
        let safe = f.tick(None, None);
        assert!(safe.safe);
        assert_eq!(safe.command.gimbal, [0; 2]);
        assert_eq!(safe.command.discrete, LOCAL_COMMAND_SAFE);
    }
    #[test]
    fn measured_descent_and_backups_are_one_shot() {
        let mut f = LocalFlightComputer::new(
            config(LocalControlCapability::MonitorOnly),
            [0, 0, 100 << 13],
            [0; 3],
        )
        .unwrap();
        let mut drogue = 0;
        let mut main = 0;
        for e in 0..(66 * 32) as u16 {
            let descending = if e >= 4 * 32 { -(10 << 19) } else { 10 << 19 };
            let feedback = if drogue > 0 { 1 } else { 0 };
            let out = f.tick(
                Some(inertial(e, e < 3 * 32)),
                if e & 3 == 0 {
                    Some(aid(
                        e,
                        if e > 60 * 32 { 100 << 13 } else { 300 << 13 },
                        descending,
                        feedback,
                    ))
                } else {
                    None
                },
            );
            if out.command.discrete & LOCAL_COMMAND_DROGUE != 0 {
                drogue += 1
            }
            if out.command.discrete & LOCAL_COMMAND_MAIN != 0 {
                main += 1
            }
        }
        assert_eq!(drogue, 1);
        assert_eq!(main, 1);
    }
}
