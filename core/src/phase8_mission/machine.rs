//! Phase 8 mission state and result retention.

use crate::evaluation::EvaluationOutcome;
use crate::numeric::NumericStatus;
use crate::phase8_numeric::{
    EnuPosition, SpatialPosition, SpatialTime, SPATIAL_COAST_TRANSLATION_STEP,
    SPATIAL_POWERED_STEP, SPATIAL_RECOVERY_STEP,
};
use crate::phase8_pack::{
    packs_are_compatible, SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindProfilePack,
};
use crate::phase8_world::{
    attitude_from_rail_axis, rail_axis_from_mission, rail_exit_distance, HobbySpatialState,
    RailState,
};
use crate::spatial_numeric::FixedVec3;

use super::propulsion::derive_mass_properties;
use super::{
    HobbySpatialPhase, Phase8Milestone, Phase8MissionError, Phase8MissionResult,
    Phase8MissionSnapshot, SpatialAeroState, SpatialMissionVariation,
};

pub struct Phase8MissionMachine<'a> {
    pub(super) vehicle: &'a SpatialVehiclePack,
    pub(super) motor: &'a SpatialMotorPack,
    pub(super) mission: SpatialMissionPack,
    pub(super) wind: &'a WindProfilePack,
    pub(super) variation: SpatialMissionVariation,
    pub(super) snapshot: Phase8MissionSnapshot,
    pub(super) rail: RailState,
    pub(super) rail_axis: FixedVec3<30>,
    pub(super) rail_exit_distance: SpatialPosition,
    pub(super) deployment_started_raw: i32,
    pub(super) previous_vertical_velocity: i32,
    pub(super) rail_exit: Phase8Milestone,
    pub(super) burnout: Phase8Milestone,
    pub(super) apogee: Phase8Milestone,
    pub(super) drogue: Phase8Milestone,
    pub(super) main: Phase8Milestone,
    pub(super) landing: Phase8Milestone,
    pub(super) max_altitude: i32,
    pub(super) max_speed: i32,
    pub(super) max_acceleration: i32,
    pub(super) max_q: i32,
    pub(super) max_aoa: i32,
    pub(super) max_rate: i32,
    pub(super) max_wind: i32,
    pub(super) min_static_margin: i32,
    pub(super) rail_exit_static_margin: i32,
    pub(super) burnout_static_margin: i32,
    pub(super) max_lateral_acceleration: i32,
    pub(super) steps: u32,
    pub(super) checksum: u32,
    pub(super) terminal_outcome: Option<EvaluationOutcome>,
}

impl<'a> Phase8MissionMachine<'a> {
    pub fn new(
        vehicle: &'a SpatialVehiclePack,
        motor: &'a SpatialMotorPack,
        mission: SpatialMissionPack,
        wind: &'a WindProfilePack,
    ) -> Result<Self, Phase8MissionError> {
        Self::new_with_variation(
            vehicle,
            motor,
            mission,
            wind,
            SpatialMissionVariation::NOMINAL,
        )
    }

    pub fn new_with_variation(
        vehicle: &'a SpatialVehiclePack,
        motor: &'a SpatialMotorPack,
        mission: SpatialMissionPack,
        wind: &'a WindProfilePack,
        variation: SpatialMissionVariation,
    ) -> Result<Self, Phase8MissionError> {
        if !packs_are_compatible(vehicle, motor, mission, wind) || !variation.is_valid() {
            return Err(Phase8MissionError::Configuration);
        }
        let mut status = NumericStatus::CLEAR;
        let rail_axis = rail_axis_from_mission(mission, &mut status)
            .map_err(|_| Phase8MissionError::Configuration)?;
        let attitude = attitude_from_rail_axis(rail_axis, &mut status)
            .map_err(|_| Phase8MissionError::Numeric)?;
        let mass = derive_mass_properties(
            vehicle,
            motor,
            motor.propellant_mass,
            variation.mass_scale_ppm,
            &mut status,
        );
        if !status.is_clear() {
            return Err(Phase8MissionError::Numeric);
        }
        let state = HobbySpatialState::at_rest(
            EnuPosition::new(0, 0, mission.launch_altitude.raw()),
            attitude,
        );
        let snapshot = Phase8MissionSnapshot {
            state,
            phase: HobbySpatialPhase::ConstrainedPowered,
            events: 0,
            mass,
            thrust_q13: 0,
            aero: SpatialAeroState::ZERO,
            wind_q22: [0; 3],
        };
        Ok(Self {
            vehicle,
            motor,
            mission,
            wind,
            variation,
            snapshot,
            rail: RailState::REST,
            rail_axis,
            rail_exit_distance: rail_exit_distance(
                mission.rail_length,
                vehicle.aft_rail_guide_from_tail,
            ),
            deployment_started_raw: 0,
            previous_vertical_velocity: 0,
            rail_exit: Phase8Milestone::ZERO,
            burnout: Phase8Milestone::ZERO,
            apogee: Phase8Milestone::ZERO,
            drogue: Phase8Milestone::ZERO,
            main: Phase8Milestone::ZERO,
            landing: Phase8Milestone::ZERO,
            max_altitude: state.position.z(),
            max_speed: 0,
            max_acceleration: 0,
            max_q: 0,
            max_aoa: 0,
            max_rate: 0,
            max_wind: 0,
            min_static_margin: i32::MAX,
            rail_exit_static_margin: 0,
            burnout_static_margin: 0,
            max_lateral_acceleration: 0,
            steps: 0,
            checksum: 0x811c_9dc5,
            terminal_outcome: None,
        })
    }

    pub const fn snapshot(&self) -> Phase8MissionSnapshot {
        self.snapshot
    }

    pub const fn is_complete(&self) -> bool {
        self.terminal_outcome.is_some()
    }

    pub(super) fn timestep(&self) -> Result<SpatialTime, Phase8MissionError> {
        match self.snapshot.phase {
            HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight => {
                Ok(SPATIAL_POWERED_STEP)
            }
            HobbySpatialPhase::Coast => Ok(SPATIAL_COAST_TRANSLATION_STEP),
            HobbySpatialPhase::DrogueRecovery | HobbySpatialPhase::MainRecovery => {
                Ok(SPATIAL_RECOVERY_STEP)
            }
            HobbySpatialPhase::Complete | HobbySpatialPhase::Failed => {
                Err(Phase8MissionError::Complete)
            }
        }
    }

    pub(super) fn fail(
        &mut self,
        outcome: EvaluationOutcome,
        error: Phase8MissionError,
    ) -> Phase8MissionError {
        self.snapshot.phase = HobbySpatialPhase::Failed;
        self.terminal_outcome = Some(outcome);
        error
    }

    pub fn result(&self) -> Option<Phase8MissionResult> {
        let outcome = self.terminal_outcome?;
        Some(Phase8MissionResult {
            outcome,
            steps: self.steps,
            final_snapshot: self.snapshot,
            rail_exit: self.rail_exit,
            burnout: self.burnout,
            apogee: self.apogee,
            drogue: self.drogue,
            main: self.main,
            landing: self.landing,
            max_altitude_raw_q13: self.max_altitude,
            max_speed_raw_q19: self.max_speed,
            max_acceleration_raw_q19: self.max_acceleration,
            max_dynamic_pressure_raw_q13: self.max_q,
            max_aoa_raw_q28: self.max_aoa,
            max_angular_rate_raw_q24: self.max_rate,
            max_wind_raw_q22: self.max_wind,
            minimum_static_margin_raw_q24: self.min_static_margin,
            rail_exit_static_margin_raw_q24: self.rail_exit_static_margin,
            burnout_static_margin_raw_q24: self.burnout_static_margin,
            max_lateral_acceleration_raw_q19: self.max_lateral_acceleration,
            checksum: self.checksum,
        })
    }
}
