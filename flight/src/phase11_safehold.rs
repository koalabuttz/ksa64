//! Independent Phase 11 contingency flight package.
//!
//! `SafeholdRecoveryV1` deliberately does not wrap `GlobalFlightComputer`.
//! It consumes the same public KLR10 cells, maintains its own bounded
//! navigation state, and implements only coast, entry, and local recovery.

use crate::phase10::{
    GlobalFlightEvidence, GlobalFlightMode, GlobalNavigation, GLOBAL_ALARM_AID,
    GLOBAL_ALARM_FAST_SENSOR, GLOBAL_ALARM_FRAME, GLOBAL_ALARM_LINK, GLOBAL_ALARM_RECOVERY,
    GLOBAL_ALARM_SAFE,
};
use crate::phase11::{EventJournal, GlobalKlr10FlightPackage};
use ksa64_core::numeric::{divide_scaled_truncating, magnitude4_floor, NumericStatus};
use ksa64_interface::phase10::{
    GlobalAidFrameCell, GlobalCommandCell, GlobalFastSensorCell, GlobalFrameId, GlobalStatusCell,
    GlobalTransitionCell, GLOBAL_AID_ATTITUDE, GLOBAL_AID_BAROMETER, GLOBAL_AID_CONTINUITY,
    GLOBAL_AID_DEPLOYMENT_FEEDBACK, GLOBAL_AID_GNSS, GLOBAL_COMMAND_DROGUE, GLOBAL_COMMAND_MAIN,
    GLOBAL_COMMAND_RCS_RESERVED, GLOBAL_COMMAND_SAFE, GLOBAL_FAST_ATTITUDE,
    GLOBAL_FAST_DELTA_ANGLE, GLOBAL_FAST_DELTA_V, KLR10_CONTRACT_ID,
};
use ksa64_interface::phase11::{
    EventJournalRecord, FlightAbiId, FlightSoftwarePackageId, FlightSoftwarePackageManifest,
    JournalEventKind, PackageCommandLossBehavior, PackageResourceClaim, PackageSafeStateId,
    PACKAGE_CAP_EVENT_JOURNAL, PACKAGE_SEGMENT_ECEF_ENTRY, PACKAGE_SEGMENT_ECI_COAST,
    PACKAGE_SEGMENT_LOCAL_RECOVERY, PACKAGE_TARGET_HOST, PACKAGE_TARGET_RUST_MOS,
};

pub const SAFEHOLD_RECOVERY_MANIFEST_ID: u32 = 0x11f5_a002;
pub const SAFEHOLD_RECOVERY_IMPLEMENTATION_ID: u32 = 0x11f5_1002;
pub const SAFEHOLD_RECOVERY_CODE_ID: u32 = 0x11f5_2002;
pub const SAFEHOLD_RECOVERY_MISSION_ID: u32 = 0x11a0_0002;

const Q30_ONE: i32 = 1 << 30;
const SUBSONIC_MACH_Q12: i16 = 3_277;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SafeholdSessionSegment {
    EciCoast = 1,
    EcefEntry = 2,
    LocalRecovery = 3,
}

impl SafeholdSessionSegment {
    const fn frame(self) -> GlobalFrameId {
        match self {
            Self::EciCoast => GlobalFrameId::EarthInertialEciV1,
            Self::EcefEntry => GlobalFrameId::EarthFixedEcefV1,
            Self::LocalRecovery => GlobalFrameId::LocalEnuV1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeholdRecoveryConfig {
    pub session: u16,
    pub initial_segment: SafeholdSessionSegment,
    pub initial_position_q12: [i32; 3],
    pub initial_velocity_q24: [i32; 3],
    pub initial_attitude_q30: [i32; 4],
    pub attitude_target_q30: [i32; 4],
    pub proportional_gain_q15: [i16; 3],
    pub derivative_gain_q15: [i16; 3],
    pub torque_limit_q12: [i32; 3],
    pub drogue_backup_time_q16: u32,
    pub main_backup_time_q16: u32,
    pub main_altitude_q12_km: i32,
    pub minimum_deployment_separation_q16: u32,
}

impl SafeholdRecoveryConfig {
    pub fn is_valid(self) -> bool {
        self.session != 0
            && self.initial_attitude_q30 != [0; 4]
            && self.attitude_target_q30 != [0; 4]
            && self.proportional_gain_q15.iter().all(|value| *value >= 0)
            && self.derivative_gain_q15.iter().all(|value| *value >= 0)
            && self.torque_limit_q12.iter().all(|value| *value > 0)
            && self.main_backup_time_q16 > self.drogue_backup_time_q16
            && self.main_altitude_q12_km > 0
            && self.minimum_deployment_separation_q16 > 0
    }
}

pub const fn ksa_g10r_safehold_coast_config() -> SafeholdRecoveryConfig {
    SafeholdRecoveryConfig {
        session: 0x10a0,
        initial_segment: SafeholdSessionSegment::EciCoast,
        initial_position_q12: [26_558_400, 0, 0],
        initial_velocity_q24: [0, 131_701_145, 0],
        initial_attitude_q30: [Q30_ONE, 0, 0, 0],
        attitude_target_q30: [Q30_ONE, 0, 0, 0],
        proportional_gain_q15: [6_144; 3],
        derivative_gain_q15: [24_576; 3],
        torque_limit_q12: [12_288; 3],
        drogue_backup_time_q16: 39_321_600,
        main_backup_time_q16: 58_982_400,
        main_altitude_q12_km: 6_144,
        minimum_deployment_separation_q16: 131_072,
    }
}

pub const fn safehold_recovery_manifest() -> FlightSoftwarePackageManifest {
    FlightSoftwarePackageManifest {
        manifest_identity: SAFEHOLD_RECOVERY_MANIFEST_ID,
        package: FlightSoftwarePackageId::SafeholdRecoveryV1,
        implementation_identity: SAFEHOLD_RECOVERY_IMPLEMENTATION_ID,
        abi: FlightAbiId::GlobalKlr10V1,
        vehicle_profile_identity: super::phase11::GLOBAL_ECEF_PROFILE_ID,
        mission_compatibility_identity: SAFEHOLD_RECOVERY_MISSION_ID,
        capabilities: PACKAGE_CAP_EVENT_JOURNAL,
        segment_support: PACKAGE_SEGMENT_ECI_COAST
            | PACKAGE_SEGMENT_ECEF_ENTRY
            | PACKAGE_SEGMENT_LOCAL_RECOVERY,
        command_load_support: 0,
        targets: PACKAGE_TARGET_HOST | PACKAGE_TARGET_RUST_MOS,
        safe_state: PackageSafeStateId::EntryRecoverySafeholdV1,
        command_loss: PackageCommandLossBehavior::ImmediateSafehold,
        resource: PackageResourceClaim {
            persistent_bytes: 1_536,
            transient_bytes: 768,
            stack_bytes: 256,
            journal_records: 32,
            maximum_object_bytes: 512,
        },
        fast_hz: 32,
        navigation_hz: 8,
        guidance_hz: 1,
        maximum_plan_events: 0,
        maximum_branches: 0,
        maximum_decisions: 0,
        code_identity: SAFEHOLD_RECOVERY_CODE_ID,
        configuration_identity: KLR10_CONTRACT_ID,
        resource_evidence_sha256: [
            0x6f, 0x73, 0x17, 0x79, 0x36, 0xf0, 0xe6, 0x53, 0x62, 0xad, 0x45, 0x3f, 0xe3, 0x87,
            0x39, 0xf2, 0x94, 0x8c, 0x4a, 0x26, 0x02, 0xa3, 0x7e, 0x5f, 0xb4, 0x5c, 0x73, 0xe7,
            0x6c, 0x04, 0xc4, 0x05,
        ],
    }
}

pub struct SafeholdRecoveryV1 {
    config: SafeholdRecoveryConfig,
    segment: SafeholdSessionSegment,
    epoch: u16,
    navigation: GlobalNavigation,
    attitude_target_q30: [i32; 4],
    last_time_q16: u32,
    last_fast: Option<GlobalFastSensorCell>,
    missing_fast: u8,
    safe: bool,
    descending_count: u8,
    drogue_latched: bool,
    main_latched: bool,
    drogue_time_q16: u32,
    alarms: u16,
    sensor_checksum: u32,
    command_checksum: u32,
    flight_checksum: u32,
    transition_count: u8,
    last_mode: GlobalFlightMode,
    journal: EventJournal,
}

impl SafeholdRecoveryV1 {
    pub fn new(config: SafeholdRecoveryConfig) -> Option<Self> {
        if !config.is_valid() {
            return None;
        }
        let frame = config.initial_segment.frame();
        let navigation = GlobalNavigation {
            frame,
            position_q12: config.initial_position_q12,
            velocity_q24: config.initial_velocity_q24,
            attitude_q30: normalize_quaternion(config.initial_attitude_q30),
            covariance_proxy_q16: [1 << 13; 3],
            checksum: 0x811c_9dc5,
        };
        Some(Self {
            config,
            segment: config.initial_segment,
            epoch: 0,
            navigation,
            attitude_target_q30: normalize_quaternion(config.attitude_target_q30),
            last_time_q16: 0,
            last_fast: None,
            missing_fast: 0,
            safe: false,
            descending_count: 0,
            drogue_latched: false,
            main_latched: false,
            drogue_time_q16: 0,
            alarms: 0,
            sensor_checksum: 0x811c_9dc5,
            command_checksum: 0x811c_9dc5,
            flight_checksum: 0x811c_9dc5,
            transition_count: 0,
            last_mode: mode_for_segment(config.initial_segment),
            journal: EventJournal::new(),
        })
    }

    pub const fn segment(&self) -> SafeholdSessionSegment {
        self.segment
    }

    pub const fn transition_count(&self) -> u8 {
        self.transition_count
    }

    pub fn recover_journal_after(&self, sequence: u32, output: &mut [EventJournalRecord]) -> usize {
        self.journal.recover_after(sequence, output)
    }

    fn apply_transition(&mut self, cell: Option<GlobalTransitionCell>) {
        let Some(cell) = cell else { return };
        let next_segment = match (self.segment, cell.from, cell.to) {
            (
                SafeholdSessionSegment::EciCoast,
                GlobalFrameId::EarthInertialEciV1,
                GlobalFrameId::EarthFixedEcefV1,
            ) => Some(SafeholdSessionSegment::EcefEntry),
            (
                SafeholdSessionSegment::EcefEntry,
                GlobalFrameId::EarthFixedEcefV1,
                GlobalFrameId::LocalEnuV1,
            ) => Some(SafeholdSessionSegment::LocalRecovery),
            _ => None,
        };
        if cell.session != self.config.session
            || cell.source_epoch != self.epoch
            || cell.effective_epoch != self.epoch
            || cell.from != self.navigation.frame
            || next_segment.is_none()
        {
            self.enter_safe(GLOBAL_ALARM_FRAME);
            return;
        }
        let old_position = self.navigation.position_q12;
        let old_velocity = self.navigation.velocity_q24;
        let rotated_position = rotate_vector(cell.rotation_q30, old_position);
        let position = add_vector(rotated_position, cell.translation_q12);
        let velocity = if cell.from == GlobalFrameId::EarthInertialEciV1
            && cell.to == GlobalFrameId::EarthFixedEcefV1
        {
            rotate_vector(
                cell.rotation_q30,
                subtract_vector(
                    old_velocity,
                    cross_rate_position(cell.omega_q24, old_position),
                ),
            )
        } else {
            add_vector(
                rotate_vector(cell.rotation_q30, old_velocity),
                cell.velocity_bias_q24,
            )
        };
        let attitude = normalize_quaternion(quaternion_product(
            cell.rotation_q30,
            self.navigation.attitude_q30,
        ));
        let target = normalize_quaternion(quaternion_product(
            cell.rotation_q30,
            self.attitude_target_q30,
        ));
        let next_segment = next_segment.unwrap();
        self.navigation.position_q12 = position;
        self.navigation.velocity_q24 = velocity;
        self.navigation.attitude_q30 = attitude;
        self.navigation.frame = cell.to;
        self.navigation.checksum = hash_navigation(
            self.navigation.checksum,
            self.epoch,
            cell.to,
            &position,
            &velocity,
        );
        self.attitude_target_q30 = target;
        self.segment = next_segment;
        self.transition_count = self.transition_count.saturating_add(1);
        self.journal.append(
            u32::from(self.epoch),
            JournalEventKind::Mode,
            cell.from as u32,
            cell.to as u32,
            [cell.transform_identity as i32, 0, 0, 0],
        );
    }

    fn enter_safe(&mut self, alarm: u16) {
        let was_safe = self.safe;
        self.alarms |= alarm | GLOBAL_ALARM_SAFE;
        self.safe = true;
        if !was_safe {
            self.journal.append(
                u32::from(self.epoch),
                JournalEventKind::SafeState,
                u32::from(alarm),
                SAFEHOLD_RECOVERY_MANIFEST_ID,
                [0; 4],
            );
        }
    }

    fn accept_fast(&mut self, cell: Option<GlobalFastSensorCell>) -> Option<GlobalFastSensorCell> {
        let valid = cell.filter(|value| {
            value.session == self.config.session
                && value.measurement_epoch == self.epoch
                && value.production_epoch == self.epoch
                && value.frame == self.navigation.frame
                && value.validity
                    & (GLOBAL_FAST_DELTA_V | GLOBAL_FAST_DELTA_ANGLE | GLOBAL_FAST_ATTITUDE)
                    == (GLOBAL_FAST_DELTA_V | GLOBAL_FAST_DELTA_ANGLE | GLOBAL_FAST_ATTITUDE)
        });
        if let Some(value) = valid {
            self.missing_fast = 0;
            self.last_time_q16 = value.mission_time_q16;
            self.sensor_checksum = hash_fast(self.sensor_checksum, &value);
            for axis in 0..3 {
                self.navigation.velocity_q24[axis] = self.navigation.velocity_q24[axis]
                    .saturating_add(i32::from(value.delta_velocity_q24[axis]));
                self.navigation.position_q12[axis] = self.navigation.position_q12[axis]
                    .saturating_add(self.navigation.velocity_q24[axis] >> 17);
                self.navigation.covariance_proxy_q16[axis] =
                    self.navigation.covariance_proxy_q16[axis].saturating_add(16);
            }
            self.navigation.attitude_q30 = integrate_small_angle(
                self.navigation.attitude_q30,
                value.delta_angle_q24.map(i32::from),
            );
            self.navigation.checksum = hash_navigation(
                self.navigation.checksum,
                self.epoch,
                self.navigation.frame,
                &self.navigation.position_q12,
                &self.navigation.velocity_q24,
            );
            self.last_fast = Some(value);
            Some(value)
        } else {
            self.missing_fast = self.missing_fast.saturating_add(1);
            self.enter_safe(GLOBAL_ALARM_FAST_SENSOR | GLOBAL_ALARM_LINK);
            None
        }
    }

    fn accept_aid(&mut self, cell: Option<GlobalAidFrameCell>) -> Option<GlobalAidFrameCell> {
        let valid = cell.filter(|value| {
            value.session == self.config.session
                && value.measurement_epoch <= value.production_epoch
                && self.epoch.wrapping_sub(value.production_epoch) <= 32
                && value.frame == self.navigation.frame
        });
        if let Some(value) = valid {
            if value.validity & GLOBAL_AID_GNSS != 0
                && self.navigation.frame == GlobalFrameId::EarthFixedEcefV1
            {
                for axis in 0..3 {
                    self.navigation.position_q12[axis] = blend(
                        self.navigation.position_q12[axis],
                        value.gnss_position_q12_km[axis],
                        3,
                    );
                    self.navigation.velocity_q24[axis] = blend(
                        self.navigation.velocity_q24[axis],
                        value.gnss_velocity_q24_km_s[axis],
                        3,
                    );
                }
            }
            if value.validity & GLOBAL_AID_BAROMETER != 0
                && self.navigation.frame == GlobalFrameId::LocalEnuV1
            {
                self.navigation.position_q12[2] =
                    blend(self.navigation.position_q12[2], value.barometer_q12_km, 2);
            }
            if value.validity & GLOBAL_AID_ATTITUDE != 0 {
                self.navigation.attitude_q30 = normalize_quaternion(value.attitude_q30);
            }
            self.navigation.checksum = hash_navigation(
                self.navigation.checksum,
                self.epoch,
                self.navigation.frame,
                &self.navigation.position_q12,
                &self.navigation.velocity_q24,
            );
            Some(value)
        } else {
            if cell.is_some() {
                self.alarms |= GLOBAL_ALARM_AID;
            }
            None
        }
    }

    fn recovery_command(&mut self, aid: Option<&GlobalAidFrameCell>) -> u8 {
        if self.segment != SafeholdSessionSegment::LocalRecovery {
            self.descending_count = 0;
            return 0;
        }
        if self.navigation.velocity_q24[2] < 0 {
            self.descending_count = self.descending_count.saturating_add(1);
        } else {
            self.descending_count = 0;
        }
        let continuity = aid
            .filter(|value| value.validity & GLOBAL_AID_CONTINUITY != 0)
            .map(|value| value.continuity & 1 != 0)
            .unwrap_or(false);
        let feedback = aid
            .filter(|value| value.validity & GLOBAL_AID_DEPLOYMENT_FEEDBACK != 0)
            .map(|value| value.deployment_feedback)
            .unwrap_or(0);
        let subsonic = self
            .last_fast
            .map(|value| value.mach_q12 < SUBSONIC_MACH_Q12)
            .unwrap_or(false);
        let drogue_due = self.descending_count >= 2 && subsonic
            || self.last_time_q16 >= self.config.drogue_backup_time_q16;
        let mut discrete = 0;
        if !self.drogue_latched && drogue_due {
            if continuity {
                self.drogue_latched = true;
                self.drogue_time_q16 = self.last_time_q16;
                discrete |= GLOBAL_COMMAND_DROGUE;
                self.journal.append(
                    u32::from(self.epoch),
                    JournalEventKind::Mode,
                    u32::from(GLOBAL_COMMAND_DROGUE),
                    0,
                    [self.last_time_q16 as i32, 0, 0, 0],
                );
            } else {
                self.alarms |= GLOBAL_ALARM_RECOVERY;
            }
        }
        let separated = self.last_time_q16.saturating_sub(self.drogue_time_q16)
            >= self.config.minimum_deployment_separation_q16;
        let main_due = self.drogue_latched
            && separated
            && ((feedback & 1 != 0
                && self.navigation.velocity_q24[2] < 0
                && self.navigation.position_q12[2] <= self.config.main_altitude_q12_km)
                || self.last_time_q16 >= self.config.main_backup_time_q16);
        if !self.main_latched && main_due {
            if continuity {
                self.main_latched = true;
                discrete |= GLOBAL_COMMAND_MAIN;
                self.journal.append(
                    u32::from(self.epoch),
                    JournalEventKind::Mode,
                    u32::from(GLOBAL_COMMAND_MAIN),
                    0,
                    [self.last_time_q16 as i32, 0, 0, 0],
                );
            } else {
                self.alarms |= GLOBAL_ALARM_RECOVERY;
            }
        }
        discrete
    }

    fn control(&self, fast: Option<&GlobalFastSensorCell>) -> ([i32; 3], [u8; 12]) {
        if self.safe || self.segment == SafeholdSessionSegment::LocalRecovery {
            return ([0; 3], [0; 12]);
        }
        let Some(fast) = fast else {
            return ([0; 3], [0; 12]);
        };
        let error = attitude_error(self.navigation.attitude_q30, self.attitude_target_q30);
        let mut torque = [0; 3];
        let mut pulses = [0; 12];
        for axis in 0..3 {
            let proportional = ((error[axis + 1] >> 15)
                * i32::from(self.config.proportional_gain_q15[axis]))
                >> 15;
            let derivative = (-i32::from(fast.angular_rate_q15[axis])
                * i32::from(self.config.derivative_gain_q15[axis]))
                >> 15;
            let normalized = proportional
                .saturating_add(derivative)
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX));
            torque[axis] = ((i64::from(normalized) * i64::from(self.config.torque_limit_q12[axis]))
                >> 15) as i32;
            let magnitude = torque[axis].unsigned_abs();
            if magnitude != 0 {
                let authority = if axis == 0 { 1_638 } else { 29_900 };
                let quantum = ((magnitude * 8 + authority / 2) / authority).clamp(1, 8) as u8;
                let channel = axis * 4 + usize::from(torque[axis] < 0) * 2;
                pulses[channel] = quantum;
                pulses[channel + 1] = quantum;
            }
        }
        if fast.rcs_propellant_q21 <= 0 {
            pulses = [0; 12];
        }
        (torque, pulses)
    }

    fn mode(&self, aid: Option<&GlobalAidFrameCell>) -> GlobalFlightMode {
        if aid
            .map(|value| value.events & (1 << 9) != 0)
            .unwrap_or(false)
        {
            GlobalFlightMode::Complete
        } else if self.safe {
            GlobalFlightMode::Safe
        } else {
            mode_for_segment(self.segment)
        }
    }
}

impl GlobalKlr10FlightPackage for SafeholdRecoveryV1 {
    fn manifest(&self) -> FlightSoftwarePackageManifest {
        safehold_recovery_manifest()
    }

    fn process_release(
        &mut self,
        fast: Option<GlobalFastSensorCell>,
        aid: Option<GlobalAidFrameCell>,
        transition: Option<GlobalTransitionCell>,
    ) -> GlobalFlightEvidence {
        self.apply_transition(transition);
        let valid_fast = self.accept_fast(fast);
        let valid_aid = self.accept_aid(aid);
        let mut discrete = self.recovery_command(valid_aid.as_ref());
        let (torque, mut pulses) = self.control(valid_fast.as_ref());
        let mut flags = 0;
        if self.safe {
            discrete |= GLOBAL_COMMAND_SAFE;
            pulses = [0; 12];
        }
        if valid_fast
            .as_ref()
            .map(|value| value.rcs_propellant_q21 <= 0)
            .unwrap_or(false)
        {
            flags |= GLOBAL_COMMAND_RCS_RESERVED;
        }
        let mode = self.mode(valid_aid.as_ref());
        if mode != self.last_mode {
            self.journal.append(
                u32::from(self.epoch),
                JournalEventKind::Mode,
                self.last_mode as u32,
                mode as u32,
                [0; 4],
            );
            self.last_mode = mode;
        }
        let mut command = GlobalCommandCell {
            session: self.config.session,
            source_epoch: self.epoch,
            effective_epoch: self.epoch.wrapping_add(1),
            frame: self.navigation.frame,
            flags,
            discrete,
            gimbal_q15: [0; 2],
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
        let status = if self.epoch & 3 == 0 {
            Some(GlobalStatusCell {
                session: self.config.session,
                source_epoch: self.epoch,
                production_epoch: self.epoch,
                frame: self.navigation.frame,
                mode: mode as u8,
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
                deadline_misses: 0,
                transition_count: self.transition_count,
            })
        } else {
            None
        };
        self.epoch = self.epoch.wrapping_add(1);
        GlobalFlightEvidence {
            command,
            status,
            navigation: self.navigation,
            mode,
            safe: self.safe,
            armed: true,
            drogue_latched: self.drogue_latched,
            main_latched: self.main_latched,
            alarms: self.alarms,
            sensor_checksum: self.sensor_checksum,
            flight_checksum: self.flight_checksum,
            deadline_misses: 0,
        }
    }
}

const fn mode_for_segment(segment: SafeholdSessionSegment) -> GlobalFlightMode {
    match segment {
        SafeholdSessionSegment::EciCoast => GlobalFlightMode::Coast,
        SafeholdSessionSegment::EcefEntry => GlobalFlightMode::Entry,
        SafeholdSessionSegment::LocalRecovery => GlobalFlightMode::Recovery,
    }
}

fn blend(current: i32, measurement: i32, shift: u8) -> i32 {
    current.saturating_add(measurement.saturating_sub(current) >> shift)
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

fn normalize_quaternion(value: [i32; 4]) -> [i32; 4] {
    let mut status = NumericStatus::CLEAR;
    let magnitude = magnitude4_floor(value[0], value[1], value[2], value[3], &mut status);
    if !status.is_clear() || magnitude == 0 || magnitude > i32::MAX as u32 {
        return [Q30_ONE, 0, 0, 0];
    }
    let denominator = magnitude as i32;
    let normalized =
        value.map(|component| divide_scaled_truncating(component, denominator, 30, &mut status));
    if status.is_clear() {
        normalized
    } else {
        [Q30_ONE, 0, 0, 0]
    }
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
    extern crate std;
    use self::std::vec::Vec;
    use super::*;
    use ksa64_interface::phase10::{GLOBAL_AID_VALID_MASK, GLOBAL_FAST_VALID_MASK};
    use ksa64_interface::phase11::{parse_kfs11, write_kfs11, KFS11_LENGTH};

    fn compact_config() -> SafeholdRecoveryConfig {
        SafeholdRecoveryConfig {
            session: 42,
            initial_segment: SafeholdSessionSegment::EciCoast,
            initial_position_q12: [0, 0, 4_096],
            initial_velocity_q24: [0, 0, -(1 << 24)],
            initial_attitude_q30: [Q30_ONE, 0, 0, 0],
            attitude_target_q30: [Q30_ONE, 0, 0, 0],
            proportional_gain_q15: [6_144; 3],
            derivative_gain_q15: [24_576; 3],
            torque_limit_q12: [12_288; 3],
            drogue_backup_time_q16: 2_000_000,
            main_backup_time_q16: 3_000_000,
            main_altitude_q12_km: 8_192,
            minimum_deployment_separation_q16: 2_048,
        }
    }

    fn fast(epoch: u16, frame: GlobalFrameId) -> GlobalFastSensorCell {
        GlobalFastSensorCell {
            session: 42,
            measurement_epoch: epoch,
            production_epoch: epoch,
            frame,
            validity: GLOBAL_FAST_VALID_MASK,
            mission_time_q16: u32::from(epoch) * 2_048,
            delta_velocity_q24: [0; 3],
            delta_angle_q24: [0; 3],
            attitude_vector_q15: [0; 3],
            angular_rate_q15: [0; 3],
            dynamic_pressure_q10: 0,
            mach_q12: 0,
            gimbal_applied_q15: [0; 2],
            rcs_propellant_q21: 5 << 21,
            actuator_feedback: 0,
            vehicle_status: 0,
            sensor_checksum: epoch,
        }
    }

    fn aid(epoch: u16, frame: GlobalFrameId, feedback: u16) -> GlobalAidFrameCell {
        GlobalAidFrameCell {
            session: 42,
            measurement_epoch: epoch,
            production_epoch: epoch,
            frame,
            validity: GLOBAL_AID_VALID_MASK,
            mission_time_q16: u32::from(epoch) * 2_048,
            barometer_q12_km: 4_096,
            gnss_position_q12_km: [0, 0, 4_096],
            gnss_velocity_q24_km_s: [0, 0, -(1 << 24)],
            attitude_q30: [Q30_ONE, 0, 0, 0],
            frame_rotation_q30: [Q30_ONE, 0, 0, 0],
            frame_omega_q24: [0; 3],
            events: 0,
            continuity: 1,
            deployment_feedback: feedback,
        }
    }

    fn transition(epoch: u16, from: GlobalFrameId, to: GlobalFrameId) -> GlobalTransitionCell {
        GlobalTransitionCell {
            session: 42,
            source_epoch: epoch,
            effective_epoch: epoch,
            from,
            to,
            flags: 0,
            mission_time_q16: u32::from(epoch) * 2_048,
            transform_identity: 0x11f5_7000 + u32::from(epoch),
            rotation_q30: [Q30_ONE, 0, 0, 0],
            omega_q24: [0; 3],
            pre_position_q12: [0; 3],
            post_position_q12: [0; 3],
            pre_velocity_q24: [0; 3],
            post_velocity_q24: [0; 3],
            pre_attitude_q30: [Q30_ONE, 0, 0, 0],
            post_attitude_q30: [Q30_ONE, 0, 0, 0],
            pre_rate_q24: [0; 3],
            post_rate_q24: [0; 3],
            translation_q12: [0; 3],
            velocity_bias_q24: [0; 3],
            transition_checksum: epoch.into(),
        }
    }

    fn run_fixture() -> Vec<GlobalFlightEvidence> {
        let mut package = SafeholdRecoveryV1::new(compact_config()).unwrap();
        let mut evidence = Vec::new();
        for epoch in 0..16u16 {
            let (frame, change) = match epoch {
                8 => (
                    GlobalFrameId::EarthFixedEcefV1,
                    Some(transition(
                        epoch,
                        GlobalFrameId::EarthInertialEciV1,
                        GlobalFrameId::EarthFixedEcefV1,
                    )),
                ),
                12 => (
                    GlobalFrameId::LocalEnuV1,
                    Some(transition(
                        epoch,
                        GlobalFrameId::EarthFixedEcefV1,
                        GlobalFrameId::LocalEnuV1,
                    )),
                ),
                0..=7 => (GlobalFrameId::EarthInertialEciV1, None),
                9..=11 => (GlobalFrameId::EarthFixedEcefV1, None),
                _ => (GlobalFrameId::LocalEnuV1, None),
            };
            let feedback = u16::from(epoch >= 14);
            evidence.push(package.process_release(
                Some(fast(epoch, frame)),
                Some(aid(epoch, frame, feedback)),
                change,
            ));
        }
        evidence
    }

    #[test]
    fn manifest_round_trips_and_declares_only_bounded_segments() {
        let manifest = safehold_recovery_manifest();
        let mut bytes = [0; KFS11_LENGTH];
        write_kfs11(&manifest, &mut bytes).unwrap();
        assert_eq!(parse_kfs11(&bytes).unwrap(), manifest);
        assert_eq!(manifest.command_load_support, 0);
        assert_eq!(
            manifest.segment_support,
            PACKAGE_SEGMENT_ECI_COAST | PACKAGE_SEGMENT_ECEF_ENTRY | PACKAGE_SEGMENT_LOCAL_RECOVERY
        );
    }

    #[test]
    fn bounded_coast_entry_recovery_is_exact_and_deploys() {
        let first = run_fixture();
        let second = run_fixture();
        assert_eq!(first, second);
        assert_eq!(first[7].mode, GlobalFlightMode::Coast);
        assert_eq!(first[8].mode, GlobalFlightMode::Entry);
        assert_eq!(first[12].mode, GlobalFlightMode::Recovery);
        assert!(first
            .iter()
            .any(|value| value.command.discrete & GLOBAL_COMMAND_DROGUE != 0));
        assert!(first
            .iter()
            .any(|value| value.command.discrete & GLOBAL_COMMAND_MAIN != 0));
        assert!(!first.last().unwrap().safe);
    }

    #[test]
    fn wrong_transition_and_missing_fast_enter_immediate_safehold() {
        let mut package = SafeholdRecoveryV1::new(compact_config()).unwrap();
        let evidence = package.process_release(
            None,
            None,
            Some(transition(
                0,
                GlobalFrameId::EarthFixedEcefV1,
                GlobalFrameId::LocalEnuV1,
            )),
        );
        assert!(evidence.safe);
        assert_eq!(
            evidence.command.discrete & GLOBAL_COMMAND_SAFE,
            GLOBAL_COMMAND_SAFE
        );
        assert_eq!(evidence.command.rcs_pulse_quanta, [0; 12]);
    }

    #[test]
    fn unsupported_launch_or_ascent_session_is_rejected_by_configuration_contract() {
        for segment in [
            SafeholdSessionSegment::EciCoast,
            SafeholdSessionSegment::EcefEntry,
        ] {
            assert_ne!(segment.frame(), GlobalFrameId::LocalEnuV1);
        }
        assert_eq!(
            safehold_recovery_manifest().segment_support & super::PACKAGE_SEGMENT_ECI_COAST,
            super::PACKAGE_SEGMENT_ECI_COAST
        );
        assert_eq!(
            safehold_recovery_manifest().segment_support
                & ksa64_interface::phase11::PACKAGE_SEGMENT_LOCAL_LAUNCH,
            0
        );
        assert_eq!(
            safehold_recovery_manifest().segment_support
                & ksa64_interface::phase11::PACKAGE_SEGMENT_ECEF_ASCENT,
            0
        );
    }
}
