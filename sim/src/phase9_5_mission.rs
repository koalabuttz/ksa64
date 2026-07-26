//! Phase 9.5 integrated advanced-effector world and truth-blind loopback.

// Fixed axis loops keep portable/MOS operation order explicit.
#![allow(clippy::needless_range_loop)]

use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase8_5_clock::{ExactClockError, ExactEventClock};
use ksa64_core::phase8_5_contract::{
    ActuatorCapabilityId, ActuatorCapabilityPack, AvionicsProfilePack,
};
use ksa64_core::phase8_mission::{
    HobbySpatialPhase, Phase85AppliedControl, Phase85DeploymentCommand, Phase8MissionError,
    Phase8MissionMachine, Phase8MissionResult, Phase8MissionSnapshot, Phase95AppliedControl,
    Phase95PhysicalFeedback, SpatialMissionVariation, EVENT_DROGUE, EVENT_MAIN,
};
use ksa64_core::phase8_numeric::{
    SpatialTime, SPATIAL_COAST_TRANSLATION_STEP, SPATIAL_POWERED_STEP, SPATIAL_RECOVERY_STEP,
};
use ksa64_core::phase8_pack::{
    SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindProfilePack,
};
use ksa64_core::phase9_5_canard::{CanardActuatorState, CanardError, CanardFaultMode};
use ksa64_core::phase9_5_contract::{
    AdvancedEffectorPack, PriorityResidualAllocatorPack, MAX_CANARDS, MAX_RCS_JETS,
};
use ksa64_core::phase9_5_rcs::{
    integrate_rcs_segment, sample_supply, RcsError, RcsJetFault, RcsPulseCommand, RcsState,
};
use ksa64_core::phase9_5_telemetry::AdvancedTelemetryFrame;
use ksa64_flight::phase9_5::{AdvancedFlightComputer, AdvancedFlightConfig, AirDataSource};
use ksa64_flight::phase9_5_allocator::{AllocatedAdvancedFlightComputer, AllocatedFlightEvidence};
use ksa64_interface::phase9_5::{
    write_advanced_aid, write_advanced_command, write_advanced_fast_sensor, write_advanced_status,
    AdvancedAidCell, AdvancedCommandCell, AdvancedFastSensorCell, ADVANCED_AID_ATTITUDE,
    ADVANCED_AID_BAROMETER, ADVANCED_AID_CONTINUITY, ADVANCED_AID_DEPLOYMENT_FEEDBACK,
    ADVANCED_AID_GPS, ADVANCED_COMMAND_DROGUE, ADVANCED_COMMAND_MAIN, ADVANCED_COMMAND_SAFE,
    ADVANCED_VALID_ACTUATOR, ADVANCED_VALID_AIR_DATA, ADVANCED_VALID_DELTA_V,
    ADVANCED_VALID_PLATFORM, ADVANCED_VALID_RATE, ADVANCED_VALID_SUPPLY,
};

use crate::phase8_5::{
    local_flight_config, reference_gimbal_capability, reference_monitor_capability, LOCAL_SESSION,
};
use crate::phase9_5::{allocator_config, AdvancedCompositionError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedWorldError {
    Mission(Phase8MissionError),
    Clock(ExactClockError),
    Rcs(RcsError),
    Canard(CanardError),
    Composition(AdvancedCompositionError),
    Identity,
    Epoch,
    Incomplete,
}
impl From<Phase8MissionError> for AdvancedWorldError {
    fn from(v: Phase8MissionError) -> Self {
        Self::Mission(v)
    }
}
impl From<ExactClockError> for AdvancedWorldError {
    fn from(v: ExactClockError) -> Self {
        Self::Clock(v)
    }
}
impl From<RcsError> for AdvancedWorldError {
    fn from(v: RcsError) -> Self {
        Self::Rcs(v)
    }
}
impl From<CanardError> for AdvancedWorldError {
    fn from(v: CanardError) -> Self {
        Self::Canard(v)
    }
}
impl From<AdvancedCompositionError> for AdvancedWorldError {
    fn from(v: AdvancedCompositionError) -> Self {
        Self::Composition(v)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedMissionFaults {
    pub canards: [CanardFaultMode; MAX_CANARDS],
    pub rcs: [RcsJetFault; MAX_RCS_JETS],
    pub pitot_dropout_start: u16,
    pub pitot_dropout_epochs: u16,
    pub fast_dropout_start: u16,
    pub fast_dropout_epochs: u16,
    pub disturbance_epoch: u16,
    pub disturbance_angular_rate_q24: [i32; 3],
}
impl AdvancedMissionFaults {
    pub const NOMINAL: Self = Self {
        canards: [CanardFaultMode::Healthy; MAX_CANARDS],
        rcs: [RcsJetFault::Healthy; MAX_RCS_JETS],
        pitot_dropout_start: u16::MAX,
        pitot_dropout_epochs: 0,
        fast_dropout_start: u16::MAX,
        fast_dropout_epochs: 0,
        disturbance_epoch: u16::MAX,
        disturbance_angular_rate_q24: [0; 3],
    };
}
fn in_window(epoch: u16, start: u16, count: u16) -> bool {
    count != 0 && epoch.wrapping_sub(start) < count
}
fn clamp_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}
fn degrees_q16_to_turn16(value: i32) -> i16 {
    ((i64::from(value) * 65_536) / (360i64 * 65_536)).clamp(i16::MIN as i64, i16::MAX as i64) as i16
}
fn phase_maximum(phase: HobbySpatialPhase) -> Result<SpatialTime, AdvancedWorldError> {
    match phase {
        HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight => {
            Ok(SPATIAL_POWERED_STEP)
        }
        HobbySpatialPhase::Coast => Ok(SPATIAL_COAST_TRANSLATION_STEP),
        HobbySpatialPhase::DrogueRecovery | HobbySpatialPhase::MainRecovery => {
            Ok(SPATIAL_RECOVERY_STEP)
        }
        HobbySpatialPhase::Complete | HobbySpatialPhase::Failed => {
            Err(AdvancedWorldError::Incomplete)
        }
    }
}

#[derive(Clone, Copy)]
struct GimbalActuator {
    enabled: bool,
    limit: i16,
    slew: i16,
    lag: usize,
    queue: [[i16; 2]; 8],
    applied: [i16; 2],
}
impl GimbalActuator {
    fn new(capability: ActuatorCapabilityPack) -> Result<Self, AdvancedWorldError> {
        if !capability.is_valid() {
            return Err(AdvancedWorldError::Identity);
        }
        Ok(Self {
            enabled: capability.capability == ActuatorCapabilityId::TwoAxisMotorGimbalV1,
            limit: degrees_q16_to_turn16(capability.gimbal_limit_q16_deg),
            slew: (degrees_q16_to_turn16(capability.slew_q16_deg_per_s) / 32).max(1),
            lag: capability.lag_releases as usize,
            queue: [[0; 2]; 8],
            applied: [0; 2],
        })
    }
    fn release(&mut self, requested: [i16; 2], powered: bool, safe: bool) {
        if !self.enabled || !powered || safe {
            self.queue = [[0; 2]; 8];
            self.applied = [0; 2];
            return;
        }
        let requested = [
            requested[0].clamp(-self.limit, self.limit),
            requested[1].clamp(-self.limit, self.limit),
        ];
        let target = if self.lag == 0 {
            requested
        } else {
            let old = self.queue[0];
            for i in 1..self.lag {
                self.queue[i - 1] = self.queue[i];
            }
            self.queue[self.lag - 1] = requested;
            old
        };
        for axis in 0..2 {
            let delta = target[axis]
                .saturating_sub(self.applied[axis])
                .clamp(-self.slew, self.slew);
            self.applied[axis] = self.applied[axis].saturating_add(delta);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedDirectorSample {
    pub epoch: u16,
    pub snapshot: Phase8MissionSnapshot,
    pub truth_checksum: u32,
    pub physical_feedback: Phase95PhysicalFeedback,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedWorldRelease {
    pub fast: AdvancedFastSensorCell,
    pub aid: Option<AdvancedAidCell>,
    pub director: AdvancedDirectorSample,
}

pub struct AdvancedWorldEndpoint<'a> {
    machine: Phase8MissionMachine<'a>,
    motor: &'a SpatialMotorPack,
    effectors: &'a AdvancedEffectorPack,
    clock: ExactEventClock,
    physical_deadline: SpatialTime,
    max_mission_time: SpatialTime,
    last_release_state: ksa64_core::phase8_world::HobbySpatialState,
    last_release_epoch: Option<u16>,
    pending_deployment: Phase85DeploymentCommand,
    gimbal: GimbalActuator,
    canards: CanardActuatorState,
    rcs: Option<RcsState>,
    faults: AdvancedMissionFaults,
    pivot_from_nose_q28: i32,
    deployment_feedback: u16,
    physical_feedback: Phase95PhysicalFeedback,
    valve_edge_count: u32,
    depletion_count: u16,
    disturbance_applied: bool,
}
impl<'a> AdvancedWorldEndpoint<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vehicle: &'a SpatialVehiclePack,
        motor: &'a SpatialMotorPack,
        mission: SpatialMissionPack,
        wind: &'a WindProfilePack,
        variation: SpatialMissionVariation,
        capability: ActuatorCapabilityPack,
        effectors: &'a AdvancedEffectorPack,
        faults: AdvancedMissionFaults,
    ) -> Result<Self, AdvancedWorldError> {
        if capability.vehicle_identity != vehicle.identity
            || effectors.vehicle_identity != vehicle.identity
            || !effectors.is_valid()
        {
            return Err(AdvancedWorldError::Identity);
        }
        let machine =
            Phase8MissionMachine::new_with_variation(vehicle, motor, mission, wind, variation)?;
        let state = machine.snapshot().state;
        let first = phase_maximum(machine.snapshot().phase)?;
        let rcs = if effectors.set.has_rcs() {
            Some(RcsState::new(effectors)?)
        } else {
            None
        };
        Ok(Self {
            machine,
            motor,
            effectors,
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
            pending_deployment: Phase85DeploymentCommand::default(),
            gimbal: GimbalActuator::new(capability)?,
            canards: CanardActuatorState::NEUTRAL,
            rcs,
            faults,
            pivot_from_nose_q28: capability.pivot_from_nose_q28,
            deployment_feedback: 0,
            physical_feedback: Phase95PhysicalFeedback::ZERO,
            valve_edge_count: 0,
            depletion_count: 0,
            disturbance_applied: false,
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
    pub const fn valve_edge_count(&self) -> u32 {
        self.valve_edge_count
    }
    pub const fn depletion_count(&self) -> u16 {
        self.depletion_count
    }
    pub fn rcs_remaining_propellant_q21(&self) -> i32 {
        self.rcs
            .as_ref()
            .map_or(0, |state| state.remaining_propellant_q21)
    }

    pub fn release(&mut self) -> Result<Option<AdvancedWorldRelease>, AdvancedWorldError> {
        while !self.clock.release_due(self.machine.snapshot().state.time) {
            let now = self.machine.snapshot().state.time;
            let base_segment = self.clock.next_segment(now, self.physical_deadline)?;
            let mut end_raw = base_segment.end.raw();
            let mut valve_edge = false;
            if let Some(rcs) = &self.rcs {
                if let Some(edge) = rcs.next_valve_edge_after(now.raw(), end_raw) {
                    if edge < end_raw {
                        end_raw = edge;
                        valve_edge = true;
                    }
                }
            }
            let phase_before = self.machine.snapshot().phase;
            let mut rcs_force = [0; 3];
            let mut rcs_torque = [0; 3];
            let mut rcs_mask = 0u16;
            let mut rcs_remaining = 0;
            let mut rcs_pressure = 0;
            let mut rcs_scale = 0;
            if let Some(rcs) = &mut self.rcs {
                let mut numeric = NumericStatus::CLEAR;
                let seg = integrate_rcs_segment(
                    rcs,
                    self.effectors,
                    now.raw(),
                    end_raw,
                    self.machine.snapshot().mass.cg_from_nose.raw(),
                    self.faults.rcs,
                    &mut numeric,
                )?;
                if !numeric.is_clear() {
                    return Err(AdvancedWorldError::Rcs(RcsError::Numeric));
                }
                end_raw = seg.integrated_end_q18;
                rcs_force = seg.force_body_q23;
                rcs_torque = seg.torque_body_q12;
                rcs_mask = seg.active_mask;
                rcs_remaining = seg.remaining_propellant_q21;
                rcs_pressure = seg.pressure_q8;
                rcs_scale = seg.thrust_scale_q30;
                if seg.depleted {
                    self.depletion_count = self.depletion_count.saturating_add(1);
                }
            }
            let phase = self.machine.snapshot().phase;
            let powered =
                phase == HobbySpatialPhase::PoweredFlight && now.raw() < self.motor.burn_time.raw();
            let control = Phase95AppliedControl {
                legacy: if powered {
                    Phase85AppliedControl {
                        gimbal_turn16: self.gimbal.applied,
                        pivot_from_nose_q28: self.pivot_from_nose_q28,
                    }
                } else {
                    Phase85AppliedControl::NEUTRAL
                },
                canard_turn16: self.canards.applied_turn16,
                rcs_force_body_q23: rcs_force,
                rcs_torque_body_q12: rcs_torque,
                rcs_remaining_propellant_q21: rcs_remaining,
                rcs_pressure_q8: rcs_pressure,
                rcs_thrust_scale_q30: rcs_scale,
                rcs_active_mask: rcs_mask,
            };
            let deployment = self.pending_deployment;
            self.pending_deployment = Phase85DeploymentCommand::default();
            let duration = SpatialTime::from_raw(end_raw - now.raw());
            if let Err(error) = self.machine.step_advanced(
                duration,
                control,
                self.effectors,
                deployment,
                &mut self.physical_feedback,
            ) {
                if self.machine.is_complete() {
                    return Ok(None);
                }
                return Err(AdvancedWorldError::Mission(error));
            }
            self.canards
                .accept_load_limits(self.physical_feedback.effective_canard_turn16);
            if valve_edge {
                self.valve_edge_count = self.valve_edge_count.saturating_add(1);
            }
            let snapshot = self.machine.snapshot();
            if snapshot.events & EVENT_DROGUE != 0 {
                self.deployment_feedback |= 1;
            }
            if snapshot.events & EVENT_MAIN != 0 {
                self.deployment_feedback |= 2;
            }
            let ended_at_physical = end_raw == self.physical_deadline.raw();
            if ended_at_physical || snapshot.phase != phase_before {
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
                    deadline = deadline.min(self.motor.burn_time.raw());
                }
                self.physical_deadline =
                    SpatialTime::from_raw(deadline.min(self.max_mission_time.raw()));
            }
        }
        let state = self.machine.snapshot().state;
        if !self.clock.release_due(state.time) {
            if self.machine.is_complete() {
                return Ok(None);
            }
            return Err(AdvancedWorldError::Clock(ExactClockError::TimeMismatch));
        }
        let epoch = u16::try_from(self.clock.consume_release(state.time)?)
            .map_err(|_| AdvancedWorldError::Epoch)?;
        if !self.disturbance_applied && epoch == self.faults.disturbance_epoch {
            self.machine
                .inject_phase95_angular_rate(self.faults.disturbance_angular_rate_q24)?;
            self.disturbance_applied = true;
        }
        let state = self.machine.snapshot().state;
        let dt_raw = self
            .last_release_epoch
            .map_or(8192, |last| i32::from(epoch.wrapping_sub(last)) * 8192);
        let gravity_delta_q19 = ((5_141_509i64 * i64::from(dt_raw)) >> 18) as i32;
        let mut delta = [0i16; 3];
        for axis in 0..3 {
            let mut dv = state
                .velocity
                .component(axis)
                .saturating_sub(self.last_release_state.velocity.component(axis));
            if axis == 2 {
                dv = dv.saturating_add(gravity_delta_q19);
            }
            delta[axis] = clamp_i16(dv >> 7);
        }
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
        let q = state.attitude;
        let mut validity = ADVANCED_VALID_PLATFORM
            | ADVANCED_VALID_RATE
            | ADVANCED_VALID_DELTA_V
            | ADVANCED_VALID_ACTUATOR
            | ADVANCED_VALID_AIR_DATA;
        let (propellant, supply_scale, valves) = if let Some(rcs) = &self.rcs {
            validity |= ADVANCED_VALID_SUPPLY;
            let mut numeric = NumericStatus::CLEAR;
            let supply = sample_supply(self.effectors, rcs.remaining_propellant_q21, &mut numeric)?;
            (
                rcs.remaining_propellant_q21,
                (supply.thrust_scale_q30 >> 15).clamp(0, 32768) as u16,
                rcs.active_mask_at(state.time.raw(), self.faults.rcs),
            )
        } else {
            (0, 0, 0)
        };
        let fast = AdvancedFastSensorCell {
            session: LOCAL_SESSION,
            measurement_epoch: epoch,
            production_epoch: epoch,
            validity,
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
            dynamic_pressure_q10: self.machine.snapshot().aero.dynamic_pressure_q13 >> 3,
            mach_q12: clamp_i16(self.machine.snapshot().aero.mach_q24 >> 12),
            gimbal_applied: self.gimbal.applied,
            canard_applied: self.canards.applied_turn16,
            valve_open_mask: valves,
            propellant_q21: propellant,
            supply_scale_q15: supply_scale,
            vehicle_status,
            actuator_feedback: self.deployment_feedback,
            flags: u16::from(self.physical_feedback.canard_load_limited_mask),
        };
        let aid = if epoch & 3 == 0 {
            Some(AdvancedAidCell {
                session: LOCAL_SESSION,
                measurement_epoch: epoch,
                production_epoch: epoch,
                validity: ADVANCED_AID_BAROMETER
                    | if epoch & 31 == 0 { ADVANCED_AID_GPS } else { 0 }
                    | ADVANCED_AID_ATTITUDE
                    | ADVANCED_AID_CONTINUITY
                    | ADVANCED_AID_DEPLOYMENT_FEEDBACK,
                events: self.machine.snapshot().events,
                onboard_time_q18: state.time.raw(),
                barometer_q13: state.position.z(),
                gps_position_q13: [state.position.x(), state.position.y(), state.position.z()],
                gps_velocity_q19: [state.velocity.x(), state.velocity.y(), state.velocity.z()],
                attitude_vector: fast.platform_angle,
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
        Ok(Some(AdvancedWorldRelease {
            fast,
            aid,
            director: AdvancedDirectorSample {
                epoch,
                snapshot: self.machine.snapshot(),
                truth_checksum: self.machine.trace_checksum(),
                physical_feedback: self.physical_feedback,
            },
        }))
    }

    pub fn accept_command(&mut self, cell: AdvancedCommandCell) -> Result<(), AdvancedWorldError> {
        let source = self.last_release_epoch.ok_or(AdvancedWorldError::Epoch)?;
        if cell.session != LOCAL_SESSION
            || cell.source_epoch != source
            || cell.effective_epoch != source.wrapping_add(1)
        {
            return Err(AdvancedWorldError::Epoch);
        }
        let safe = cell.discrete & ADVANCED_COMMAND_SAFE != 0;
        self.pending_deployment = Phase85DeploymentCommand {
            drogue: !safe && cell.discrete & ADVANCED_COMMAND_DROGUE != 0,
            main: !safe && cell.discrete & ADVANCED_COMMAND_MAIN != 0,
        };
        let phase = self.machine.snapshot().phase;
        let powered = matches!(
            phase,
            HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight
        ) && self.machine.snapshot().state.time.raw() < self.motor.burn_time.raw();
        self.gimbal.release(cell.gimbal, powered, safe);
        if self.effectors.set.has_canards() {
            self.canards.release(
                if safe { [0; 4] } else { cell.canards },
                self.effectors,
                self.faults.canards,
            )?;
        }
        if let Some(rcs) = &mut self.rcs {
            if safe {
                rcs.safe();
            } else {
                rcs.schedule_successor(
                    self.machine.snapshot().state.time.raw(),
                    RcsPulseCommand {
                        quanta: cell.rcs_pulse_quanta,
                    },
                    self.effectors,
                    self.faults.rcs,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct AdvancedLoopbackRequest<'a> {
    pub vehicle: &'a SpatialVehiclePack,
    pub motor: &'a SpatialMotorPack,
    pub mission: SpatialMissionPack,
    pub wind: &'a WindProfilePack,
    pub variation: SpatialMissionVariation,
    pub variation_checksum: u32,
    pub avionics: AvionicsProfilePack,
    pub capability: ActuatorCapabilityPack,
    pub effectors: &'a AdvancedEffectorPack,
    pub allocator: &'a PriorityResidualAllocatorPack,
    pub faults: AdvancedMissionFaults,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedLoopbackEvidence {
    pub releases: u32,
    pub result: Phase8MissionResult,
    pub last: AllocatedFlightEvidence,
    pub cell_checksum: u32,
    pub max_navigation_error_q13: i32,
    pub max_attitude_error_turn16: i16,
    pub rail_settle_error_turn16: i16,
    pub disturbance_settle_error_turn16: i16,
    pub max_hinge_q24: [i32; 4],
    pub saturation_count: u32,
    pub pulse_count: u32,
    pub valve_edge_count: u32,
    pub depletion_count: u16,
    pub authority_handoffs: u16,
    pub air_fallback_epochs: u16,
    pub rcs_final_propellant_q21: i32,
    pub checksum_chains: [u32; 8],
}
fn hash_bytes(mut hash: u32, bytes: &[u8]) -> u32 {
    for b in bytes {
        hash ^= u32::from(*b);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}
fn advanced_flight_config(
    request: &AdvancedLoopbackRequest<'_>,
) -> Result<AdvancedFlightConfig, AdvancedWorldError> {
    let local_cap = reference_monitor_capability(request.vehicle.identity);
    let mut local = local_flight_config(request.avionics, local_cap, request.motor)
        .map_err(|_| AdvancedWorldError::Identity)?;
    // The frozen local kernel stays monitor-only for physical actuation, but its
    // deterministic pitch/yaw demand path is configured by the advanced KPA9
    // controller gains and then consumed by PriorityResidualV1.
    local.proportional_gain_q15 = request.allocator.roll_kp_q15.clamp(0, i16::MAX as i32) as i16;
    local.derivative_gain_q15 = request.allocator.roll_kd_q15.clamp(0, i16::MAX as i32) as i16;
    let mut limits = [0; 3];
    for axis in 0..3 {
        limits[axis] = request.allocator.group_authority_q12[axis]
            .iter()
            .copied()
            .sum::<i32>()
            .max(1);
    }
    Ok(AdvancedFlightConfig {
        local,
        roll_proportional_gain_q15: request.allocator.roll_kp_q15.clamp(0, i16::MAX as i32) as i16,
        roll_derivative_gain_q15: request.allocator.roll_kd_q15.clamp(0, i16::MAX as i32) as i16,
        torque_limit_q12: limits,
        fallback_density_upper_q10: 2 << 10,
        maximum_wind_q19: 20 << 19,
        minimum_sound_speed_mps: 250,
        maximum_navigation_speed_mps: 1500,
        propellant_wet_q21: request.effectors.propellant_wet_mass_q21.max(1),
        reserve_q15: request.allocator.reserve_q15,
    })
}

pub fn run_advanced_loopback(
    request: AdvancedLoopbackRequest<'_>,
) -> Result<AdvancedLoopbackEvidence, AdvancedWorldError> {
    run_advanced_loopback_observed(request, |_| {})
}

pub fn run_advanced_loopback_observed<F>(
    request: AdvancedLoopbackRequest<'_>,
    mut observer: F,
) -> Result<AdvancedLoopbackEvidence, AdvancedWorldError>
where
    F: FnMut(&AdvancedTelemetryFrame),
{
    let cfg = advanced_flight_config(&request)?;
    let allocation_cfg = allocator_config(
        request.allocator,
        request.effectors,
        [degrees_q16_to_turn16(request.capability.gimbal_limit_q16_deg); 2],
    )?;
    let mut world = AdvancedWorldEndpoint::new(
        request.vehicle,
        request.motor,
        request.mission,
        request.wind,
        request.variation,
        request.capability,
        request.effectors,
        request.faults,
    )?;
    let initial = world.snapshot().state;
    let q = initial.attitude;
    let target = [
        clamp_i16(q.x() >> 15),
        clamp_i16(q.y() >> 15),
        clamp_i16(q.z() >> 15),
    ];
    let base = AdvancedFlightComputer::new(
        cfg,
        [
            initial.position.x(),
            initial.position.y(),
            initial.position.z(),
        ],
        target,
    )
    .ok_or(AdvancedWorldError::Identity)?;
    let mut flight = AllocatedAdvancedFlightComputer::new(base, allocation_cfg)
        .ok_or(AdvancedWorldError::Identity)?;
    let mut releases = 0u32;
    let mut sensor_hash = 0x811c9dc5;
    let mut command_hash = 0x811c9dc5;
    let mut status_hash = 0x811c9dc5;
    let mut max_nav = 0;
    let mut max_att = 0i16;
    let mut max_hinge = [0; 4];
    let mut saturation = 0u32;
    let mut pulses = 0u32;
    let mut handoffs = 0u16;
    let mut fallback = 0u16;
    let mut rail_exit_epoch: Option<u16> = None;
    let mut rail_settle_error = i16::MAX;
    let mut disturbance_settle_error = i16::MAX;
    let mut last: Option<AllocatedFlightEvidence> = None;
    while !world.is_complete() {
        let Some(mut release) = world.release()? else {
            break;
        };
        let epoch = release.fast.measurement_epoch;
        if in_window(
            epoch,
            request.faults.pitot_dropout_start,
            request.faults.pitot_dropout_epochs,
        ) {
            release.fast.validity &= !ADVANCED_VALID_AIR_DATA;
        }
        let fast_missing = in_window(
            epoch,
            request.faults.fast_dropout_start,
            request.faults.fast_dropout_epochs,
        );
        let out = flight.tick(
            if fast_missing {
                None
            } else {
                Some(release.fast)
            },
            release.aid,
        );
        if !fast_missing {
            world.accept_command(out.command)?;
        }
        let mut b = [0u8; 64];
        write_advanced_fast_sensor(&release.fast, &mut b)
            .map_err(|_| AdvancedWorldError::Identity)?;
        sensor_hash = hash_bytes(sensor_hash, &b);
        if let Some(a) = release.aid {
            write_advanced_aid(&a, &mut b).map_err(|_| AdvancedWorldError::Identity)?;
            sensor_hash = hash_bytes(sensor_hash, &b);
        }
        write_advanced_command(&out.command, &mut b).map_err(|_| AdvancedWorldError::Identity)?;
        command_hash = hash_bytes(command_hash, &b);
        if let Some(s) = out.status {
            let mut sb = [0u8; 80];
            write_advanced_status(&s, &mut sb).map_err(|_| AdvancedWorldError::Identity)?;
            status_hash = hash_bytes(status_hash, &sb);
        }
        let truth = release.director.snapshot.state;
        let attitude = truth.attitude;
        observer(&AdvancedTelemetryFrame {
            time_q18: truth.time.raw(),
            epoch,
            phase: release.director.snapshot.phase as u8,
            events: release.director.snapshot.events,
            flags: release.fast.flags,
            truth_position_q13: [truth.position.x(), truth.position.y(), truth.position.z()],
            truth_velocity_q19: [truth.velocity.x(), truth.velocity.y(), truth.velocity.z()],
            attitude_q30: [attitude.w(), attitude.x(), attitude.y(), attitude.z()],
            angular_rate_q24: [
                truth.angular_rate.x(),
                truth.angular_rate.y(),
                truth.angular_rate.z(),
            ],
            navigation_position_q13: out.base.local.navigation.position_q13,
            navigation_velocity_q19: out.base.local.navigation.velocity_q19,
            dynamic_pressure_q10: release.fast.dynamic_pressure_q10,
            mach_q12: release.fast.mach_q12,
            mass_q21: release.director.snapshot.mass.mass.raw(),
            cg_q28: release.director.snapshot.mass.cg_from_nose.raw(),
            gimbal: release.fast.gimbal_applied,
            canards: release.fast.canard_applied,
            valve_mask: release.fast.valve_open_mask,
            authority_state: out.allocation.authority_state,
            propellant_q21: release.fast.propellant_q21,
            pressure_q8: release.director.physical_feedback.rcs_pressure_q8,
            supply_scale_q15: release.fast.supply_scale_q15,
            requested_torque_q12: out.allocation.requested_q12,
            achieved_torque_q12: out.allocation.achieved_q12,
            residual_torque_q12: out.allocation.residual_q12,
            hinge_q24: release.director.physical_feedback.canard_hinge_q24,
            pulse_quanta: out.command.rcs_pulse_quanta,
            alarms: out.base.local.alarms,
            saturation_count: out.allocation.saturation_count,
            checksums: [
                release.director.truth_checksum,
                sensor_hash,
                out.base.local.navigation.checksum,
                out.base.demand_checksum,
                command_hash,
                out.allocator_checksum,
                status_hash,
                out.base.command_checksum,
            ],
            rcs_force_body_q23: release.director.physical_feedback.rcs_force_body_q23,
            rcs_torque_body_q12: release.director.physical_feedback.rcs_torque_body_q12,
        });
        for axis in 0..3 {
            max_nav = max_nav.max(
                out.base.local.navigation.position_q13[axis]
                    .saturating_sub(release.director.snapshot.state.position.component(axis))
                    .saturating_abs(),
            );
        }
        max_att = max_att.max(
            release
                .fast
                .platform_angle
                .iter()
                .zip(target)
                .map(|(a, t)| a.saturating_sub(t).saturating_abs())
                .max()
                .unwrap_or(0),
        );
        let current_attitude_error = release
            .fast
            .platform_angle
            .iter()
            .zip(target)
            .map(|(a, t)| a.saturating_sub(t).saturating_abs())
            .max()
            .unwrap_or(0);
        if rail_exit_epoch.is_none() && release.fast.vehicle_status & 1 == 0 {
            rail_exit_epoch = Some(epoch);
        }
        if rail_exit_epoch.is_some_and(|exit| epoch == exit.wrapping_add(16)) {
            rail_settle_error = current_attitude_error;
        }
        if request.faults.disturbance_epoch != u16::MAX
            && epoch == request.faults.disturbance_epoch.wrapping_add(32)
        {
            disturbance_settle_error = current_attitude_error;
        }
        for i in 0..4 {
            max_hinge[i] = max_hinge[i]
                .max(release.director.physical_feedback.canard_hinge_q24[i].saturating_abs());
        }
        saturation = saturation.saturating_add(u32::from(out.allocation.saturation_count));
        pulses = pulses.saturating_add(
            out.command
                .rcs_pulse_quanta
                .iter()
                .map(|q| u32::from(*q))
                .sum::<u32>(),
        );
        if out.allocation.authority_state & 8 != 0 {
            handoffs = handoffs.saturating_add(1);
        }
        if out.base.air_data.source != AirDataSource::Pitot {
            fallback = fallback.saturating_add(1);
        }
        releases = releases.saturating_add(1);
        last = Some(out);
    }
    let result = world.result().ok_or(AdvancedWorldError::Incomplete)?;
    let last = last.ok_or(AdvancedWorldError::Incomplete)?;
    Ok(AdvancedLoopbackEvidence {
        releases,
        result,
        last,
        cell_checksum: sensor_hash ^ command_hash.rotate_left(7) ^ status_hash.rotate_left(13),
        max_navigation_error_q13: max_nav,
        max_attitude_error_turn16: max_att,
        rail_settle_error_turn16: rail_settle_error,
        disturbance_settle_error_turn16: disturbance_settle_error,
        max_hinge_q24: max_hinge,
        saturation_count: saturation,
        pulse_count: pulses,
        valve_edge_count: world.valve_edge_count(),
        depletion_count: world.depletion_count(),
        authority_handoffs: handoffs,
        air_fallback_epochs: fallback,
        rcs_final_propellant_q21: world.rcs_remaining_propellant_q21(),
        checksum_chains: [
            result.checksum,
            sensor_hash,
            last.base.local.navigation.checksum,
            last.base.demand_checksum,
            command_hash,
            last.allocator_checksum,
            status_hash,
            world.snapshot().state.time.raw() as u32,
        ],
    })
}

#[derive(Clone, Copy)]
pub struct AdvancedEffectorEvaluationRequest<'a> {
    pub vehicle: &'a SpatialVehiclePack,
    pub motor: &'a SpatialMotorPack,
    pub mission: SpatialMissionPack,
    pub wind: &'a WindProfilePack,
    pub variation: SpatialMissionVariation,
    pub variation_checksum: u32,
    pub avionics: AvionicsProfilePack,
    pub capability: ActuatorCapabilityPack,
    pub effectors: &'a AdvancedEffectorPack,
    pub allocator: &'a PriorityResidualAllocatorPack,
    pub uncertainty_identity: u32,
    pub evaluator_identity: u32,
    pub faults: AdvancedMissionFaults,
}

pub fn evaluate_with_advanced_effectors(
    request: AdvancedEffectorEvaluationRequest<'_>,
) -> Result<ksa64_core::phase9_5_contract::AdvancedEffectorEvaluationSummary, AdvancedWorldError> {
    let evidence = run_advanced_loopback(AdvancedLoopbackRequest {
        vehicle: request.vehicle,
        motor: request.motor,
        mission: request.mission,
        wind: request.wind,
        variation: request.variation,
        variation_checksum: request.variation_checksum,
        avionics: request.avionics,
        capability: request.capability,
        effectors: request.effectors,
        allocator: request.allocator,
        faults: request.faults,
    })?;
    let physical = crate::evaluation::adapt_hobby_spatial(
        request.vehicle,
        request.motor,
        request.mission,
        request.wind,
        request.variation_checksum,
        evidence.result,
    );
    Ok(
        ksa64_core::phase9_5_contract::AdvancedEffectorEvaluationSummary {
            physical,
            physical_summary_identity: ksa64_core::phase8_result::spatial_evaluation_identity(
                physical,
            ),
            avionics_identity: request.avionics.identity,
            legacy_gimbal_identity: request.allocator.legacy_gimbal_identity,
            effector_identity: request.effectors.identity,
            allocator_identity: request.allocator.identity,
            uncertainty_identity: request.uncertainty_identity,
            evaluator_identity: request.evaluator_identity,
            releases: evidence.releases,
            max_navigation_error_q13: evidence.max_navigation_error_q13,
            max_attitude_error_turn16: evidence.max_attitude_error_turn16,
            alarms: evidence.last.base.local.alarms,
            saturation_count: evidence.saturation_count,
            pulse_count: evidence.pulse_count,
            valve_edge_count: evidence.valve_edge_count,
            depletion_count: evidence.depletion_count,
            authority_handoffs: evidence.authority_handoffs,
            air_fallback_epochs: evidence.air_fallback_epochs,
            deployment_feedback: evidence.result.event_history & (EVENT_DROGUE | EVENT_MAIN),
            max_hinge_q24: evidence.max_hinge_q24,
            rcs_initial_propellant_q21: request.effectors.propellant_wet_mass_q21,
            rcs_final_propellant_q21: evidence.rcs_final_propellant_q21,
            checksum_chains: evidence.checksum_chains,
        },
    )
}

pub fn reference_capability(
    vehicle_identity: u32,
    allocator: &PriorityResidualAllocatorPack,
) -> ActuatorCapabilityPack {
    if allocator.legacy_gimbal_identity != 0 {
        let mut c = reference_gimbal_capability(vehicle_identity);
        c.identity = allocator.legacy_gimbal_identity;
        c
    } else {
        reference_monitor_capability(vehicle_identity)
    }
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
    use ksa64_core::phase8_pack::{
        parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
        parse_wind_profile_pack,
    };
    use ksa64_core::phase9_5_contract::{parse_allocator_pack, parse_effector_pack};
    #[test]
    fn canard_reference_completes_with_exact_release_clock() {
        let vehicle =
            parse_spatial_vehicle_pack(include_bytes!("../../phase9_5/examples/firestorm-c9.kvp8"))
                .unwrap();
        let motor =
            parse_spatial_motor_pack(include_bytes!("../../phase8/examples/aerotech-i211w.kmp8"))
                .unwrap();
        let mut mission =
            parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
                .unwrap();
        mission.vehicle_identity = vehicle.identity;
        mission.identity ^= vehicle.identity;
        let wind =
            parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
                .unwrap();
        let effectors =
            parse_effector_pack(include_bytes!("../../phase9_5/examples/firestorm-c9.kpe9"))
                .unwrap();
        let allocator =
            parse_allocator_pack(include_bytes!("../../phase9_5/examples/firestorm-c9.kpa9"))
                .unwrap();
        let capability = reference_capability(vehicle.identity, &allocator);
        let evidence = run_advanced_loopback(AdvancedLoopbackRequest {
            vehicle: &vehicle,
            motor: &motor,
            mission,
            wind: &wind,
            variation: SpatialMissionVariation::NOMINAL,
            variation_checksum: 0,
            avionics: crate::phase8_5::reference_avionics_profile(false),
            capability,
            effectors: &effectors,
            allocator: &allocator,
            faults: AdvancedMissionFaults::NOMINAL,
        })
        .unwrap();
        assert_eq!(
            evidence.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact
        );
        assert_eq!(
            evidence.result.event_history & (EVENT_DROGUE | EVENT_MAIN),
            EVENT_DROGUE | EVENT_MAIN
        );
        assert!(evidence.releases > 100);
    }

    fn run_reference(
        vehicle_bytes: &[u8],
        effector_bytes: &[u8],
        allocator_bytes: &[u8],
        faults: AdvancedMissionFaults,
    ) -> AdvancedLoopbackEvidence {
        let vehicle = parse_spatial_vehicle_pack(vehicle_bytes).unwrap();
        let motor =
            parse_spatial_motor_pack(include_bytes!("../../phase8/examples/aerotech-i211w.kmp8"))
                .unwrap();
        let mut mission =
            parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
                .unwrap();
        mission.vehicle_identity = vehicle.identity;
        mission.identity ^= vehicle.identity;
        let wind =
            parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
                .unwrap();
        let effectors = parse_effector_pack(effector_bytes).unwrap();
        let allocator = parse_allocator_pack(allocator_bytes).unwrap();
        let capability = reference_capability(vehicle.identity, &allocator);
        run_advanced_loopback(AdvancedLoopbackRequest {
            vehicle: &vehicle,
            motor: &motor,
            mission,
            wind: &wind,
            variation: SpatialMissionVariation::NOMINAL,
            variation_checksum: 0,
            avionics: crate::phase8_5::reference_avionics_profile(false),
            capability,
            effectors: &effectors,
            allocator: &allocator,
            faults,
        })
        .unwrap()
    }

    #[test]
    fn rcs_and_mixed_reference_missions_complete() {
        let rcs = run_reference(
            include_bytes!("../../phase9_5/examples/firestorm-r9.kvp8"),
            include_bytes!("../../phase9_5/examples/firestorm-r9.kpe9"),
            include_bytes!("../../phase9_5/examples/firestorm-r9.kpa9"),
            {
                let mut f = AdvancedMissionFaults::NOMINAL;
                f.disturbance_epoch = 256;
                f.disturbance_angular_rate_q24 = [1 << 22, -(1 << 22), 1 << 22];
                f
            },
        );
        let mixed = run_reference(
            include_bytes!("../../phase9_5/examples/firestorm-m9.kvp8"),
            include_bytes!("../../phase9_5/examples/firestorm-m9.kpe9"),
            include_bytes!("../../phase9_5/examples/firestorm-m9.kpa9"),
            AdvancedMissionFaults::NOMINAL,
        );
        assert_eq!(
            rcs.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact,
            "rcs pulses {} maxatt {} checksum {:08x} time {} phase {:?} steps {}",
            rcs.pulse_count,
            rcs.max_attitude_error_turn16,
            rcs.result.checksum,
            rcs.result.final_snapshot.state.time.raw(),
            rcs.result.final_snapshot.phase,
            rcs.result.steps
        );
        assert_eq!(
            mixed.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact,
            "mixed pulses {} maxatt {} checksum {:08x}",
            mixed.pulse_count,
            mixed.max_attitude_error_turn16,
            mixed.result.checksum
        );
        for evidence in [rcs, mixed] {
            assert_eq!(
                evidence.result.event_history & (EVENT_DROGUE | EVENT_MAIN),
                EVENT_DROGUE | EVENT_MAIN
            );
            assert!(evidence.releases > 100);
        }
        assert!(rcs.pulse_count > 0);
        assert!(mixed.rcs_final_propellant_q21 >= 0);
    }

    #[test]
    fn pitot_loss_uses_truth_blind_conservative_fallback() {
        let mut faults = AdvancedMissionFaults::NOMINAL;
        faults.pitot_dropout_start = 32;
        faults.pitot_dropout_epochs = 64;
        let evidence = run_reference(
            include_bytes!("../../phase9_5/examples/firestorm-m9.kvp8"),
            include_bytes!("../../phase9_5/examples/firestorm-m9.kpe9"),
            include_bytes!("../../phase9_5/examples/firestorm-m9.kpa9"),
            faults,
        );
        assert!(evidence.air_fallback_epochs >= 64);
        assert_eq!(
            evidence.result.outcome,
            ksa64_core::evaluation::EvaluationOutcome::GroundContact
        );
    }
}
