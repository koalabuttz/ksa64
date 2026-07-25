//! Phase 8.5 exact local world endpoint and in-memory loopback.

use ksa64_core::phase8_5_clock::{ExactClockError, ExactEventClock};
use ksa64_core::phase8_5_contract::{ActuatorCapabilityId, ActuatorCapabilityPack};
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
use ksa64_flight::phase8_5::{LocalFlightComputer, LocalFlightConfig, LocalFlightEvidence};
use ksa64_interface::phase8_5::{
    LocalAidCell, LocalCommandCell, LocalInertialCell, LOCAL_AID_ATTITUDE, LOCAL_AID_BAROMETER,
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
            physical_deadline: SpatialTime::from_raw(state.time.raw() + first.raw()),
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
            self.machine
                .step_avionics(segment.duration(), applied, deployment)?;
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
                self.physical_deadline = SpatialTime::from_raw(deadline)
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
        self.actuator.release(cell.gimbal, powered);
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
    let mut world = LocalWorldEndpoint::new(vehicle, motor, mission, wind, variation, capability)?;
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
    let mut checksum = 0x811c_9dc5;
    let mut last = None;
    while !world.is_complete() {
        let Some(release) = world.release()? else {
            break;
        };
        let out = flight.tick(Some(release.inertial), release.aid);
        world.accept_command(out.command)?;
        checksum = hash_cell(checksum, release.inertial.measurement_epoch, out.command);
        releases = releases.saturating_add(1);
        last = Some(out)
    }
    let result = world.result().ok_or(LocalWorldError::Incomplete)?;
    Ok(LocalLoopbackEvidence {
        releases,
        result,
        last_flight: last.ok_or(LocalWorldError::Incomplete)?,
        cell_checksum: checksum,
    })
}
fn hash_cell(mut h: u32, epoch: u16, c: LocalCommandCell) -> u32 {
    for v in [
        epoch as u32,
        c.source_epoch as u32,
        c.effective_epoch as u32,
        c.gimbal[0] as u16 as u32,
        c.gimbal[1] as u16 as u32,
        c.discrete as u32,
    ] {
        for b in v.to_le_bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16_777_619)
        }
    }
    h
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
}
