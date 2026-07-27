//! Phase 10 host-world/global-flight-computer composition.

#[cfg(test)]
use crate::phase10::EVENT_LANDING;
use crate::phase10::{
    FrameService, GlobalWorldError, GlobalWorldMachine, GlobalWorldSnapshot,
    TransitionServiceRecord,
};
use ksa64_core::numeric::{magnitude3_floor, NumericStatus};
use ksa64_core::phase10_contract::{EarthModelPack, TransformPack};
use ksa64_core::phase10_environment::CompiledAtmospherePack;
use ksa64_core::phase10_geodesy::launch_direction_enu;
use ksa64_core::phase10_numeric::{GlobalKinematicState, GLOBAL_AVIONICS_PERIOD_Q16};
use ksa64_core::phase10_vehicle::{GlobalMissionPack, GlobalVehiclePack};
#[cfg(test)]
use ksa64_flight::phase10::GlobalFlightMode;
use ksa64_flight::phase10::{GlobalFlightComputer, GlobalFlightConfig, GlobalFlightEvidence};
use ksa64_flight::phase11::GlobalKlr10FlightPackage;
use ksa64_interface::phase10::{
    GlobalAidFrameCell, GlobalCommandCell, GlobalFastSensorCell, GlobalFrameId,
    GlobalTransitionCell, GLOBAL_AID_ATTITUDE, GLOBAL_AID_BAROMETER, GLOBAL_AID_CONTINUITY,
    GLOBAL_AID_DEPLOYMENT_FEEDBACK, GLOBAL_AID_FRAME_SERVICE, GLOBAL_AID_GNSS,
    GLOBAL_FAST_ACTUATOR, GLOBAL_FAST_AIR_DATA, GLOBAL_FAST_ATTITUDE, GLOBAL_FAST_DELTA_ANGLE,
    GLOBAL_FAST_DELTA_V, GLOBAL_FAST_SUPPLY, KLR10_CONTRACT_ID,
};
use ksa64_interface::phase11::FlightAbiId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalSensorFaults {
    pub imu_delta_velocity_bias: [i16; 3],
    pub imu_delta_angle_bias: [i16; 3],
    pub barometer_bias_q12_km: i32,
    pub gnss_position_bias_q12_km: [i32; 3],
    pub gnss_velocity_bias_q24_km_s: [i32; 3],
    pub clock_drift_ppm: i16,
    pub fast_dropout_start: u16,
    pub fast_dropout_length: u8,
    pub gnss_dropout_start: u16,
    pub gnss_dropout_length: u8,
    /// Optional long-duration dropout window; zero/zero disables it.
    pub gnss_dropout_from_release: u16,
    pub gnss_dropout_until_release: u16,
}

impl GlobalSensorFaults {
    pub const NONE: Self = Self {
        imu_delta_velocity_bias: [0; 3],
        imu_delta_angle_bias: [0; 3],
        barometer_bias_q12_km: 0,
        gnss_position_bias_q12_km: [0; 3],
        gnss_velocity_bias_q24_km_s: [0; 3],
        clock_drift_ppm: 0,
        fast_dropout_start: 0,
        fast_dropout_length: 0,
        gnss_dropout_start: 0,
        gnss_dropout_length: 0,
        gnss_dropout_from_release: 0,
        gnss_dropout_until_release: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalAvionicsMissionSummary {
    pub terminal: GlobalWorldSnapshot,
    pub terminal_ecef: GlobalKinematicState,
    pub terminal_gcrf: GlobalKinematicState,
    pub flight: GlobalFlightEvidence,
    pub releases: u32,
    pub physical_steps: u32,
    pub transition_count: u8,
    pub max_dynamic_pressure_q14: i32,
    pub max_mach_q24: i32,
    pub max_acceleration_q28: i32,
    pub max_navigation_position_error_q12: i32,
    pub max_navigation_velocity_error_q24: i32,
    pub transition_records: [crate::phase10::FrameTransitionRecord; 4],
    pub sensor_checksum: u32,
    pub command_checksum: u32,
    pub placement_checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalReleaseBundle {
    pub fast: Option<GlobalFastSensorCell>,
    pub aid: Option<GlobalAidFrameCell>,
    pub transition: Option<GlobalTransitionCell>,
    pub evidence: GlobalFlightEvidence,
}

/// Processes one KLR10 release without gaining access to world truth.
///
/// The two public adapters below deliberately keep the frozen Phase 10 flight
/// computer and Phase 11 packages on the same sensor/command boundary.
pub trait GlobalFlightReleaseProcessor {
    fn process_release(
        &mut self,
        fast: Option<GlobalFastSensorCell>,
        aid: Option<GlobalAidFrameCell>,
        transition: Option<GlobalTransitionCell>,
    ) -> GlobalFlightEvidence;
}

pub struct Phase10GlobalFlightAdapter {
    inner: GlobalFlightComputer,
}

impl Phase10GlobalFlightAdapter {
    const fn new(inner: GlobalFlightComputer) -> Self {
        Self { inner }
    }

    pub const fn flight(&self) -> &GlobalFlightComputer {
        &self.inner
    }
}

impl GlobalFlightReleaseProcessor for Phase10GlobalFlightAdapter {
    fn process_release(
        &mut self,
        fast: Option<GlobalFastSensorCell>,
        aid: Option<GlobalAidFrameCell>,
        transition: Option<GlobalTransitionCell>,
    ) -> GlobalFlightEvidence {
        self.inner.tick(fast, aid, transition)
    }
}

pub struct GlobalKlr10PackageAdapter<P> {
    inner: P,
}

impl<P> GlobalKlr10PackageAdapter<P> {
    const fn new(inner: P) -> Self {
        Self { inner }
    }

    pub const fn package(&self) -> &P {
        &self.inner
    }

    pub fn package_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

impl<P: GlobalKlr10FlightPackage> GlobalFlightReleaseProcessor for GlobalKlr10PackageAdapter<P> {
    fn process_release(
        &mut self,
        fast: Option<GlobalFastSensorCell>,
        aid: Option<GlobalAidFrameCell>,
        transition: Option<GlobalTransitionCell>,
    ) -> GlobalFlightEvidence {
        self.inner.process_release(fast, aid, transition)
    }
}

pub type GlobalPackageAvionicsMission<'a, P> =
    GlobalAvionicsMission<'a, GlobalKlr10PackageAdapter<P>>;

pub struct GlobalAvionicsMission<'a, P: GlobalFlightReleaseProcessor = Phase10GlobalFlightAdapter> {
    world: GlobalWorldMachine<'a>,
    processor: P,
    vehicle: &'a GlobalVehiclePack,
    faults: GlobalSensorFaults,
    seed: u32,
    epoch: u16,
    releases: u32,
    physical_steps: u32,
    last_transition_count: u8,
    previous_active: GlobalKinematicState,
    held_command: GlobalCommandCell,
    last_flight: Option<GlobalFlightEvidence>,
    max_dynamic_pressure_q14: i32,
    max_mach_q24: i32,
    max_acceleration_q28: i32,
    max_navigation_position_error_q12: i32,
    max_navigation_velocity_error_q24: i32,
    sensor_checksum: u32,
    command_checksum: u32,
    placement_checksum: u32,
}

impl<'a> GlobalAvionicsMission<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        earth: &'a EarthModelPack,
        transforms: &'a TransformPack,
        atmosphere: &'a CompiledAtmospherePack,
        vehicle: &'a GlobalVehiclePack,
        mission: GlobalMissionPack,
        flight_config: GlobalFlightConfig,
        faults: GlobalSensorFaults,
        seed: u32,
    ) -> Result<Self, GlobalWorldError> {
        let flight = GlobalFlightComputer::new(flight_config).ok_or(GlobalWorldError::Identity)?;
        Self::with_processor(
            earth,
            transforms,
            atmosphere,
            vehicle,
            mission,
            flight_config,
            Phase10GlobalFlightAdapter::new(flight),
            faults,
            seed,
        )
    }

    pub const fn flight(&self) -> &GlobalFlightComputer {
        self.processor.flight()
    }
}

impl<'a, P: GlobalKlr10FlightPackage> GlobalAvionicsMission<'a, GlobalKlr10PackageAdapter<P>> {
    #[allow(clippy::too_many_arguments)]
    pub fn with_package(
        earth: &'a EarthModelPack,
        transforms: &'a TransformPack,
        atmosphere: &'a CompiledAtmospherePack,
        vehicle: &'a GlobalVehiclePack,
        mission: GlobalMissionPack,
        flight_config: GlobalFlightConfig,
        package: P,
        faults: GlobalSensorFaults,
        seed: u32,
    ) -> Result<Self, GlobalWorldError> {
        let manifest = package.manifest();
        if manifest.abi != FlightAbiId::GlobalKlr10V1
            || manifest.configuration_identity != KLR10_CONTRACT_ID
            || manifest.vehicle_profile_identity != 5
        {
            return Err(GlobalWorldError::Identity);
        }
        Self::with_processor(
            earth,
            transforms,
            atmosphere,
            vehicle,
            mission,
            flight_config,
            GlobalKlr10PackageAdapter::new(package),
            faults,
            seed,
        )
    }

    pub const fn package(&self) -> &P {
        self.processor.package()
    }

    pub fn package_mut(&mut self) -> &mut P {
        self.processor.package_mut()
    }
}

impl<'a, P: GlobalFlightReleaseProcessor> GlobalAvionicsMission<'a, P> {
    #[allow(clippy::too_many_arguments)]
    fn with_processor(
        earth: &'a EarthModelPack,
        transforms: &'a TransformPack,
        atmosphere: &'a CompiledAtmospherePack,
        vehicle: &'a GlobalVehiclePack,
        mission: GlobalMissionPack,
        flight_config: GlobalFlightConfig,
        processor: P,
        faults: GlobalSensorFaults,
        seed: u32,
    ) -> Result<Self, GlobalWorldError> {
        let world = GlobalWorldMachine::new(earth, transforms, atmosphere, vehicle, mission)?;
        let previous_active = world.active_state()?;
        if flight_config.initial_frame != GlobalFrameId::LocalEnuV1
            || flight_config.initial_position_q12
                != [
                    previous_active.position.x(),
                    previous_active.position.y(),
                    previous_active.position.z(),
                ]
        {
            return Err(GlobalWorldError::Identity);
        }
        let held_command = GlobalCommandCell {
            session: flight_config.session,
            source_epoch: u16::MAX,
            effective_epoch: 0,
            frame: GlobalFrameId::LocalEnuV1,
            flags: 0,
            discrete: 0,
            gimbal_q15: [0; 2],
            rcs_pulse_quanta: [0; 12],
            torque_demand_q12: [0; 3],
            status: 0,
            command_checksum: 0x811c_9dc5,
        };
        Ok(Self {
            world,
            processor,
            vehicle,
            faults,
            seed,
            epoch: 0,
            releases: 0,
            physical_steps: 0,
            last_transition_count: 0,
            previous_active,
            held_command,
            last_flight: None,
            max_dynamic_pressure_q14: 0,
            max_mach_q24: 0,
            max_acceleration_q28: 0,
            max_navigation_position_error_q12: 0,
            max_navigation_velocity_error_q24: 0,
            sensor_checksum: 0x811c_9dc5,
            command_checksum: 0x811c_9dc5,
            placement_checksum: 0x811c_9dc5,
        })
    }

    pub const fn world(&self) -> &GlobalWorldMachine<'a> {
        &self.world
    }

    pub fn release(&mut self) -> Result<GlobalFlightEvidence, GlobalWorldError> {
        Ok(self.release_bundle()?.evidence)
    }

    pub fn release_bundle(&mut self) -> Result<GlobalReleaseBundle, GlobalWorldError> {
        let snapshot = self.world.snapshot()?;
        let active = self.world.active_state()?;
        if active.time.raw() & (GLOBAL_AVIONICS_PERIOD_Q16 - 1) != 0 {
            return Err(GlobalWorldError::Transition);
        }
        let transition_service = if snapshot.transition_count > self.last_transition_count {
            Some(
                self.world
                    .transition_service(snapshot.transition_count as usize - 1)?,
            )
        } else {
            None
        };
        let transition = transition_service.map(|service| self.transition_cell(service));
        let fast = if in_window(
            self.epoch,
            self.faults.fast_dropout_start,
            self.faults.fast_dropout_length,
        ) {
            None
        } else {
            Some(self.fast_sensor(snapshot, active, transition_service)?)
        };
        let aid = Some(self.aid_frame(snapshot, active)?);
        if let Some(cell) = fast {
            self.sensor_checksum = hash_fast(self.sensor_checksum, &cell);
        }
        let evidence = self.processor.process_release(fast, aid, transition);
        self.command_checksum = evidence.command.command_checksum;
        self.placement_checksum = hash_placement(
            self.placement_checksum,
            self.epoch,
            self.sensor_checksum,
            self.command_checksum,
            evidence.navigation.checksum,
        );
        self.max_dynamic_pressure_q14 = self
            .max_dynamic_pressure_q14
            .max(snapshot.dynamic_pressure_q14_pa);
        self.max_mach_q24 = self.max_mach_q24.max(snapshot.mach_q24);
        let position_error =
            vector_difference(active_position(active), evidence.navigation.position_q12);
        let velocity_error =
            vector_difference(active_velocity(active), evidence.navigation.velocity_q24);
        self.max_navigation_position_error_q12 = self
            .max_navigation_position_error_q12
            .max(vector_magnitude(position_error)?);
        self.max_navigation_velocity_error_q24 = self
            .max_navigation_velocity_error_q24
            .max(vector_magnitude(velocity_error)?);
        if transition_service.is_none() {
            let delta_velocity = vector_difference(
                active_velocity(active),
                active_velocity(self.previous_active),
            );
            let acceleration_q28 = [
                delta_velocity[0].saturating_mul(512),
                delta_velocity[1].saturating_mul(512),
                delta_velocity[2].saturating_mul(512),
            ];
            self.max_acceleration_q28 = self
                .max_acceleration_q28
                .max(vector_magnitude(acceleration_q28)?);
        }
        self.held_command = evidence.command;
        self.last_flight = Some(evidence);
        self.previous_active = active;
        self.last_transition_count = snapshot.transition_count;
        self.epoch = self.epoch.wrapping_add(1);
        self.releases = self.releases.saturating_add(1);
        Ok(GlobalReleaseBundle {
            fast,
            aid,
            transition,
            evidence,
        })
    }

    pub fn advance_to_next_release(&mut self) -> Result<GlobalWorldSnapshot, GlobalWorldError> {
        let start = self.world.active_state()?.time.raw();
        let target = start
            .checked_add(GLOBAL_AVIONICS_PERIOD_Q16)
            .ok_or(GlobalWorldError::Numeric)?;
        let mut snapshot = self.world.snapshot()?;
        while !self.world.is_complete() && self.world.active_state()?.time.raw() < target {
            snapshot = self.world.step_commanded(&self.held_command)?;
            self.physical_steps = self.physical_steps.saturating_add(1);
        }
        if !self.world.is_complete() && self.world.active_state()?.time.raw() != target {
            return Err(GlobalWorldError::Transition);
        }
        Ok(snapshot)
    }

    pub fn completed_summary(&self) -> Result<GlobalAvionicsMissionSummary, GlobalWorldError> {
        if !self.world.is_complete() {
            return Err(GlobalWorldError::Transition);
        }
        self.summary(
            self.world.snapshot()?,
            self.last_flight.ok_or(GlobalWorldError::Transition)?,
        )
    }

    pub fn run(mut self) -> Result<GlobalAvionicsMissionSummary, GlobalWorldError> {
        loop {
            let evidence = self.release()?;
            if self.world.is_complete() {
                let terminal = self.world.snapshot()?;
                return self.summary(terminal, evidence);
            }
            let terminal = self.advance_to_next_release()?;
            if self.world.is_complete() {
                let final_evidence = self.release()?;
                return self.summary(terminal, final_evidence);
            }
            if self.releases > 460_800 {
                return Err(GlobalWorldError::Timeout);
            }
        }
    }

    fn summary(
        &self,
        terminal: GlobalWorldSnapshot,
        flight: GlobalFlightEvidence,
    ) -> Result<GlobalAvionicsMissionSummary, GlobalWorldError> {
        Ok(GlobalAvionicsMissionSummary {
            terminal,
            terminal_ecef: self.world.ecef_state_public()?,
            terminal_gcrf: self.world.gcrf_state_public()?,
            flight,
            releases: self.releases,
            physical_steps: self.physical_steps,
            transition_count: self.last_transition_count,
            max_dynamic_pressure_q14: self.max_dynamic_pressure_q14,
            max_mach_q24: self.max_mach_q24,
            max_acceleration_q28: self.max_acceleration_q28,
            max_navigation_position_error_q12: self.max_navigation_position_error_q12,
            max_navigation_velocity_error_q24: self.max_navigation_velocity_error_q24,
            transition_records: self.world.transition_records(),
            sensor_checksum: self.sensor_checksum,
            command_checksum: self.command_checksum,
            placement_checksum: self.placement_checksum,
        })
    }

    fn fast_sensor(
        &self,
        snapshot: GlobalWorldSnapshot,
        active: GlobalKinematicState,
        transition: Option<TransitionServiceRecord>,
    ) -> Result<GlobalFastSensorCell, GlobalWorldError> {
        let previous_velocity = if transition.is_some() {
            active.velocity
        } else {
            self.previous_active.velocity
        };
        let velocity = [
            active.velocity.x(),
            active.velocity.y(),
            active.velocity.z(),
        ];
        let previous = [
            previous_velocity.x(),
            previous_velocity.y(),
            previous_velocity.z(),
        ];
        let mut delta_velocity = [0; 3];
        let mut delta_angle = [0; 3];
        let rate = [
            active.angular_rate.x(),
            active.angular_rate.y(),
            active.angular_rate.z(),
        ];
        for axis in 0..3 {
            let noise = keyed_signed(self.seed, self.epoch, axis as u32, 3);
            delta_velocity[axis] = clamp_i16(
                velocity[axis]
                    .saturating_sub(previous[axis])
                    .saturating_add(i32::from(self.faults.imu_delta_velocity_bias[axis]))
                    .saturating_add(noise >> 13),
            );
            delta_angle[axis] = clamp_i16(
                (rate[axis] >> 5)
                    .saturating_add(i32::from(self.faults.imu_delta_angle_bias[axis]))
                    .saturating_add(noise >> 15),
            );
        }
        let mission_time = drifted_time(active.time.raw(), self.faults.clock_drift_ppm)?;
        Ok(GlobalFastSensorCell {
            session: self.held_command.session,
            measurement_epoch: self.epoch,
            production_epoch: self.epoch,
            frame: interface_frame(snapshot.frame),
            validity: GLOBAL_FAST_DELTA_V
                | GLOBAL_FAST_DELTA_ANGLE
                | GLOBAL_FAST_ATTITUDE
                | GLOBAL_FAST_AIR_DATA
                | GLOBAL_FAST_ACTUATOR
                | GLOBAL_FAST_SUPPLY,
            mission_time_q16: mission_time,
            delta_velocity_q24: delta_velocity,
            delta_angle_q24: delta_angle,
            attitude_vector_q15: [
                clamp_i16(active.attitude.x() >> 15),
                clamp_i16(active.attitude.y() >> 15),
                clamp_i16(active.attitude.z() >> 15),
            ],
            angular_rate_q15: [
                clamp_i16(active.angular_rate.x() >> 9),
                clamp_i16(active.angular_rate.y() >> 9),
                clamp_i16(active.angular_rate.z() >> 9),
            ],
            dynamic_pressure_q10: snapshot.dynamic_pressure_q14_pa >> 4,
            mach_q12: clamp_i16(snapshot.mach_q24 >> 12),
            gimbal_applied_q15: self.world.applied_gimbal_q15(),
            rcs_propellant_q21: snapshot.rcs_propellant_q21_kg,
            actuator_feedback: self.world.deployment_feedback(),
            vehicle_status: u16::from(active.time.raw() < self.vehicle.burn_time_q16_s) << 1,
            sensor_checksum: (self.sensor_checksum ^ self.sensor_checksum.rotate_right(16)) as u16,
        })
    }

    fn aid_frame(
        &self,
        snapshot: GlobalWorldSnapshot,
        active: GlobalKinematicState,
    ) -> Result<GlobalAidFrameCell, GlobalWorldError> {
        let cadence_8 = self.epoch & 3 == 0;
        let cadence_1 = self.epoch & 31 == 0;
        let in_long_gnss_dropout = self.faults.gnss_dropout_until_release
            > self.faults.gnss_dropout_from_release
            && self.epoch >= self.faults.gnss_dropout_from_release
            && self.epoch < self.faults.gnss_dropout_until_release;
        let gnss_available = cadence_1
            && !in_window(
                self.epoch,
                self.faults.gnss_dropout_start,
                self.faults.gnss_dropout_length,
            )
            && !in_long_gnss_dropout;
        let ecef = self.world.ecef_state_public()?;
        let service = self.world.frame_service()?;
        let mut validity = 0u8;
        if cadence_8 {
            validity |= GLOBAL_AID_BAROMETER
                | GLOBAL_AID_ATTITUDE
                | GLOBAL_AID_FRAME_SERVICE
                | GLOBAL_AID_CONTINUITY
                | GLOBAL_AID_DEPLOYMENT_FEEDBACK;
        }
        if gnss_available {
            validity |= GLOBAL_AID_GNSS;
        }
        let mut position = [ecef.position.x(), ecef.position.y(), ecef.position.z()];
        let mut velocity = [ecef.velocity.x(), ecef.velocity.y(), ecef.velocity.z()];
        if gnss_available {
            for axis in 0..3 {
                let noise = keyed_signed(self.seed, self.epoch, axis as u32, 7);
                position[axis] = position[axis]
                    .saturating_add(self.faults.gnss_position_bias_q12_km[axis])
                    .saturating_add(noise >> 14);
                velocity[axis] = velocity[axis]
                    .saturating_add(self.faults.gnss_velocity_bias_q24_km_s[axis])
                    .saturating_add(noise >> 6);
            }
        }
        Ok(GlobalAidFrameCell {
            session: self.held_command.session,
            measurement_epoch: self.epoch,
            production_epoch: self.epoch,
            frame: interface_frame(snapshot.frame),
            validity,
            mission_time_q16: drifted_time(active.time.raw(), self.faults.clock_drift_ppm)?,
            barometer_q12_km: snapshot
                .altitude_q12_km
                .saturating_add(self.faults.barometer_bias_q12_km),
            gnss_position_q12_km: position,
            gnss_velocity_q24_km_s: velocity,
            attitude_q30: quaternion_array(active),
            frame_rotation_q30: quaternion_array_from(service),
            frame_omega_q24: [
                service.omega_q24.x(),
                service.omega_q24.y(),
                service.omega_q24.z(),
            ],
            events: snapshot.events,
            continuity: 1,
            deployment_feedback: self.world.deployment_feedback(),
        })
    }

    fn transition_cell(&self, service: TransitionServiceRecord) -> GlobalTransitionCell {
        GlobalTransitionCell {
            session: self.held_command.session,
            source_epoch: self.epoch,
            effective_epoch: self.epoch,
            from: service.from,
            to: service.to,
            flags: 0,
            mission_time_q16: service.time.raw(),
            transform_identity: service.transform_identity,
            rotation_q30: quaternion_array_raw(service.rotation_q30),
            omega_q24: [
                service.omega_q24.x(),
                service.omega_q24.y(),
                service.omega_q24.z(),
            ],
            pre_position_q12: [
                service.before.position.x(),
                service.before.position.y(),
                service.before.position.z(),
            ],
            post_position_q12: [
                service.after.position.x(),
                service.after.position.y(),
                service.after.position.z(),
            ],
            pre_velocity_q24: [
                service.before.velocity.x(),
                service.before.velocity.y(),
                service.before.velocity.z(),
            ],
            post_velocity_q24: [
                service.after.velocity.x(),
                service.after.velocity.y(),
                service.after.velocity.z(),
            ],
            pre_attitude_q30: quaternion_array(service.before),
            post_attitude_q30: quaternion_array(service.after),
            pre_rate_q24: [
                service.before.angular_rate.x(),
                service.before.angular_rate.y(),
                service.before.angular_rate.z(),
            ],
            post_rate_q24: [
                service.after.angular_rate.x(),
                service.after.angular_rate.y(),
                service.after.angular_rate.z(),
            ],
            translation_q12: [
                service.translation_q12.x(),
                service.translation_q12.y(),
                service.translation_q12.z(),
            ],
            velocity_bias_q24: [
                service.velocity_bias_q24.x(),
                service.velocity_bias_q24.y(),
                service.velocity_bias_q24.z(),
            ],
            transition_checksum: service.checksum,
        }
    }
}

pub fn reference_global_flight_config(
    session: u16,
    initial: GlobalKinematicState,
    mission: GlobalMissionPack,
) -> Result<GlobalFlightConfig, GlobalWorldError> {
    let initial_attitude = quaternion_array(initial);
    let final_elevation = mission.pitch[mission.pitch_count as usize - 1].elevation_q28_rad;
    let final_direction = launch_direction_enu(mission.launch_azimuth_q28_rad, final_elevation)?;
    let final_attitude = ksa64_core::phase10_geodesy::body_x_attitude(final_direction)?;
    let final_attitude = [
        final_attitude.w(),
        final_attitude.x(),
        final_attitude.y(),
        final_attitude.z(),
    ];
    Ok(GlobalFlightConfig {
        session,
        initial_frame: GlobalFrameId::LocalEnuV1,
        initial_position_q12: [
            initial.position.x(),
            initial.position.y(),
            initial.position.z(),
        ],
        initial_attitude_q30: initial_attitude,
        launch_target_q30: initial_attitude,
        powered_target_q30: final_attitude,
        entry_target_q30: initial_attitude,
        pitch_program_end_q16: 60 << 16,
        proportional_gain_q15: [8_192; 3],
        derivative_gain_q15: [32_767; 3],
        torque_limit_q12: [4 << 12; 3],
        gimbal_limit_q15: 455,
        minimum_arming_time_q16: 60 << 16,
        drogue_backup_time_q16: 600 << 16,
        main_backup_time_q16: 900 << 16,
        main_altitude_q12_km: mission.main_deployment_altitude_q12_km,
        minimum_deployment_separation_q16: 2 << 16,
    })
}

fn interface_frame(frame: ksa64_core::phase10_contract::ReferenceFrameId) -> GlobalFrameId {
    match frame {
        ksa64_core::phase10_contract::ReferenceFrameId::LocalEnuV1 => GlobalFrameId::LocalEnuV1,
        ksa64_core::phase10_contract::ReferenceFrameId::EarthFixedEcefV1 => {
            GlobalFrameId::EarthFixedEcefV1
        }
        ksa64_core::phase10_contract::ReferenceFrameId::EarthInertialEciV1 => {
            GlobalFrameId::EarthInertialEciV1
        }
    }
}

fn quaternion_array(state: GlobalKinematicState) -> [i32; 4] {
    quaternion_array_raw(state.attitude)
}

fn quaternion_array_raw(value: ksa64_core::spatial_numeric::QuaternionQ30) -> [i32; 4] {
    [value.w(), value.x(), value.y(), value.z()]
}

fn quaternion_array_from(service: FrameService) -> [i32; 4] {
    quaternion_array_raw(service.rotation_q30)
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn active_position(state: GlobalKinematicState) -> [i32; 3] {
    [state.position.x(), state.position.y(), state.position.z()]
}

fn active_velocity(state: GlobalKinematicState) -> [i32; 3] {
    [state.velocity.x(), state.velocity.y(), state.velocity.z()]
}
fn vector_difference(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [
        a[0].saturating_sub(b[0]),
        a[1].saturating_sub(b[1]),
        a[2].saturating_sub(b[2]),
    ]
}

fn vector_magnitude(value: [i32; 3]) -> Result<i32, GlobalWorldError> {
    let mut status = NumericStatus::CLEAR;
    let magnitude = magnitude3_floor(value[0], value[1], value[2], &mut status);
    if !status.is_clear() || magnitude > i32::MAX as u32 {
        return Err(GlobalWorldError::Numeric);
    }
    Ok(magnitude as i32)
}
fn in_window(epoch: u16, start: u16, length: u8) -> bool {
    length != 0 && epoch.wrapping_sub(start) < u16::from(length)
}

fn drifted_time(time_q16: u32, ppm: i16) -> Result<u32, GlobalWorldError> {
    let correction = (i64::from(time_q16) * i64::from(ppm)) / 1_000_000;
    let result = i64::from(time_q16) + correction;
    u32::try_from(result).map_err(|_| GlobalWorldError::Numeric)
}

fn keyed_signed(seed: u32, epoch: u16, axis: u32, stream: u32) -> i32 {
    let mut value = seed
        ^ u32::from(epoch).wrapping_mul(0x9e37_79b9)
        ^ axis.wrapping_mul(0x85eb_ca6b)
        ^ stream.wrapping_mul(0xc2b2_ae35);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    ((value ^ (value >> 16)) & 0xffff) as i32 - 32_768
}

fn hash_word(mut checksum: u32, value: u32) -> u32 {
    for byte in value.to_le_bytes() {
        checksum ^= u32::from(byte);
        checksum = checksum.wrapping_mul(0x0100_0193);
    }
    checksum
}

fn hash_fast(mut checksum: u32, cell: &GlobalFastSensorCell) -> u32 {
    checksum = hash_word(checksum, u32::from(cell.measurement_epoch));
    checksum = hash_word(checksum, cell.mission_time_q16);
    checksum = hash_word(checksum, cell.frame as u32);
    for value in cell.delta_velocity_q24 {
        checksum = hash_word(checksum, value as u16 as u32);
    }
    checksum
}

fn hash_placement(
    mut checksum: u32,
    epoch: u16,
    sensor: u32,
    command: u32,
    navigation: u32,
) -> u32 {
    checksum = hash_word(checksum, u32::from(epoch));
    checksum = hash_word(checksum, sensor);
    checksum = hash_word(checksum, command);
    hash_word(checksum, navigation)
}

#[cfg(test)]
mod tests {
    use super::*;

    use ksa64_core::phase10_contract::{EarthModelPack, TransformPack};
    use ksa64_core::phase10_environment::CompiledAtmospherePack;
    use ksa64_core::phase10_vehicle::{GlobalMissionPack, GlobalVehiclePack};
    use ksa64_flight::phase11::{ksa_g10r_reference_mission_plan, KsaG10rReferenceOpsV1};

    fn fixture() -> (
        EarthModelPack,
        TransformPack,
        CompiledAtmospherePack,
        GlobalVehiclePack,
        GlobalMissionPack,
    ) {
        (
            EarthModelPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kem10"))
                .unwrap(),
            TransformPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kft10"))
                .unwrap(),
            CompiledAtmospherePack::decode(include_bytes!(
                "../../phase10/generated/ksa-g10r.kat10"
            ))
            .unwrap(),
            GlobalVehiclePack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgv10"))
                .unwrap(),
            GlobalMissionPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgm10"))
                .unwrap(),
        )
    }

    #[test]
    fn nominal_truth_blind_flight_crosses_frames_and_recovers() {
        let (earth, transforms, atmosphere, vehicle, mission) = fixture();
        let world =
            GlobalWorldMachine::new(&earth, &transforms, &atmosphere, &vehicle, mission).unwrap();
        let initial = world.active_state().unwrap();
        let config = reference_global_flight_config(10, initial, mission).unwrap();
        let runner = GlobalAvionicsMission::new(
            &earth,
            &transforms,
            &atmosphere,
            &vehicle,
            mission,
            config,
            GlobalSensorFaults::NONE,
            0x4b53_41a0,
        )
        .unwrap();
        let summary = runner.run().unwrap();
        assert_eq!(summary.transition_count, 4);
        assert_ne!(summary.placement_checksum, 0);
        assert_eq!(summary.terminal.events & EVENT_LANDING, EVENT_LANDING);
        assert!(summary.flight.drogue_latched);
        assert!(summary.flight.main_latched);
        assert!(!summary.flight.safe);
        assert_eq!(summary.flight.mode, GlobalFlightMode::Complete);
    }

    #[test]
    fn deterministic_sensor_faults_do_not_depend_on_repetition() {
        let run = || {
            let (earth, transforms, atmosphere, vehicle, mission) = fixture();
            let world =
                GlobalWorldMachine::new(&earth, &transforms, &atmosphere, &vehicle, mission)
                    .unwrap();
            let config =
                reference_global_flight_config(10, world.active_state().unwrap(), mission).unwrap();
            GlobalAvionicsMission::new(
                &earth,
                &transforms,
                &atmosphere,
                &vehicle,
                mission,
                config,
                GlobalSensorFaults {
                    imu_delta_velocity_bias: [1, -1, 2],
                    gnss_dropout_start: 32,
                    gnss_dropout_length: 16,
                    ..GlobalSensorFaults::NONE
                },
                0x4b53_41a0,
            )
            .unwrap()
            .run()
            .unwrap()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn inactive_reference_package_matches_frozen_phase10_releases_exactly() {
        let (earth, transforms, atmosphere, vehicle, mission) = fixture();
        let initial = GlobalWorldMachine::new(&earth, &transforms, &atmosphere, &vehicle, mission)
            .unwrap()
            .active_state()
            .unwrap();
        let config = reference_global_flight_config(10, initial, mission).unwrap();
        let mut frozen = GlobalAvionicsMission::new(
            &earth,
            &transforms,
            &atmosphere,
            &vehicle,
            mission,
            config,
            GlobalSensorFaults::NONE,
            0x4b53_41a0,
        )
        .unwrap();
        let package = KsaG10rReferenceOpsV1::new(config).unwrap();
        let mut packaged = GlobalPackageAvionicsMission::with_package(
            &earth,
            &transforms,
            &atmosphere,
            &vehicle,
            mission,
            config,
            package,
            GlobalSensorFaults::NONE,
            0x4b53_41a0,
        )
        .unwrap();

        for _ in 0..64 {
            assert_eq!(
                frozen.release_bundle().unwrap(),
                packaged.release_bundle().unwrap()
            );
            assert_eq!(frozen.world().snapshot(), packaged.world().snapshot());
            assert_eq!(
                frozen.advance_to_next_release().unwrap(),
                packaged.advance_to_next_release().unwrap()
            );
        }
    }

    #[test]
    #[ignore = "full Phase 10 mission compatibility acceptance"]
    fn inactive_reference_package_matches_frozen_phase10_full_mission_exactly() {
        let (earth, transforms, atmosphere, vehicle, mission) = fixture();
        let initial = GlobalWorldMachine::new(&earth, &transforms, &atmosphere, &vehicle, mission)
            .unwrap()
            .active_state()
            .unwrap();
        let config = reference_global_flight_config(10, initial, mission).unwrap();
        let mut frozen = GlobalAvionicsMission::new(
            &earth,
            &transforms,
            &atmosphere,
            &vehicle,
            mission,
            config,
            GlobalSensorFaults::NONE,
            0x4b53_41a0,
        )
        .unwrap();
        let package = KsaG10rReferenceOpsV1::new(config).unwrap();
        let mut packaged = GlobalPackageAvionicsMission::with_package(
            &earth,
            &transforms,
            &atmosphere,
            &vehicle,
            mission,
            config,
            package,
            GlobalSensorFaults::NONE,
            0x4b53_41a0,
        )
        .unwrap();

        loop {
            assert_eq!(
                frozen.release_bundle().unwrap(),
                packaged.release_bundle().unwrap()
            );
            assert_eq!(frozen.world().snapshot(), packaged.world().snapshot());
            if frozen.world().is_complete() {
                assert!(packaged.world().is_complete());
                break;
            }
            assert_eq!(
                frozen.advance_to_next_release().unwrap(),
                packaged.advance_to_next_release().unwrap()
            );
        }
        assert_eq!(
            frozen.completed_summary().unwrap(),
            packaged.completed_summary().unwrap()
        );
    }

    #[test]
    fn initialized_reference_ops_package_processes_world_generated_cells() {
        let (earth, transforms, atmosphere, vehicle, mission) = fixture();
        let initial = GlobalWorldMachine::new(&earth, &transforms, &atmosphere, &vehicle, mission)
            .unwrap()
            .active_state()
            .unwrap();
        let config = reference_global_flight_config(10, initial, mission).unwrap();
        let mut package = KsaG10rReferenceOpsV1::new(config).unwrap();
        assert!(package.initialize_mission_plan(ksa_g10r_reference_mission_plan()));
        let mut runner = GlobalPackageAvionicsMission::with_package(
            &earth,
            &transforms,
            &atmosphere,
            &vehicle,
            mission,
            config,
            package,
            GlobalSensorFaults::NONE,
            0x4b53_41a0,
        )
        .unwrap();

        let first = runner.release_bundle().unwrap();
        let fast = first.fast.expect("the nominal world produces a fast cell");
        let aid = first.aid.expect("the nominal world produces an aid cell");
        assert_eq!(fast.session, config.session);
        assert_eq!(fast.measurement_epoch, 0);
        assert_eq!(aid.session, config.session);
        assert_eq!(aid.measurement_epoch, 0);
        assert_eq!(first.evidence.command.source_epoch, 0);
        assert!(runner.package().prediction_summary().is_some());

        runner.advance_to_next_release().unwrap();
        let second = runner.release_bundle().unwrap();
        assert_eq!(second.fast.unwrap().measurement_epoch, 1);
        assert_eq!(second.evidence.command.source_epoch, 1);
        assert_ne!(
            first.evidence.command.command_checksum,
            second.evidence.command.command_checksum
        );
    }
}
