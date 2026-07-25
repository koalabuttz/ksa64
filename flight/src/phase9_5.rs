//! Truth-blind Phase 9.5 advanced-effector avionics wrapper.

use crate::phase8_5::{
    LocalControlCapability, LocalFlightComputer, LocalFlightConfig, LocalFlightEvidence,
};
use ksa64_interface::phase8_5::{
    LocalAidCell, LocalInertialCell, LOCAL_AID_ATTITUDE, LOCAL_AID_BAROMETER, LOCAL_AID_CONTINUITY,
    LOCAL_AID_DEPLOYMENT_FEEDBACK, LOCAL_AID_GPS, LOCAL_INERTIAL_VALID_ACTUATOR,
    LOCAL_INERTIAL_VALID_DELTA_V, LOCAL_INERTIAL_VALID_PLATFORM, LOCAL_INERTIAL_VALID_RATE,
};
use ksa64_interface::phase9_5::{
    AdvancedAidCell, AdvancedCommandCell, AdvancedFastSensorCell, AdvancedStatusCell,
    ADVANCED_AID_ATTITUDE, ADVANCED_AID_BAROMETER, ADVANCED_AID_CONTINUITY,
    ADVANCED_AID_DEPLOYMENT_FEEDBACK, ADVANCED_AID_GPS, ADVANCED_COMMAND_FLAG_AIRDATA_FALLBACK,
    ADVANCED_COMMAND_FLAG_HOLD, ADVANCED_COMMAND_FLAG_RCS_RESERVED, ADVANCED_COMMAND_SAFE,
    ADVANCED_VALID_ACTUATOR, ADVANCED_VALID_AIR_DATA, ADVANCED_VALID_DELTA_V,
    ADVANCED_VALID_PLATFORM, ADVANCED_VALID_RATE, ADVANCED_VALID_SUPPLY,
};

pub const ADVANCED_FLIGHT_CONTRACT_ID: u32 = 0x0953_0001;
pub const ADVANCED_ALARM_AIR_DATA: u16 = 1 << 8;
pub const ADVANCED_ALARM_SUPPLY: u16 = 1 << 9;
pub const ADVANCED_ALARM_AUTHORITY: u16 = 1 << 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AirDataSource {
    Pitot = 1,
    ConservativeFallback = 2,
    Unavailable = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AirDataEstimate {
    pub source: AirDataSource,
    pub dynamic_pressure_q10: i32,
    pub mach_q12: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFlightConfig {
    pub local: LocalFlightConfig,
    pub roll_proportional_gain_q15: i16,
    pub roll_derivative_gain_q15: i16,
    pub torque_limit_q12: [i32; 3],
    pub fallback_density_upper_q10: i32,
    pub maximum_wind_q19: i32,
    pub minimum_sound_speed_mps: u16,
    pub maximum_navigation_speed_mps: u16,
    pub propellant_wet_q21: i32,
    pub reserve_q15: u16,
}
impl AdvancedFlightConfig {
    pub const fn is_valid(self) -> bool {
        self.local.is_valid()
            && matches!(self.local.capability, LocalControlCapability::MonitorOnly)
            && self.roll_proportional_gain_q15 >= 0
            && self.roll_derivative_gain_q15 >= 0
            && self.torque_limit_q12[0] > 0
            && self.torque_limit_q12[1] > 0
            && self.torque_limit_q12[2] > 0
            && self.fallback_density_upper_q10 > 0
            && self.maximum_wind_q19 >= 0
            && self.minimum_sound_speed_mps > 0
            && self.maximum_navigation_speed_mps > 0
            && self.propellant_wet_q21 > 0
            && self.reserve_q15 <= 32_768
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFlightEvidence {
    pub command: AdvancedCommandCell,
    pub status: Option<AdvancedStatusCell>,
    pub local: LocalFlightEvidence,
    pub air_data: AirDataEstimate,
    pub sensor_checksum: u32,
    pub demand_checksum: u32,
    pub command_checksum: u32,
    pub missing_fast_epochs: u8,
}

pub struct AdvancedFlightComputer {
    config: AdvancedFlightConfig,
    local: LocalFlightComputer,
    epoch: u16,
    roll_target: i16,
    missing_fast_epochs: u8,
    sensor_checksum: u32,
    demand_checksum: u32,
    command_checksum: u32,
    last_command: AdvancedCommandCell,
    last_status: Option<AdvancedStatusCell>,
    last_air_data: AirDataEstimate,
}

impl AdvancedFlightComputer {
    pub fn new(
        config: AdvancedFlightConfig,
        position_q13: [i32; 3],
        attitude_vector: [i16; 3],
    ) -> Option<Self> {
        if !config.is_valid() {
            return None;
        }
        let local = LocalFlightComputer::new(config.local, position_q13, attitude_vector)?;
        Some(Self {
            config,
            local,
            epoch: 0,
            roll_target: attitude_vector[0],
            missing_fast_epochs: 0,
            sensor_checksum: 0x811c_9dc5,
            demand_checksum: 0x811c_9dc5,
            command_checksum: 0x811c_9dc5,
            last_command: AdvancedCommandCell {
                session: config.local.session,
                source_epoch: 0,
                effective_epoch: 1,
                flags: 0,
                discrete: 0,
                gimbal: [0; 2],
                canards: [0; 4],
                torque_demand_q12: [0; 3],
                rcs_pulse_quanta: [0; 12],
                status: 0,
                authority_mode: 0,
                command_checksum: 0x811c_9dc5,
            },
            last_status: None,
            last_air_data: AirDataEstimate {
                source: AirDataSource::Unavailable,
                dynamic_pressure_q10: 0,
                mach_q12: 0,
            },
        })
    }

    pub const fn local(&self) -> &LocalFlightComputer {
        &self.local
    }
    pub const fn command(&self) -> AdvancedCommandCell {
        self.last_command
    }
    pub const fn status(&self) -> Option<AdvancedStatusCell> {
        self.last_status
    }

    pub fn record_deadline_miss(&mut self) {
        self.local.record_deadline_miss();
    }

    pub fn tick(
        &mut self,
        fast: Option<AdvancedFastSensorCell>,
        aid: Option<AdvancedAidCell>,
    ) -> AdvancedFlightEvidence {
        let valid_fast = fast.filter(|cell| {
            cell.session == self.config.local.session
                && cell.measurement_epoch == self.epoch
                && cell.production_epoch == self.epoch
                && cell.validity
                    & (ADVANCED_VALID_PLATFORM | ADVANCED_VALID_RATE | ADVANCED_VALID_DELTA_V)
                    == (ADVANCED_VALID_PLATFORM | ADVANCED_VALID_RATE | ADVANCED_VALID_DELTA_V)
        });
        if valid_fast.is_some() {
            self.missing_fast_epochs = 0;
        } else {
            self.missing_fast_epochs = self.missing_fast_epochs.saturating_add(1);
        }
        if let Some(cell) = valid_fast {
            self.sensor_checksum = hash_fast(self.sensor_checksum, &cell);
        }
        let local_inertial = valid_fast.map(to_local_inertial);
        let local_aid = aid.map(to_local_aid);
        let local = self.local.tick(local_inertial, local_aid);
        let air_data = self.resolve_air_data(valid_fast.as_ref(), &local);
        let mut demand_i16 = [0i16; 3];
        if let Some(cell) = valid_fast {
            let angle = i32::from(cell.platform_angle[0].saturating_sub(self.roll_target));
            let rate = i32::from(cell.angular_rate[0]);
            let p = (-angle * i32::from(self.config.roll_proportional_gain_q15)) >> 15;
            let d = (-rate * i32::from(self.config.roll_derivative_gain_q15)) >> 15;
            demand_i16[0] = clamp_i16(p.saturating_add(d));
            demand_i16[1] = local.command.control_demand[0];
            demand_i16[2] = local.command.control_demand[1];
        }
        let mut torque = [0; 3];
        for axis in 0..3 {
            torque[axis] = ((i64::from(demand_i16[axis])
                * i64::from(self.config.torque_limit_q12[axis]))
                >> 15)
                .clamp(
                    i64::from(-self.config.torque_limit_q12[axis]),
                    i64::from(self.config.torque_limit_q12[axis]),
                ) as i32;
        }
        let held = self.missing_fast_epochs != 0 && self.missing_fast_epochs <= 2;
        if held {
            torque = self.last_command.torque_demand_q12;
        }
        if self.missing_fast_epochs >= 3 || local.safe {
            torque = [0; 3];
        }
        self.demand_checksum = hash_demand(self.demand_checksum, self.epoch, torque);
        let mut flags = 0u8;
        if held {
            flags |= ADVANCED_COMMAND_FLAG_HOLD;
        }
        if air_data.source == AirDataSource::ConservativeFallback {
            flags |= ADVANCED_COMMAND_FLAG_AIRDATA_FALLBACK;
        }
        let propellant = valid_fast.map(|cell| cell.propellant_q21).unwrap_or(0);
        let reserve_raw = ((i64::from(self.config.propellant_wet_q21)
            * i64::from(self.config.reserve_q15))
            >> 15) as i32;
        let supply_valid = valid_fast
            .map(|cell| cell.validity & ADVANCED_VALID_SUPPLY != 0)
            .unwrap_or(false);
        if supply_valid && propellant <= reserve_raw {
            flags |= ADVANCED_COMMAND_FLAG_RCS_RESERVED;
        }
        let safe = self.missing_fast_epochs >= 3 || local.safe;
        let discrete = if safe {
            ADVANCED_COMMAND_SAFE
        } else if self.missing_fast_epochs == 0 {
            local.command.discrete
        } else {
            0
        };
        let mut command = AdvancedCommandCell {
            session: self.config.local.session,
            source_epoch: self.epoch,
            effective_epoch: self.epoch.wrapping_add(1),
            flags,
            discrete,
            gimbal: [0; 2],
            canards: [0; 4],
            torque_demand_q12: torque,
            rcs_pulse_quanta: [0; 12],
            status: local.alarms
                | air_alarm(air_data)
                | (u16::from(!supply_valid) * ADVANCED_ALARM_SUPPLY),
            authority_mode: 0,
            command_checksum: 0,
        };
        self.command_checksum = hash_command(self.command_checksum, &command);
        command.command_checksum = self.command_checksum;
        let status = local.status.map(|value| AdvancedStatusCell {
            session: value.session,
            source_epoch: value.source_epoch,
            production_epoch: value.production_epoch,
            mode: value.mode,
            flags,
            alarms: command.status,
            navigation_position_q13: value.navigation_position_q13,
            navigation_velocity_q19: value.navigation_velocity_q19,
            flight_checksum: value.flight_checksum,
            deadline_misses: value.deadline_misses,
            navigation_checksum: value.navigation_checksum,
            authority_state: 0,
            requested_torque_q12: torque,
            achieved_torque_q12: [0; 3],
            residual_torque_q12: torque.map(clamp_i16),
            saturation_count: 0,
            reserve_q15: reserve_fraction(propellant, self.config.propellant_wet_q21),
            actuator_flags: 0,
        });
        self.last_air_data = air_data;
        self.last_command = command;
        self.last_status = status;
        self.epoch = self.epoch.wrapping_add(1);
        AdvancedFlightEvidence {
            command,
            status,
            local,
            air_data,
            sensor_checksum: self.sensor_checksum,
            demand_checksum: self.demand_checksum,
            command_checksum: self.command_checksum,
            missing_fast_epochs: self.missing_fast_epochs,
        }
    }

    fn resolve_air_data(
        &self,
        fast: Option<&AdvancedFastSensorCell>,
        local: &LocalFlightEvidence,
    ) -> AirDataEstimate {
        if let Some(cell) = fast {
            if cell.validity & ADVANCED_VALID_AIR_DATA != 0
                && cell.dynamic_pressure_q10 >= 0
                && cell.mach_q12 >= 0
            {
                return AirDataEstimate {
                    source: AirDataSource::Pitot,
                    dynamic_pressure_q10: cell.dynamic_pressure_q10,
                    mach_q12: cell.mach_q12,
                };
            }
        }
        let mut speed_bound_q19 = self.config.maximum_wind_q19;
        for value in local.navigation.velocity_q19 {
            speed_bound_q19 =
                speed_bound_q19.saturating_add(value.unsigned_abs().min(i32::MAX as u32) as i32);
        }
        let speed_mps = speed_bound_q19.saturating_add((1 << 19) - 1) >> 19;
        if speed_mps < 0 || speed_mps > i32::from(self.config.maximum_navigation_speed_mps) {
            return AirDataEstimate {
                source: AirDataSource::Unavailable,
                dynamic_pressure_q10: 0,
                mach_q12: 0,
            };
        }
        let speed_squared = speed_mps.saturating_mul(speed_mps);
        let pressure = speed_squared.saturating_mul(self.config.fallback_density_upper_q10) / 2;
        let mach = ((speed_mps << 12) / i32::from(self.config.minimum_sound_speed_mps))
            .clamp(0, i16::MAX as i32) as i16;
        AirDataEstimate {
            source: AirDataSource::ConservativeFallback,
            dynamic_pressure_q10: pressure,
            mach_q12: mach,
        }
    }
}

fn to_local_inertial(cell: AdvancedFastSensorCell) -> LocalInertialCell {
    let mut validity = 0u8;
    if cell.validity & ADVANCED_VALID_PLATFORM != 0 {
        validity |= LOCAL_INERTIAL_VALID_PLATFORM;
    }
    if cell.validity & ADVANCED_VALID_RATE != 0 {
        validity |= LOCAL_INERTIAL_VALID_RATE;
    }
    if cell.validity & ADVANCED_VALID_DELTA_V != 0 {
        validity |= LOCAL_INERTIAL_VALID_DELTA_V;
    }
    if cell.validity & ADVANCED_VALID_ACTUATOR != 0 {
        validity |= LOCAL_INERTIAL_VALID_ACTUATOR;
    }
    LocalInertialCell {
        session: cell.session,
        measurement_epoch: cell.measurement_epoch,
        production_epoch: cell.production_epoch,
        validity,
        flags: cell.flags as u8,
        platform_angle: cell.platform_angle,
        angular_rate: cell.angular_rate,
        delta_velocity: cell.delta_velocity,
        gimbal_applied: cell.gimbal_applied,
        vehicle_status: cell.vehicle_status,
        actuator_feedback: cell.actuator_feedback,
    }
}
fn to_local_aid(cell: AdvancedAidCell) -> LocalAidCell {
    let mut validity = 0u16;
    if cell.validity & ADVANCED_AID_BAROMETER != 0 {
        validity |= LOCAL_AID_BAROMETER;
    }
    if cell.validity & ADVANCED_AID_GPS != 0 {
        validity |= LOCAL_AID_GPS;
    }
    if cell.validity & ADVANCED_AID_ATTITUDE != 0 {
        validity |= LOCAL_AID_ATTITUDE;
    }
    if cell.validity & ADVANCED_AID_CONTINUITY != 0 {
        validity |= LOCAL_AID_CONTINUITY;
    }
    if cell.validity & ADVANCED_AID_DEPLOYMENT_FEEDBACK != 0 {
        validity |= LOCAL_AID_DEPLOYMENT_FEEDBACK;
    }
    LocalAidCell {
        session: cell.session,
        measurement_epoch: cell.measurement_epoch,
        production_epoch: cell.production_epoch,
        validity,
        events: cell.events,
        onboard_time_q18: cell.onboard_time_q18,
        barometer_q13: cell.barometer_q13,
        gps_position_q13: cell.gps_position_q13,
        gps_velocity_q19: cell.gps_velocity_q19,
        attitude_vector: cell.attitude_vector,
        continuity: cell.continuity,
        deployment_feedback: cell.deployment_feedback,
        vehicle_status: cell.vehicle_status,
        clock_flags: cell.clock_flags,
    }
}
fn clamp_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}
fn reserve_fraction(remaining: i32, wet: i32) -> u16 {
    if remaining <= 0 || wet <= 0 {
        0
    } else {
        ((i64::from(remaining) << 15) / i64::from(wet)).clamp(0, 32_768) as u16
    }
}
fn air_alarm(air: AirDataEstimate) -> u16 {
    u16::from(air.source != AirDataSource::Pitot) * ADVANCED_ALARM_AIR_DATA
}
fn mix(mut hash: u32, value: u32) -> u32 {
    hash = hash.rotate_left(5).wrapping_add(0x9e37_79b9);
    hash ^ value
}
fn hash_fast(mut hash: u32, cell: &AdvancedFastSensorCell) -> u32 {
    hash = mix(hash, u32::from(cell.measurement_epoch));
    hash = mix(hash, cell.dynamic_pressure_q10 as u32);
    hash = mix(hash, cell.propellant_q21 as u32);
    for v in cell.platform_angle {
        hash = mix(hash, v as u16 as u32)
    }
    hash
}
fn hash_demand(mut hash: u32, epoch: u16, demand: [i32; 3]) -> u32 {
    hash = mix(hash, u32::from(epoch));
    for v in demand {
        hash = mix(hash, v as u32)
    }
    hash
}
fn hash_command(mut hash: u32, command: &AdvancedCommandCell) -> u32 {
    hash = mix(hash, u32::from(command.source_epoch));
    for v in command.torque_demand_q12 {
        hash = mix(hash, v as u32)
    }
    hash = mix(hash, u32::from(command.discrete));
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_interface::phase8_5::{write_local_command, LOCAL_COMMAND_LENGTH};
    fn local_config() -> LocalFlightConfig {
        LocalFlightConfig {
            session: 0x9510,
            capability: LocalControlCapability::MonitorOnly,
            minimum_arming_time_q18: 1 << 18,
            minimum_arming_altitude_q13: 10 << 13,
            burnout_qualification_time_q18: 3 << 18,
            drogue_backup_time_q18: 15 << 18,
            main_backup_time_q18: 65 << 18,
            main_altitude_q13: 200 << 13,
            minimum_deployment_separation_q18: 2 << 18,
            proportional_gain_q15: 8192,
            derivative_gain_q15: 4096,
            gimbal_limit_q15: 0,
        }
    }
    fn config() -> AdvancedFlightConfig {
        AdvancedFlightConfig {
            local: local_config(),
            roll_proportional_gain_q15: 8192,
            roll_derivative_gain_q15: 4096,
            torque_limit_q12: [4096; 3],
            fallback_density_upper_q10: 1255,
            maximum_wind_q19: 10 << 19,
            minimum_sound_speed_mps: 300,
            maximum_navigation_speed_mps: 400,
            propellant_wet_q21: 209_715,
            reserve_q15: 6554,
        }
    }
    fn fast(epoch: u16, air: bool) -> AdvancedFastSensorCell {
        AdvancedFastSensorCell {
            session: 0x9510,
            measurement_epoch: epoch,
            production_epoch: epoch,
            validity: ADVANCED_VALID_PLATFORM
                | ADVANCED_VALID_RATE
                | ADVANCED_VALID_DELTA_V
                | ADVANCED_VALID_ACTUATOR
                | ADVANCED_VALID_SUPPLY
                | if air { ADVANCED_VALID_AIR_DATA } else { 0 },
            platform_angle: [100, -100, 50],
            angular_rate: [5, 0, 0],
            delta_velocity: [0, 0, 2500],
            dynamic_pressure_q10: 1200 << 10,
            mach_q12: 1024,
            gimbal_applied: [0; 2],
            canard_applied: [0; 4],
            valve_open_mask: 0,
            propellant_q21: 209_715,
            supply_scale_q15: 32768,
            vehicle_status: 2,
            actuator_feedback: 0,
            flags: 0,
        }
    }
    #[test]
    fn wrapper_preserves_frozen_local_kernel_output() {
        let mut legacy = LocalFlightComputer::new(local_config(), [0; 3], [0; 3]).unwrap();
        let mut advanced = AdvancedFlightComputer::new(config(), [0; 3], [0; 3]).unwrap();
        for epoch in 0..8 {
            let f = fast(epoch, true);
            let l = legacy.tick(Some(to_local_inertial(f)), None);
            let a = advanced.tick(Some(f), None);
            assert_eq!(a.local, l);
            let mut left = [0; LOCAL_COMMAND_LENGTH];
            let mut right = [0; LOCAL_COMMAND_LENGTH];
            write_local_command(&l.command, &mut left).unwrap();
            write_local_command(&a.local.command, &mut right).unwrap();
            assert_eq!(left, right);
        }
    }
    #[test]
    fn pitot_fallback_is_conservative_and_unavailable_fails_closed() {
        let mut f = AdvancedFlightComputer::new(config(), [0; 3], [0; 3]).unwrap();
        let out = f.tick(Some(fast(0, false)), None);
        assert_eq!(out.air_data.source, AirDataSource::ConservativeFallback);
        assert!(out.command.flags & ADVANCED_COMMAND_FLAG_AIRDATA_FALLBACK != 0);
        let mut bad = config();
        bad.maximum_navigation_speed_mps = 1;
        let mut unavailable = AdvancedFlightComputer::new(bad, [0; 3], [0; 3]).unwrap();
        let out = unavailable.tick(Some(fast(0, false)), None);
        assert_eq!(out.air_data.source, AirDataSource::Unavailable);
        assert!(out.command.status & ADVANCED_ALARM_AIR_DATA != 0);
    }
    #[test]
    fn continuous_demand_holds_two_epochs_and_discrete_never_replays() {
        let mut f = AdvancedFlightComputer::new(config(), [0; 3], [0; 3]).unwrap();
        let first = f.tick(Some(fast(0, true)), None);
        assert_ne!(first.command.torque_demand_q12, [0; 3]);
        let one = f.tick(None, None);
        let two = f.tick(None, None);
        let three = f.tick(None, None);
        assert_eq!(
            one.command.torque_demand_q12,
            first.command.torque_demand_q12
        );
        assert_eq!(
            two.command.torque_demand_q12,
            first.command.torque_demand_q12
        );
        assert_eq!(one.command.discrete, 0);
        assert_eq!(two.command.discrete, 0);
        assert_eq!(three.command.torque_demand_q12, [0; 3]);
        assert_eq!(three.command.discrete, ADVANCED_COMMAND_SAFE);
    }
    #[test]
    fn bounded_mos_probe_signature_is_frozen() {
        let mut flight = AdvancedFlightComputer::new(config(), [0; 3], [0; 3]).unwrap();
        flight.tick(Some(fast(0, true)), None);
        flight.tick(None, None);
        flight.tick(None, None);
        let final_epoch = flight.tick(None, None);
        assert_eq!(final_epoch.command_checksum, 0x8c16_5977);
    }

    #[test]
    fn roll_demand_and_checksum_chains_are_deterministic() {
        let mut a = AdvancedFlightComputer::new(config(), [0; 3], [0; 3]).unwrap();
        let mut b = AdvancedFlightComputer::new(config(), [0; 3], [0; 3]).unwrap();
        for e in 0..16 {
            assert_eq!(
                a.tick(Some(fast(e, true)), None),
                b.tick(Some(fast(e, true)), None)
            );
        }
        assert_ne!(a.command().torque_demand_q12[0], 0);
    }
}
