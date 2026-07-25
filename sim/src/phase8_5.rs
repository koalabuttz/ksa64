//! Phase 8.5 exact local world endpoint and in-memory loopback.

use ksa64_core::phase8_5_clock::{ExactClockError, ExactEventClock};
use ksa64_core::phase8_5_contract::{
    ActuatorCapabilityId, ActuatorCapabilityPack, AvionicsEvaluationSummary, AvionicsProfileId,
    AvionicsProfilePack,
};
use ksa64_core::phase8_mission::{
    HobbySpatialPhase, Phase85AppliedControl, Phase85DeploymentCommand, Phase8MissionError,
    Phase8MissionMachine, Phase8MissionResult, Phase8MissionSnapshot, SpatialMissionVariation,
    EVENT_DROGUE, EVENT_MAIN,
};
use ksa64_core::phase8_numeric::{
    SpatialTime, SPATIAL_COAST_TRANSLATION_STEP, SPATIAL_POWERED_STEP, SPATIAL_RECOVERY_STEP,
};
use ksa64_core::phase8_pack::{
    SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindProfilePack,
};
use ksa64_core::phase8_result::spatial_evaluation_identity;
use ksa64_flight::phase8_5::{
    LocalControlCapability, LocalFlightComputer, LocalFlightConfig, LocalFlightEvidence,
};
use ksa64_interface::phase8_5::{
    write_local_aid, write_local_command, write_local_inertial, write_local_status, LocalAidCell,
    LocalCommandCell, LocalInertialCell, LOCAL_AID_ATTITUDE, LOCAL_AID_BAROMETER,
    LOCAL_AID_CONTINUITY, LOCAL_AID_DEPLOYMENT_FEEDBACK, LOCAL_AID_GPS, LOCAL_INERTIAL_VALID_MASK,
};

pub const LOCAL_SESSION: u16 = 0x8501;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalWorldError {
    Mission(Phase8MissionError),
    Clock(ExactClockError),
    Epoch,
    Capability,
    Incomplete,
}
impl From<Phase8MissionError> for LocalWorldError {
    fn from(v: Phase8MissionError) -> Self {
        Self::Mission(v)
    }
}
impl From<ExactClockError> for LocalWorldError {
    fn from(v: ExactClockError) -> Self {
        Self::Clock(v)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalAvionicsVariation {
    pub seed: u32,
    pub imu_delta_bias: [i16; 3],
    pub gyro_bias: [i16; 3],
    pub barometer_bias_q13: i32,
    pub gps_position_bias_q13: [i32; 3],
    pub gps_velocity_bias_q19: [i32; 3],
    pub sensor_noise_scale: u16,
    pub aid_delay_epochs: u8,
    pub imu_dropout_start: u16,
    pub imu_dropout_epochs: u8,
    pub barometer_dropout_start: u16,
    pub barometer_dropout_epochs: u8,
    pub gps_dropout_start: u16,
    pub gps_dropout_epochs: u8,
    pub link_dropout_start: u16,
    pub link_dropout_epochs: u8,
    pub clock_drift_ppm: i16,
    pub continuity_open: bool,
    pub feedback_fail_mask: u16,
    pub gimbal_jam_epoch: u16,
    pub missed_deadline_epoch: u16,
    pub deployment_actuation_delay_epochs: u8,
}
impl LocalAvionicsVariation {
    pub const NOMINAL: Self = Self {
        seed: 0x4b53_4185,
        imu_delta_bias: [0; 3],
        gyro_bias: [0; 3],
        barometer_bias_q13: 0,
        gps_position_bias_q13: [0; 3],
        gps_velocity_bias_q19: [0; 3],
        sensor_noise_scale: 0,
        aid_delay_epochs: 0,
        imu_dropout_start: 0,
        imu_dropout_epochs: 0,
        barometer_dropout_start: 0,
        barometer_dropout_epochs: 0,
        gps_dropout_start: 0,
        gps_dropout_epochs: 0,
        link_dropout_start: 0,
        link_dropout_epochs: 0,
        clock_drift_ppm: 0,
        continuity_open: false,
        feedback_fail_mask: 0,
        gimbal_jam_epoch: u16::MAX,
        missed_deadline_epoch: u16::MAX,
        deployment_actuation_delay_epochs: 0,
    };
    pub const fn is_valid(self) -> bool {
        self.aid_delay_epochs <= 32
            && self.sensor_noise_scale <= 4096
            && self.clock_drift_ppm >= -10_000
            && self.clock_drift_ppm <= 10_000
            && self.deployment_actuation_delay_epochs <= 32
    }
}
fn epoch_in_window(epoch: u16, start: u16, count: u8) -> bool {
    count != 0 && epoch.wrapping_sub(start) < u16::from(count)
}
fn keyed_noise(seed: u32, epoch: u16, sensor: u8, axis: u8, amplitude: i32) -> i32 {
    if amplitude == 0 {
        return 0;
    }
    let mut value = seed
        ^ (u32::from(epoch).wrapping_mul(0x9e37_79b9))
        ^ (u32::from(sensor) << 16)
        ^ u32::from(axis);
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    let span = amplitude.saturating_mul(2).saturating_add(1).max(1) as u32;
    (value % span) as i32 - amplitude
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalDirectorSample {
    pub epoch: u16,
    pub snapshot: Phase8MissionSnapshot,
    pub truth_checksum: u32,
    pub applied_gimbal: [i16; 2],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalWorldRelease {
    pub inertial: LocalInertialCell,
    pub aid: Option<LocalAidCell>,
    pub director: LocalDirectorSample,
    pub complete: bool,
}

#[derive(Clone, Copy)]
struct GimbalActuator {
    capability: ActuatorCapabilityId,
    limit: i16,
    slew_per_release: i16,
    lag_releases: usize,
    queue: [[i16; 2]; 8],
    applied: [i16; 2],
}
impl GimbalActuator {
    fn new(pack: ActuatorCapabilityPack) -> Result<Self, LocalWorldError> {
        let limit = degrees_q16_to_turn16(pack.gimbal_limit_q16_deg);
        let slew = degrees_q16_to_turn16(pack.slew_q16_deg_per_s) / 32;
        Ok(Self {
            capability: pack.capability,
            limit,
            slew_per_release: slew.max(1),
            lag_releases: pack.lag_releases as usize,
            queue: [[0; 2]; 8],
            applied: [0; 2],
        })
    }
    fn release(&mut self, command: [i16; 2], powered: bool) {
        if self.capability == ActuatorCapabilityId::MonitorOnlyV1 || !powered {
            self.queue = [[0; 2]; 8];
            self.applied = [0; 2];
            return;
        }
        let requested = [
            command[0].clamp(-self.limit, self.limit),
            command[1].clamp(-self.limit, self.limit),
        ];
        let lagged = if self.lag_releases == 0 {
            requested
        } else {
            let lagged = self.queue[0];
            let mut index = 1;
            while index < self.lag_releases {
                self.queue[index - 1] = self.queue[index];
                index += 1;
            }
            self.queue[self.lag_releases - 1] = requested;
            lagged
        };
        let mut a = 0;
        while a < 2 {
            let delta = lagged[a]
                .saturating_sub(self.applied[a])
                .clamp(-self.slew_per_release, self.slew_per_release);
            self.applied[a] = self.applied[a].saturating_add(delta);
            a += 1
        }
    }
}
fn degrees_q16_to_turn16(value: i32) -> i16 {
    ((value as i64 * 65536) / (360i64 * 65536)).clamp(i16::MIN as i64, i16::MAX as i64) as i16
}
fn clamp_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}
fn phase_maximum(phase: HobbySpatialPhase) -> Result<SpatialTime, LocalWorldError> {
    match phase {
        HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight => {
            Ok(SPATIAL_POWERED_STEP)
        }
        HobbySpatialPhase::Coast => Ok(SPATIAL_COAST_TRANSLATION_STEP),
        HobbySpatialPhase::DrogueRecovery | HobbySpatialPhase::MainRecovery => {
            Ok(SPATIAL_RECOVERY_STEP)
        }
        HobbySpatialPhase::Complete | HobbySpatialPhase::Failed => Err(LocalWorldError::Incomplete),
    }
}

pub struct LocalWorldEndpoint<'a> {
    machine: Phase8MissionMachine<'a>,
    motor: &'a SpatialMotorPack,
    clock: ExactEventClock,
    physical_deadline: SpatialTime,
    max_mission_time: SpatialTime,
    last_release_state: ksa64_core::phase8_world::HobbySpatialState,
    last_release_epoch: Option<u16>,
    command: LocalCommandCell,
    pending_deployment: Phase85DeploymentCommand,
    actuator: GimbalActuator,
    pivot_from_nose_q28: i32,
    deployment_feedback: u16,
}
impl<'a> LocalWorldEndpoint<'a> {
    pub fn new(
        vehicle: &'a SpatialVehiclePack,
        motor: &'a SpatialMotorPack,
        mission: SpatialMissionPack,
        wind: &'a WindProfilePack,
        variation: SpatialMissionVariation,
        capability: ActuatorCapabilityPack,
    ) -> Result<Self, LocalWorldError> {
        if capability.vehicle_identity != vehicle.identity {
            return Err(LocalWorldError::Capability);
        }
        let machine =
            Phase8MissionMachine::new_with_variation(vehicle, motor, mission, wind, variation)?;
        let state = machine.snapshot().state;
        let first = phase_maximum(machine.snapshot().phase)?;
        Ok(Self {
            machine,
            motor,
            clock: ExactEventClock::new(),
            physical_deadline: SpatialTime::from_raw(
                state
                    .time
                    .raw()
                    .saturating_add(first.raw())
                    .min(mission.max_mission_time.raw()),
            ),
            max_mission_time: mission.max_mission_time,
            last_release_state: state,
            last_release_epoch: None,
            command: LocalCommandCell {
                session: LOCAL_SESSION,
                source_epoch: 0,
                effective_epoch: 1,
                flags: 0,
                discrete: 0,
                gimbal: [0; 2],
                control_demand: [0; 2],
                status: 0,
            },
            pending_deployment: Phase85DeploymentCommand::default(),
            actuator: GimbalActuator::new(capability)?,
            pivot_from_nose_q28: capability.pivot_from_nose_q28,
            deployment_feedback: 0,
        })
    }
    pub const fn is_complete(&self) -> bool {
        self.machine.is_complete()
    }
    pub const fn snapshot(&self) -> Phase8MissionSnapshot {
        self.machine.snapshot()
    }
    pub fn result(&self) -> Option<Phase8MissionResult> {
        self.machine.result()
    }
    pub fn release(&mut self) -> Result<Option<LocalWorldRelease>, LocalWorldError> {
        while !self.clock.release_due(self.machine.snapshot().state.time) {
            let now = self.machine.snapshot().state.time;
            let segment = self.clock.next_segment(now, self.physical_deadline)?;
            let phase_before = self.machine.snapshot().phase;
            let applied = if phase_before == HobbySpatialPhase::PoweredFlight {
                Phase85AppliedControl {
                    gimbal_turn16: self.actuator.applied,
                    pivot_from_nose_q28: self.pivot_from_nose_q28,
                }
            } else {
                Phase85AppliedControl::NEUTRAL
            };
            let deployment = self.pending_deployment;
            self.pending_deployment = Phase85DeploymentCommand::default();
            if let Err(error) = self
                .machine
                .step_avionics(segment.duration(), applied, deployment)
            {
                if self.machine.is_complete() {
                    return Ok(None);
                }
                return Err(LocalWorldError::Mission(error));
            }
            let snapshot = self.machine.snapshot();
            if snapshot.events & EVENT_DROGUE != 0 {
                self.deployment_feedback |= 1
            }
            if snapshot.events & EVENT_MAIN != 0 {
                self.deployment_feedback |= 2
            }
            if segment.ends_at_physical_deadline || snapshot.phase != phase_before {
                if snapshot.phase == HobbySpatialPhase::Complete {
                    break;
                }
                let maximum = phase_maximum(snapshot.phase)?;
                let mut deadline = snapshot.state.time.raw().saturating_add(maximum.raw());
                if matches!(
                    snapshot.phase,
                    HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight
                ) && snapshot.state.time.raw() < self.motor.burn_time.raw()
                {
                    deadline = deadline.min(self.motor.burn_time.raw())
                }
                self.physical_deadline =
                    SpatialTime::from_raw(deadline.min(self.max_mission_time.raw()))
            }
        }
        let state = self.machine.snapshot().state;
        if !self.clock.release_due(state.time) {
            // A terminal physical event may occur between avionics releases.
            // Completion is a physical boundary, not a synthetic sensor epoch.
            if self.machine.is_complete() {
                return Ok(None);
            }
            return Err(LocalWorldError::Clock(ExactClockError::TimeMismatch));
        }
        let epoch_u32 = self.clock.consume_release(state.time)?;
        let epoch = u16::try_from(epoch_u32).map_err(|_| LocalWorldError::Epoch)?;
        let dt_raw = if let Some(last) = self.last_release_epoch {
            (epoch.wrapping_sub(last) as i32) * 8192
        } else {
            8192
        };
        let gravity_delta_q19 = ((5_141_509i64 * dt_raw as i64) >> 18) as i32;
        let mut delta = [0i16; 3];
        let mut a = 0;
        while a < 3 {
            let mut dv = state
                .velocity
                .component(a)
                .saturating_sub(self.last_release_state.velocity.component(a));
            if a == 2 {
                dv = dv.saturating_add(gravity_delta_q19)
            }
            delta[a] = clamp_i16(dv >> 7);
            a += 1
        }
        let q = state.attitude;
        let phase = self.machine.snapshot().phase;
        let powered = matches!(
            phase,
            HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight
        ) && state.time.raw() < self.motor.burn_time.raw();
        let vehicle_status = u16::from(phase == HobbySpatialPhase::ConstrainedPowered)
            | if powered { 2 } else { 0 }
            | if matches!(
                phase,
                HobbySpatialPhase::DrogueRecovery | HobbySpatialPhase::MainRecovery
            ) {
                4
            } else {
                0
            }
            | if phase == HobbySpatialPhase::Complete {
                8
            } else {
                0
            };
        let inertial = LocalInertialCell {
            session: LOCAL_SESSION,
            measurement_epoch: epoch,
            production_epoch: epoch,
            validity: LOCAL_INERTIAL_VALID_MASK,
            flags: 0,
            platform_angle: [
                clamp_i16(q.x() >> 15),
                clamp_i16(q.y() >> 15),
                clamp_i16(q.z() >> 15),
            ],
            angular_rate: [
                clamp_i16(state.angular_rate.x() >> 12),
                clamp_i16(state.angular_rate.y() >> 12),
                clamp_i16(state.angular_rate.z() >> 12),
            ],
            delta_velocity: delta,
            gimbal_applied: self.actuator.applied,
            vehicle_status,
            actuator_feedback: self.deployment_feedback,
        };
        let aid = if epoch & 3 == 0 {
            Some(LocalAidCell {
                session: LOCAL_SESSION,
                measurement_epoch: epoch,
                production_epoch: epoch,
                validity: LOCAL_AID_BAROMETER
                    | if epoch & 31 == 0 { LOCAL_AID_GPS } else { 0 }
                    | LOCAL_AID_ATTITUDE
                    | LOCAL_AID_CONTINUITY
                    | LOCAL_AID_DEPLOYMENT_FEEDBACK,
                events: self.machine.snapshot().events,
                onboard_time_q18: state.time.raw(),
                barometer_q13: state.position.z(),
                gps_position_q13: [state.position.x(), state.position.y(), state.position.z()],
                gps_velocity_q19: [state.velocity.x(), state.velocity.y(), state.velocity.z()],
                attitude_vector: inertial.platform_angle,
                continuity: 1,
                deployment_feedback: self.deployment_feedback,
                vehicle_status: u32::from(vehicle_status),
                clock_flags: 0,
            })
        } else {
            None
        };
        self.last_release_state = state;
        self.last_release_epoch = Some(epoch);
        Ok(Some(LocalWorldRelease {
            inertial,
            aid,
            director: LocalDirectorSample {
                epoch,
                snapshot: self.machine.snapshot(),
                truth_checksum: self.machine.trace_checksum(),
                applied_gimbal: self.actuator.applied,
            },
            complete: self.machine.is_complete(),
        }))
    }
    pub fn accept_command(&mut self, cell: LocalCommandCell) -> Result<(), LocalWorldError> {
        self.accept_command_faulted(cell, false)
    }
    pub fn accept_command_faulted(
        &mut self,
        cell: LocalCommandCell,
        gimbal_jammed: bool,
    ) -> Result<(), LocalWorldError> {
        let source = self.last_release_epoch.ok_or(LocalWorldError::Epoch)?;
        if cell.session != LOCAL_SESSION
            || cell.source_epoch != source
            || cell.effective_epoch != source.wrapping_add(1)
        {
            return Err(LocalWorldError::Epoch);
        }
        self.command = cell;
        self.pending_deployment = Phase85DeploymentCommand {
            drogue: cell.discrete & 1 != 0,
            main: cell.discrete & 2 != 0,
        };
        let phase = self.machine.snapshot().phase;
        let powered = matches!(
            phase,
            HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight
        ) && self.machine.snapshot().state.time.raw() < self.motor.burn_time.raw();
        if !gimbal_jammed {
            self.actuator.release(cell.gimbal, powered);
        }
        Ok(())
    }
}

/// Compile the declared actuator point mass into a separately identified derivative.
pub fn derive_gimbal_derivative_vehicle(
    base: SpatialVehiclePack,
    capability: ActuatorCapabilityPack,
) -> Result<SpatialVehiclePack, LocalWorldError> {
    if capability.capability != ActuatorCapabilityId::TwoAxisMotorGimbalV1
        || capability.vehicle_identity == base.identity
        || capability.actuator_mass_q21 <= 0
    {
        return Err(LocalWorldError::Capability);
    }
    let base_mass = i64::from(base.dry_mass.raw());
    let actuator_mass = i64::from(capability.actuator_mass_q21);
    let total = base_mass + actuator_mass;
    let cg = (base_mass * i64::from(base.dry_cg_from_nose.raw())
        + actuator_mass * i64::from(capability.pivot_from_nose_q28))
        / total;
    let distance = i64::from(capability.pivot_from_nose_q28) - cg;
    let parallel_q19 = (actuator_mass * ((distance * distance) >> 28)) >> 30;
    let mass_raw = i32::try_from(total).map_err(|_| LocalWorldError::Capability)?;
    let cg_raw = i32::try_from(cg).map_err(|_| LocalWorldError::Capability)?;
    let parallel_raw = i32::try_from(parallel_q19).map_err(|_| LocalWorldError::Capability)?;
    let mut derivative = base;
    derivative.identity = capability.vehicle_identity;
    derivative.dry_mass = ksa64_core::phase8_numeric::SpatialMass::from_raw(mass_raw);
    derivative.dry_cg_from_nose = ksa64_core::phase8_numeric::SpatialMomentArm::from_raw(cg_raw);
    derivative.dry_inertia[1] = ksa64_core::phase8_numeric::SpatialInertia::from_raw(
        derivative.dry_inertia[1].raw().saturating_add(parallel_raw),
    );
    derivative.dry_inertia[2] = ksa64_core::phase8_numeric::SpatialInertia::from_raw(
        derivative.dry_inertia[2].raw().saturating_add(parallel_raw),
    );
    if !derivative.is_valid() {
        return Err(LocalWorldError::Capability);
    }
    Ok(derivative)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalLoopbackEvidence {
    pub releases: u32,
    pub result: Phase8MissionResult,
    pub last_flight: LocalFlightEvidence,
    pub cell_checksum: u32,
    pub max_navigation_error_q13: i32,
    pub max_attitude_error_turn16: i16,
    pub saturation_count: u16,
    pub deployment_decisions: u16,
    pub link_losses: u16,
    pub checksum_chains: [u32; 6],
}
fn apply_inertial_variation(
    mut cell: LocalInertialCell,
    faults: LocalAvionicsVariation,
) -> Option<LocalInertialCell> {
    let epoch = cell.measurement_epoch;
    if epoch_in_window(epoch, faults.imu_dropout_start, faults.imu_dropout_epochs) {
        return None;
    }
    for axis in 0..3 {
        let noise = keyed_noise(
            faults.seed,
            epoch,
            1,
            axis as u8,
            i32::from(faults.sensor_noise_scale) / 32,
        );
        cell.delta_velocity[axis] = cell.delta_velocity[axis]
            .saturating_add(faults.imu_delta_bias[axis])
            .saturating_add(clamp_i16(noise));
        cell.angular_rate[axis] = cell.angular_rate[axis]
            .saturating_add(faults.gyro_bias[axis])
            .saturating_add(clamp_i16(noise / 2));
    }
    Some(cell)
}
fn apply_aid_variation(mut cell: LocalAidCell, faults: LocalAvionicsVariation) -> LocalAidCell {
    let epoch = cell.measurement_epoch;
    if epoch_in_window(
        epoch,
        faults.barometer_dropout_start,
        faults.barometer_dropout_epochs,
    ) {
        cell.validity &= !LOCAL_AID_BAROMETER;
    } else {
        cell.barometer_q13 = cell
            .barometer_q13
            .saturating_add(faults.barometer_bias_q13)
            .saturating_add(keyed_noise(
                faults.seed,
                epoch,
                2,
                2,
                i32::from(faults.sensor_noise_scale) << 4,
            ));
    }
    if epoch_in_window(epoch, faults.gps_dropout_start, faults.gps_dropout_epochs) {
        cell.validity &= !LOCAL_AID_GPS;
    } else if cell.validity & LOCAL_AID_GPS != 0 {
        for axis in 0..3 {
            cell.gps_position_q13[axis] = cell.gps_position_q13[axis]
                .saturating_add(faults.gps_position_bias_q13[axis])
                .saturating_add(keyed_noise(
                    faults.seed,
                    epoch,
                    3,
                    axis as u8,
                    i32::from(faults.sensor_noise_scale) << 5,
                ));
            cell.gps_velocity_q19[axis] =
                cell.gps_velocity_q19[axis].saturating_add(faults.gps_velocity_bias_q19[axis]);
        }
    }
    if faults.continuity_open {
        cell.continuity = 0;
    }
    cell.deployment_feedback &= !faults.feedback_fail_mask;
    let drift = (i64::from(cell.onboard_time_q18) * i64::from(faults.clock_drift_ppm)) / 1_000_000;
    cell.onboard_time_q18 = cell.onboard_time_q18.saturating_add(drift as i32);
    cell.clock_flags = u16::from(faults.clock_drift_ppm != 0);
    cell
}

#[derive(Clone, Copy)]
pub struct LocalLoopbackRequest<'a> {
    pub vehicle: &'a SpatialVehiclePack,
    pub motor: &'a SpatialMotorPack,
    pub mission: SpatialMissionPack,
    pub wind: &'a WindProfilePack,
    pub physical_variation: SpatialMissionVariation,
    pub capability: ActuatorCapabilityPack,
    pub flight_config: LocalFlightConfig,
    pub avionics_variation: LocalAvionicsVariation,
}

pub fn run_local_loopback(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
    variation: SpatialMissionVariation,
    capability: ActuatorCapabilityPack,
    flight_config: LocalFlightConfig,
) -> Result<LocalLoopbackEvidence, LocalWorldError> {
    run_local_loopback_with_faults(LocalLoopbackRequest {
        vehicle,
        motor,
        mission,
        wind,
        physical_variation: variation,
        capability,
        flight_config,
        avionics_variation: LocalAvionicsVariation::NOMINAL,
    })
}

pub fn run_local_loopback_with_faults(
    request: LocalLoopbackRequest<'_>,
) -> Result<LocalLoopbackEvidence, LocalWorldError> {
    let LocalLoopbackRequest {
        vehicle,
        motor,
        mission,
        wind,
        physical_variation,
        capability,
        flight_config,
        avionics_variation: faults,
    } = request;
    if !faults.is_valid() {
        return Err(LocalWorldError::Capability);
    }
    let mut world = LocalWorldEndpoint::new(
        vehicle,
        motor,
        mission,
        wind,
        physical_variation,
        capability,
    )?;
    let initial = world.snapshot().state;
    let q = initial.attitude;
    let mut flight = LocalFlightComputer::new(
        flight_config,
        [
            initial.position.x(),
            initial.position.y(),
            initial.position.z(),
        ],
        [
            clamp_i16(q.x() >> 15),
            clamp_i16(q.y() >> 15),
            clamp_i16(q.z() >> 15),
        ],
    )
    .ok_or(LocalWorldError::Capability)?;
    let mut releases = 0u32;
    let mut command_checksum = 0x811c_9dc5;
    let mut sensor_checksum = 0x811c_9dc5;
    let mut status_checksum = 0x811c_9dc5;
    let mut max_navigation_error_q13 = 0i32;
    let mut max_attitude_error_turn16 = 0i16;
    let mut saturation_count = 0u16;
    let mut deployment_decisions = 0u16;
    let mut link_losses = 0u16;
    let mut aid_queue = [None; 33];
    let mut deployment_queue = [0u8; 33];
    let mut last = None;
    while !world.is_complete() {
        let Some(release) = world.release()? else {
            break;
        };
        let epoch = release.inertial.measurement_epoch;
        let mut delivered_aid = aid_queue[usize::from(epoch) % aid_queue.len()].take();
        if let Some(aid) = release.aid {
            let varied = apply_aid_variation(aid, faults);
            if faults.aid_delay_epochs == 0 {
                delivered_aid = Some(varied);
            } else {
                let delivery = epoch.wrapping_add(u16::from(faults.aid_delay_epochs));
                aid_queue[usize::from(delivery) % aid_queue.len()] = Some(varied);
            }
        }
        let varied_inertial = apply_inertial_variation(release.inertial, faults);
        let link_missing =
            epoch_in_window(epoch, faults.link_dropout_start, faults.link_dropout_epochs);
        if epoch == faults.missed_deadline_epoch {
            flight.record_deadline_miss();
        }
        let out = flight.tick(
            if link_missing { None } else { varied_inertial },
            if link_missing { None } else { delivered_aid },
        );
        if link_missing {
            link_losses = link_losses.saturating_add(1);
        } else {
            let mut physical_command = out.command;
            let queued = deployment_queue[usize::from(epoch) % deployment_queue.len()];
            deployment_queue[usize::from(epoch) % deployment_queue.len()] = 0;
            if faults.deployment_actuation_delay_epochs == 0 {
                physical_command.discrete |= queued;
            } else {
                if out.command.discrete != 0 {
                    let delivery =
                        epoch.wrapping_add(u16::from(faults.deployment_actuation_delay_epochs));
                    deployment_queue[usize::from(delivery) % deployment_queue.len()] |=
                        out.command.discrete;
                }
                physical_command.discrete = queued;
            }
            world.accept_command_faulted(physical_command, epoch >= faults.gimbal_jam_epoch)?;
        }
        let mut command_bytes = [0; ksa64_interface::phase8_5::LOCAL_COMMAND_LENGTH];
        write_local_command(&out.command, &mut command_bytes)
            .map_err(|_| LocalWorldError::Capability)?;
        command_checksum = hash_bytes(command_checksum, &command_bytes);
        let mut inertial_bytes = [0; ksa64_interface::phase8_5::LOCAL_INERTIAL_LENGTH];
        if let Some(inertial) = varied_inertial {
            write_local_inertial(&inertial, &mut inertial_bytes)
                .map_err(|_| LocalWorldError::Capability)?;
            sensor_checksum = hash_bytes(sensor_checksum, &inertial_bytes);
        }
        if let Some(aid) = delivered_aid {
            let mut aid_bytes = [0; ksa64_interface::phase8_5::LOCAL_AID_LENGTH];
            write_local_aid(&aid, &mut aid_bytes).map_err(|_| LocalWorldError::Capability)?;
            sensor_checksum = hash_bytes(sensor_checksum, &aid_bytes);
        }
        if let Some(status) = out.status {
            let mut status_bytes = [0; ksa64_interface::phase8_5::LOCAL_STATUS_LENGTH];
            write_local_status(&status, &mut status_bytes)
                .map_err(|_| LocalWorldError::Capability)?;
            status_checksum = hash_bytes(status_checksum, &status_bytes);
        }
        let truth = release.director.snapshot.state;
        for axis in 0..3 {
            max_navigation_error_q13 = max_navigation_error_q13.max(
                out.navigation.position_q13[axis]
                    .saturating_sub(truth.position.component(axis))
                    .saturating_abs(),
            );
        }
        if let Some(inertial) = varied_inertial {
            max_attitude_error_turn16 = max_attitude_error_turn16.max(
                inertial.platform_angle[1]
                    .saturating_abs()
                    .max(inertial.platform_angle[2].saturating_abs()),
            );
        }
        if out.command.gimbal[0].saturating_abs() >= flight_config.gimbal_limit_q15
            || out.command.gimbal[1].saturating_abs() >= flight_config.gimbal_limit_q15
        {
            saturation_count = saturation_count.saturating_add(1);
        }
        deployment_decisions |= u16::from(out.command.discrete & 3);
        releases = releases.saturating_add(1);
        last = Some(out);
    }
    let result = world.result().ok_or(LocalWorldError::Incomplete)?;
    let last_flight = last.ok_or(LocalWorldError::Incomplete)?;
    Ok(LocalLoopbackEvidence {
        releases,
        result,
        last_flight,
        cell_checksum: command_checksum ^ sensor_checksum.rotate_left(7),
        max_navigation_error_q13,
        max_attitude_error_turn16,
        saturation_count,
        deployment_decisions,
        link_losses,
        checksum_chains: [
            result.checksum,
            sensor_checksum,
            last_flight.navigation.checksum,
            command_checksum,
            status_checksum,
            world.snapshot().state.time.raw() as u32,
        ],
    })
}
fn hash_bytes(mut hash: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}
pub fn reference_monitor_capability(vehicle_identity: u32) -> ActuatorCapabilityPack {
    ActuatorCapabilityPack {
        identity: 0x8500_0001,
        capability: ActuatorCapabilityId::MonitorOnlyV1,
        flags: 0,
        lag_releases: 0,
        vehicle_identity,
        gimbal_limit_q16_deg: 0,
        slew_q16_deg_per_s: 0,
        pivot_from_nose_q28: 0,
        actuator_mass_q21: 0,
        proportional_gain_q15: 0,
        derivative_gain_q15: 0,
    }
}
pub fn reference_gimbal_capability(vehicle_identity: u32) -> ActuatorCapabilityPack {
    ActuatorCapabilityPack {
        identity: 0x8500_0002,
        capability: ActuatorCapabilityId::TwoAxisMotorGimbalV1,
        flags: 0,
        lag_releases: 2,
        vehicle_identity,
        gimbal_limit_q16_deg: 5 * 65_536,
        slew_q16_deg_per_s: 30 * 65_536,
        pivot_from_nose_q28: 510_000_000,
        actuator_mass_q21: 314_573,
        proportional_gain_q15: 8_192,
        derivative_gain_q15: 4_096,
    }
}
pub fn reference_avionics_profile(gimbal: bool) -> AvionicsProfilePack {
    AvionicsProfilePack {
        identity: if gimbal { 0x8500_2002 } else { 0x8500_2001 },
        profile: if gimbal {
            AvionicsProfileId::LocalEnuGimbalV1
        } else {
            AvionicsProfileId::LocalEnuRecoveryV1
        },
        frame: ksa64_core::phase8_5_contract::ReferenceFrameId::LocalEnuV1,
        fast_hz: 32,
        navigation_hz: 8,
        guidance_hz: 1,
        flags: 0,
        sensor_flags: 0,
        minimum_arming_time_q18: 1 << 18,
        minimum_arming_altitude_q13: 10 << 13,
        drogue_backup_time_q18: 15 << 18,
        main_backup_time_q18: 65 << 18,
        main_altitude_q13: 200 << 13,
        minimum_deployment_separation_q18: 2 << 18,
        sensor_seed: 0x4b53_4185,
        hold_epochs: 2,
        safe_epochs: 3,
        barometer_delay_epochs: 0,
        gps_delay_epochs: 0,
    }
}
pub fn local_flight_config(
    avionics: AvionicsProfilePack,
    capability: ActuatorCapabilityPack,
    motor: &SpatialMotorPack,
) -> Result<LocalFlightConfig, LocalWorldError> {
    let expected_profile = match capability.capability {
        ActuatorCapabilityId::MonitorOnlyV1 => AvionicsProfileId::LocalEnuRecoveryV1,
        ActuatorCapabilityId::TwoAxisMotorGimbalV1 => AvionicsProfileId::LocalEnuGimbalV1,
    };
    if !avionics.is_valid() || !capability.is_valid() || avionics.profile != expected_profile {
        return Err(LocalWorldError::Capability);
    }
    Ok(LocalFlightConfig {
        session: LOCAL_SESSION,
        capability: match capability.capability {
            ActuatorCapabilityId::MonitorOnlyV1 => LocalControlCapability::MonitorOnly,
            ActuatorCapabilityId::TwoAxisMotorGimbalV1 => LocalControlCapability::TwoAxisGimbal,
        },
        minimum_arming_time_q18: avionics.minimum_arming_time_q18,
        minimum_arming_altitude_q13: avionics.minimum_arming_altitude_q13,
        burnout_qualification_time_q18: motor.burn_time.raw(),
        drogue_backup_time_q18: avionics.drogue_backup_time_q18,
        main_backup_time_q18: avionics.main_backup_time_q18,
        main_altitude_q13: avionics.main_altitude_q13,
        minimum_deployment_separation_q18: avionics.minimum_deployment_separation_q18,
        proportional_gain_q15: capability.proportional_gain_q15.clamp(0, i16::MAX as i32) as i16,
        derivative_gain_q15: capability.derivative_gain_q15.clamp(0, i16::MAX as i32) as i16,
        gimbal_limit_q15: degrees_q16_to_turn16(capability.gimbal_limit_q16_deg),
    })
}

#[derive(Clone, Copy)]
pub struct AvionicsEvaluationRequest<'a> {
    pub vehicle: &'a SpatialVehiclePack,
    pub motor: &'a SpatialMotorPack,
    pub mission: SpatialMissionPack,
    pub wind: &'a WindProfilePack,
    pub variation: SpatialMissionVariation,
    pub variation_checksum: u32,
    pub avionics: AvionicsProfilePack,
    pub capability: ActuatorCapabilityPack,
    pub uncertainty_case: LocalAvionicsVariation,
}

pub fn evaluate_with_avionics(
    request: AvionicsEvaluationRequest<'_>,
) -> Result<AvionicsEvaluationSummary, LocalWorldError> {
    if request.capability.vehicle_identity != request.vehicle.identity {
        return Err(LocalWorldError::Capability);
    }
    let flight_config = local_flight_config(request.avionics, request.capability, request.motor)?;
    let evidence = run_local_loopback_with_faults(LocalLoopbackRequest {
        vehicle: request.vehicle,
        motor: request.motor,
        mission: request.mission,
        wind: request.wind,
        physical_variation: request.variation,
        capability: request.capability,
        flight_config,
        avionics_variation: request.uncertainty_case,
    })?;
    let physical = crate::evaluation::adapt_hobby_spatial(
        request.vehicle,
        request.motor,
        request.mission,
        request.wind,
        request.variation_checksum,
        evidence.result,
    );
    Ok(AvionicsEvaluationSummary {
        physical,
        physical_summary_identity: spatial_evaluation_identity(physical),
        avionics_identity: request.avionics.identity,
        actuator_identity: request.capability.identity,
        max_navigation_error_q13: evidence.max_navigation_error_q13,
        max_attitude_error_turn16: evidence.max_attitude_error_turn16,
        saturation_count: evidence.saturation_count,
        deployment_decisions: evidence.deployment_decisions,
        deployment_feedback: evidence.result.event_history & (EVENT_DROGUE | EVENT_MAIN),
        alarms: evidence.last_flight.alarms,
        deadline_misses: evidence.last_flight.deadline_misses,
        link_losses: evidence.link_losses,
        checksum_chains: evidence.checksum_chains,
    })
}

trait Component {
    fn component(self, index: usize) -> i32;
}
impl<const F: u8> Component for ksa64_core::spatial_numeric::FixedVec3<F> {
    fn component(self, index: usize) -> i32 {
        match index {
            0 => self.x(),
            1 => self.y(),
            _ => self.z(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase8_5_contract::ActuatorCapabilityId;
    use ksa64_core::phase8_pack::{
        parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
        parse_wind_profile_pack,
    };
    use ksa64_flight::phase8_5::LocalControlCapability;

    fn fixtures() -> (
        SpatialVehiclePack,
        SpatialMotorPack,
        SpatialMissionPack,
        WindProfilePack,
    ) {
        (
            parse_spatial_vehicle_pack(include_bytes!("../../phase8/examples/firestorm54.kvp8"))
                .unwrap(),
            parse_spatial_motor_pack(include_bytes!("../../phase8/examples/aerotech-i211w.kmp8"))
                .unwrap(),
            parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
                .unwrap(),
            parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
                .unwrap(),
        )
    }
    fn capability(vehicle_identity: u32, active: bool) -> ActuatorCapabilityPack {
        ActuatorCapabilityPack {
            identity: if active { 0x8500_0002 } else { 0x8500_0001 },
            capability: if active {
                ActuatorCapabilityId::TwoAxisMotorGimbalV1
            } else {
                ActuatorCapabilityId::MonitorOnlyV1
            },
            flags: 0,
            lag_releases: if active { 2 } else { 0 },
            vehicle_identity,
            gimbal_limit_q16_deg: if active { 5 * 65_536 } else { 0 },
            slew_q16_deg_per_s: if active { 30 * 65_536 } else { 0 },
            pivot_from_nose_q28: if active { 510_000_000 } else { 0 },
            actuator_mass_q21: if active { 314_573 } else { 0 },
            proportional_gain_q15: if active { 8_192 } else { 0 },
            derivative_gain_q15: if active { 4_096 } else { 0 },
        }
    }
    fn flight(active: bool) -> LocalFlightConfig {
        LocalFlightConfig {
            session: LOCAL_SESSION,
            capability: if active {
                LocalControlCapability::TwoAxisGimbal
            } else {
                LocalControlCapability::MonitorOnly
            },
            minimum_arming_time_q18: 1 << 18,
            minimum_arming_altitude_q13: 10 << 13,
            burnout_qualification_time_q18: 3 << 18,
            drogue_backup_time_q18: 15 << 18,
            main_backup_time_q18: 65 << 18,
            main_altitude_q13: 200 << 13,
            minimum_deployment_separation_q18: 2 << 18,
            proportional_gain_q15: 8_192,
            derivative_gain_q15: 4_096,
            gimbal_limit_q15: 910,
        }
    }
    #[test]
    fn monitor_loopback_is_repeatable_and_recovers() {
        let (vehicle, motor, mission, wind) = fixtures();
        let cap = capability(vehicle.identity, false);
        let a = run_local_loopback(
            &vehicle,
            &motor,
            mission,
            &wind,
            SpatialMissionVariation::NOMINAL,
            cap,
            flight(false),
        )
        .unwrap();
        let b = run_local_loopback(
            &vehicle,
            &motor,
            mission,
            &wind,
            SpatialMissionVariation::NOMINAL,
            cap,
            flight(false),
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact
        );
        assert_ne!(a.result.event_history & EVENT_DROGUE, 0);
        assert_ne!(a.result.event_history & EVENT_MAIN, 0);
        assert_eq!(a.last_flight.command.gimbal, [0; 2]);
    }
    #[test]
    fn derivative_compiles_actuator_mass_and_runs() {
        let (base, motor, mut mission, wind) = fixtures();
        let cap = capability(0x8500_1001, true);
        let derivative = derive_gimbal_derivative_vehicle(base, cap).unwrap();
        assert!(derivative.dry_mass.raw() > base.dry_mass.raw());
        mission.vehicle_identity = derivative.identity;
        let evidence = run_local_loopback(
            &derivative,
            &motor,
            mission,
            &wind,
            SpatialMissionVariation::NOMINAL,
            cap,
            flight(true),
        )
        .unwrap();
        assert_eq!(
            evidence.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact
        );
    }
    #[test]
    fn public_avionics_evaluator_wraps_the_physical_summary() {
        let (vehicle, motor, mission, wind) = fixtures();
        let cap = capability(vehicle.identity, false);
        let avionics = AvionicsProfilePack {
            identity: 0x8500_2001,
            profile: AvionicsProfileId::LocalEnuRecoveryV1,
            frame: ksa64_core::phase8_5_contract::ReferenceFrameId::LocalEnuV1,
            fast_hz: 32,
            navigation_hz: 8,
            guidance_hz: 1,
            flags: 0,
            sensor_flags: 0,
            minimum_arming_time_q18: 1 << 18,
            minimum_arming_altitude_q13: 10 << 13,
            drogue_backup_time_q18: 15 << 18,
            main_backup_time_q18: 65 << 18,
            main_altitude_q13: 200 << 13,
            minimum_deployment_separation_q18: 2 << 18,
            sensor_seed: 0x4b53_4185,
            hold_epochs: 2,
            safe_epochs: 3,
            barometer_delay_epochs: 0,
            gps_delay_epochs: 0,
        };
        let summary = evaluate_with_avionics(AvionicsEvaluationRequest {
            vehicle: &vehicle,
            motor: &motor,
            mission,
            wind: &wind,
            variation: SpatialMissionVariation::NOMINAL,
            variation_checksum: 0,
            avionics,
            capability: cap,
            uncertainty_case: LocalAvionicsVariation::NOMINAL,
        })
        .unwrap();
        assert_eq!(
            summary.physical.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact
        );
        assert_eq!(summary.deployment_decisions, 3);
        assert_eq!(summary.deployment_feedback, EVENT_DROGUE | EVENT_MAIN);
        assert_ne!(summary.checksum_chains[1], 0);
    }
    fn run_fault_case(faults: LocalAvionicsVariation) -> LocalLoopbackEvidence {
        let (vehicle, motor, mission, wind) = fixtures();
        run_local_loopback_with_faults(LocalLoopbackRequest {
            vehicle: &vehicle,
            motor: &motor,
            mission,
            wind: &wind,
            physical_variation: SpatialMissionVariation::NOMINAL,
            capability: capability(vehicle.identity, false),
            flight_config: flight(false),
            avionics_variation: faults,
        })
        .unwrap()
    }
    #[test]
    fn bounded_link_loss_holds_then_third_epoch_safes() {
        let mut two = LocalAvionicsVariation::NOMINAL;
        two.link_dropout_start = 2_100;
        two.link_dropout_epochs = 2;
        let held = run_fault_case(two);
        assert!(!held.last_flight.safe);
        assert_eq!(held.link_losses, 2);
        let mut three = two;
        three.link_dropout_epochs = 3;
        let safe = run_fault_case(three);
        assert!(safe.last_flight.safe);
        assert_eq!(safe.link_losses, 3);
        assert_ne!(
            safe.last_flight.alarms & ksa64_flight::phase8_5::LOCAL_ALARM_LINK,
            0
        );
    }
    #[test]
    fn recovery_backups_do_not_require_truth_or_feedback() {
        let mut faults = LocalAvionicsVariation::NOMINAL;
        faults.barometer_dropout_start = 1;
        faults.barometer_dropout_epochs = u8::MAX;
        faults.gps_dropout_start = 1;
        faults.gps_dropout_epochs = u8::MAX;
        faults.feedback_fail_mask = 3;
        let evidence = run_fault_case(faults);
        assert_eq!(evidence.deployment_decisions, 3);
        assert_eq!(
            evidence.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact
        );
    }
    #[test]
    fn sensor_noise_latency_and_clock_drift_are_repeatable() {
        let mut faults = LocalAvionicsVariation::NOMINAL;
        faults.sensor_noise_scale = 256;
        faults.aid_delay_epochs = 3;
        faults.clock_drift_ppm = 500;
        faults.imu_delta_bias = [1, -2, 3];
        faults.gyro_bias = [1, 1, -1];
        let a = run_fault_case(faults);
        let b = run_fault_case(faults);
        assert_eq!(a.checksum_chains, b.checksum_chains);
        assert_ne!(
            a.checksum_chains[1],
            run_fault_case(LocalAvionicsVariation::NOMINAL).checksum_chains[1]
        );
    }
    #[test]
    fn one_epoch_loss_and_deployment_delay_remain_bounded() {
        let mut one = LocalAvionicsVariation::NOMINAL;
        one.link_dropout_start = 2_100;
        one.link_dropout_epochs = 1;
        let held = run_fault_case(one);
        assert!(!held.last_flight.safe);
        assert_eq!(held.link_losses, 1);
        let mut delayed = LocalAvionicsVariation::NOMINAL;
        delayed.deployment_actuation_delay_epochs = 4;
        let a = run_fault_case(delayed);
        let b = run_fault_case(delayed);
        assert_eq!(a.checksum_chains, b.checksum_chains);
        assert_eq!(
            a.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact
        );
        assert_ne!(
            a.checksum_chains,
            run_fault_case(LocalAvionicsVariation::NOMINAL).checksum_chains
        );
    }
    #[test]
    fn gps_outage_and_imu_bias_are_deterministic_named_cases() {
        let mut gps = LocalAvionicsVariation::NOMINAL;
        gps.gps_dropout_start = 1;
        gps.gps_dropout_epochs = u8::MAX;
        let gps_a = run_fault_case(gps);
        let gps_b = run_fault_case(gps);
        assert_eq!(gps_a.checksum_chains, gps_b.checksum_chains);
        let mut imu = LocalAvionicsVariation::NOMINAL;
        imu.imu_delta_bias = [3, -2, 4];
        imu.gyro_bias = [2, -1, 1];
        let biased = run_fault_case(imu);
        assert_ne!(biased.checksum_chains[1], gps_a.checksum_chains[1]);
    }
    #[test]
    fn deadline_and_open_continuity_fail_closed() {
        let mut deadline = LocalAvionicsVariation::NOMINAL;
        deadline.missed_deadline_epoch = 2_100;
        let missed = run_fault_case(deadline);
        assert!(missed.last_flight.safe);
        assert_eq!(missed.last_flight.deadline_misses, 1);
        let mut open = LocalAvionicsVariation::NOMINAL;
        open.continuity_open = true;
        let incomplete = run_fault_case(open);
        assert_eq!(incomplete.deployment_decisions, 0);
        assert_eq!(
            incomplete.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::RecoveryIncomplete
        );
    }
}
