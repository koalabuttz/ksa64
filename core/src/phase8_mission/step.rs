//! Phase transitions, extrema, and complete Phase 8 mission execution.

use crate::evaluation::EvaluationOutcome;
use crate::numeric::NumericStatus;
use crate::phase8_numeric::{BodyAngularRate, EnuPosition};
use crate::phase8_pack::{
    SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindProfilePack,
};

use super::advance::{advance, advance_controlled};
use super::machine::Phase8MissionMachine;
use super::{
    magnitude3_i32, phase8_snapshot_checksum, HobbySpatialPhase, Phase85AppliedControl,
    Phase85DeploymentCommand, Phase8Milestone, Phase8MissionError, Phase8MissionResult,
    Phase8MissionSnapshot, SpatialMissionVariation, EVENT_APOGEE, EVENT_BURNOUT, EVENT_DROGUE,
    EVENT_LANDING, EVENT_MAIN, EVENT_RAIL_EXIT,
};

impl Phase8MissionMachine<'_> {
    pub fn step(&mut self) -> Result<Phase8MissionSnapshot, Phase8MissionError> {
        if self.is_complete() {
            return Err(Phase8MissionError::Complete);
        }
        if self.snapshot.state.time.raw() >= self.mission.max_mission_time.raw() {
            self.fail(
                EvaluationOutcome::RecoveryIncomplete,
                Phase8MissionError::Complete,
            );
            return Err(Phase8MissionError::Complete);
        }
        let phase = self.snapshot.phase;
        let timestep = self.timestep()?;
        let advanced = match advance(self, timestep) {
            Ok(value) => value,
            Err(Phase8MissionError::ModelEnvelopeExceeded) => {
                return Err(self.fail(
                    EvaluationOutcome::ModelEnvelopeExceeded,
                    Phase8MissionError::ModelEnvelopeExceeded,
                ));
            }
            Err(_) => {
                return Err(self.fail(EvaluationOutcome::NumericFault, Phase8MissionError::Numeric));
            }
        };
        let mut successor = advanced.state;
        let mut events = 0u16;
        let mut next_phase = phase;

        if phase == HobbySpatialPhase::ConstrainedPowered
            && self.rail.distance.raw() >= self.rail_exit_distance.raw()
        {
            next_phase = HobbySpatialPhase::PoweredFlight;
            events |= EVENT_RAIL_EXIT;
            self.rail_exit = Phase8Milestone::from_state(successor);
        }
        if matches!(
            phase,
            HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight
        ) && successor.time.raw() >= self.motor.burn_time.raw()
        {
            next_phase = HobbySpatialPhase::Coast;
            events |= EVENT_BURNOUT;
            self.burnout = Phase8Milestone::from_state(successor);
        }
        if phase == HobbySpatialPhase::Coast
            && self.previous_vertical_velocity >= 0
            && successor.velocity.z() < 0
        {
            next_phase = HobbySpatialPhase::DrogueRecovery;
            events |= EVENT_APOGEE | EVENT_DROGUE;
            self.apogee = Phase8Milestone::from_state(successor);
            self.drogue = self.apogee;
            self.deployment_started_raw = successor.time.raw();
            successor.angular_rate = BodyAngularRate::ZERO;
        }
        if phase == HobbySpatialPhase::DrogueRecovery
            && successor.velocity.z() < 0
            && successor.position.z() <= self.mission.main_deployment_altitude.raw()
        {
            next_phase = HobbySpatialPhase::MainRecovery;
            events |= EVENT_MAIN;
            self.main = Phase8Milestone::from_state(successor);
            self.deployment_started_raw = successor.time.raw();
        }
        if matches!(
            phase,
            HobbySpatialPhase::DrogueRecovery | HobbySpatialPhase::MainRecovery
        ) && successor.position.z() <= self.mission.launch_altitude.raw()
            && successor.velocity.z() < 0
        {
            successor.position = EnuPosition::new(
                successor.position.x(),
                successor.position.y(),
                self.mission.launch_altitude.raw(),
            );
            next_phase = HobbySpatialPhase::Complete;
            events |= EVENT_LANDING;
            self.landing = Phase8Milestone::from_state(successor);
            self.terminal_outcome = Some(EvaluationOutcome::GroundContact);
        }
        self.previous_vertical_velocity = successor.velocity.z();
        self.event_history |= events;
        self.steps = self.steps.saturating_add(1);
        self.snapshot = Phase8MissionSnapshot {
            state: successor,
            phase: next_phase,
            events,
            mass: advanced.mass,
            thrust_q13: advanced.thrust_q13,
            aero: advanced.aero,
            wind_q22: [
                advanced.environment.wind.total.x(),
                advanced.environment.wind.total.y(),
                advanced.environment.wind.total.z(),
            ],
        };
        self.update_extrema(advanced.environment);
        if events & EVENT_RAIL_EXIT != 0 {
            self.rail_exit_static_margin = self.snapshot.aero.static_margin_q24;
        }
        if events & EVENT_BURNOUT != 0 {
            self.burnout_static_margin = self.snapshot.aero.static_margin_q24;
        }
        self.checksum = phase8_snapshot_checksum(self.checksum, self.snapshot);
        Ok(self.snapshot)
    }

    /// Advance the separately identified Phase 8.5 world by one exact segment.
    /// Recovery transitions occur only from accepted avionics commands.
    pub fn step_avionics(
        &mut self,
        timestep: crate::phase8_numeric::SpatialTime,
        control: Phase85AppliedControl,
        deployment: Phase85DeploymentCommand,
    ) -> Result<Phase8MissionSnapshot, Phase8MissionError> {
        if self.is_complete() {
            return Err(Phase8MissionError::Complete);
        }
        let maximum = self.timestep()?;
        if timestep.raw() <= 0 || timestep.raw() > maximum.raw() {
            return Err(Phase8MissionError::Configuration);
        }
        if self
            .snapshot
            .state
            .time
            .raw()
            .saturating_add(timestep.raw())
            > self.mission.max_mission_time.raw()
        {
            return Err(Phase8MissionError::Configuration);
        }
        let phase = self.snapshot.phase;
        let advanced = match advance_controlled(self, timestep, control, true) {
            Ok(value) => value,
            Err(Phase8MissionError::ModelEnvelopeExceeded) => {
                let outcome = if self.event_history & EVENT_APOGEE != 0
                    && self.event_history & EVENT_DROGUE == 0
                {
                    EvaluationOutcome::RecoveryIncomplete
                } else {
                    EvaluationOutcome::ModelEnvelopeExceeded
                };
                return Err(self.fail(outcome, Phase8MissionError::ModelEnvelopeExceeded));
            }
            Err(_) => {
                return Err(self.fail(EvaluationOutcome::NumericFault, Phase8MissionError::Numeric))
            }
        };
        let mut successor = advanced.state;
        let mut events = 0u16;
        let mut next_phase = phase;
        if phase == HobbySpatialPhase::ConstrainedPowered
            && self.rail.distance.raw() >= self.rail_exit_distance.raw()
        {
            next_phase = HobbySpatialPhase::PoweredFlight;
            events |= EVENT_RAIL_EXIT;
            self.rail_exit = Phase8Milestone::from_state(successor);
        }
        if matches!(
            phase,
            HobbySpatialPhase::ConstrainedPowered | HobbySpatialPhase::PoweredFlight
        ) && successor.time.raw() >= self.motor.burn_time.raw()
        {
            next_phase = HobbySpatialPhase::Coast;
            events |= EVENT_BURNOUT;
            self.burnout = Phase8Milestone::from_state(successor);
        }
        if phase == HobbySpatialPhase::Coast
            && self.previous_vertical_velocity >= 0
            && successor.velocity.z() < 0
        {
            events |= EVENT_APOGEE;
            self.apogee = Phase8Milestone::from_state(successor);
        }
        if phase == HobbySpatialPhase::Coast
            && deployment.drogue
            && self.event_history & EVENT_BURNOUT != 0
        {
            next_phase = HobbySpatialPhase::DrogueRecovery;
            events |= EVENT_DROGUE;
            self.drogue = Phase8Milestone::from_state(successor);
            self.deployment_started_raw = successor.time.raw();
            successor.angular_rate = BodyAngularRate::ZERO;
        }
        if phase == HobbySpatialPhase::DrogueRecovery && deployment.main {
            next_phase = HobbySpatialPhase::MainRecovery;
            events |= EVENT_MAIN;
            self.main = Phase8Milestone::from_state(successor);
            self.deployment_started_raw = successor.time.raw();
        }
        if matches!(
            phase,
            HobbySpatialPhase::DrogueRecovery | HobbySpatialPhase::MainRecovery
        ) && successor.position.z() <= self.mission.launch_altitude.raw()
            && successor.velocity.z() < 0
        {
            successor.position = EnuPosition::new(
                successor.position.x(),
                successor.position.y(),
                self.mission.launch_altitude.raw(),
            );
            next_phase = HobbySpatialPhase::Complete;
            events |= EVENT_LANDING;
            self.landing = Phase8Milestone::from_state(successor);
            self.terminal_outcome = Some(EvaluationOutcome::GroundContact);
        }
        if successor.time.raw() >= self.mission.max_mission_time.raw()
            && next_phase != HobbySpatialPhase::Complete
        {
            next_phase = HobbySpatialPhase::Failed;
            self.terminal_outcome = Some(EvaluationOutcome::RecoveryIncomplete);
        }
        self.previous_vertical_velocity = successor.velocity.z();
        self.event_history |= events;
        self.steps = self.steps.saturating_add(1);
        self.snapshot = Phase8MissionSnapshot {
            state: successor,
            phase: next_phase,
            events,
            mass: advanced.mass,
            thrust_q13: advanced.thrust_q13,
            aero: advanced.aero,
            wind_q22: [
                advanced.environment.wind.total.x(),
                advanced.environment.wind.total.y(),
                advanced.environment.wind.total.z(),
            ],
        };
        self.update_extrema(advanced.environment);
        if events & EVENT_RAIL_EXIT != 0 {
            self.rail_exit_static_margin = self.snapshot.aero.static_margin_q24;
        }
        if events & EVENT_BURNOUT != 0 {
            self.burnout_static_margin = self.snapshot.aero.static_margin_q24;
        }
        self.checksum = phase8_snapshot_checksum(self.checksum, self.snapshot);
        Ok(self.snapshot)
    }

    fn update_extrema(&mut self, environment: crate::phase8_world::HobbySpatialEnvironment) {
        self.max_altitude = self.max_altitude.max(self.snapshot.state.position.z());
        let mut status = NumericStatus::CLEAR;
        self.max_speed = self
            .max_speed
            .max(magnitude3_i32(self.snapshot.state.velocity, &mut status));
        self.max_acceleration = self.max_acceleration.max(magnitude3_i32(
            self.snapshot.state.acceleration,
            &mut status,
        ));
        self.max_q = self.max_q.max(self.snapshot.aero.dynamic_pressure_q13);
        self.max_aoa = self
            .max_aoa
            .max(self.snapshot.aero.angle_of_attack_q28.abs());
        self.max_rate = self.max_rate.max(magnitude3_i32(
            self.snapshot.state.angular_rate,
            &mut status,
        ));
        if self.snapshot.aero.dynamic_pressure_q13 >= super::forces::MIN_DIRECTIONAL_AERO_Q13 {
            self.min_static_margin = self
                .min_static_margin
                .min(self.snapshot.aero.static_margin_q24);
        }
        let lateral = crate::numeric::magnitude3_floor(
            self.snapshot.state.acceleration.x(),
            self.snapshot.state.acceleration.y(),
            0,
            &mut status,
        )
        .min(i32::MAX as u32) as i32;
        self.max_lateral_acceleration = self.max_lateral_acceleration.max(lateral);
        self.max_wind = self
            .max_wind
            .max(magnitude3_i32(environment.wind.total, &mut status));
    }
}

pub fn run_phase8_mission(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
    variation: SpatialMissionVariation,
) -> Result<Phase8MissionResult, Phase8MissionError> {
    let mut machine =
        Phase8MissionMachine::new_with_variation(vehicle, motor, mission, wind, variation)?;
    while !machine.is_complete() {
        match machine.step() {
            Ok(_) | Err(Phase8MissionError::Complete) => {}
            Err(Phase8MissionError::ModelEnvelopeExceeded) if machine.is_complete() => {}
            Err(error) => return Err(error),
        }
    }
    machine.result().ok_or(Phase8MissionError::Numeric)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase8_format::KWP8_MAX_WIND_KNOTS;
    use crate::phase8_numeric::{SpatialPosition, SpatialTime, SpatialWind};
    use crate::phase8_pack::{
        parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
        parse_wind_profile_pack, WindKnot,
    };

    fn fixtures() -> (
        SpatialVehiclePack,
        SpatialMotorPack,
        SpatialMissionPack,
        WindProfilePack,
    ) {
        (
            parse_spatial_vehicle_pack(include_bytes!("../../../phase8/examples/firestorm54.kvp8"))
                .unwrap(),
            parse_spatial_motor_pack(include_bytes!(
                "../../../phase8/examples/aerotech-i211w.kmp8"
            ))
            .unwrap(),
            parse_spatial_mission_pack(include_bytes!(
                "../../../phase8/examples/firestorm-i211.kmc8"
            ))
            .unwrap(),
            parse_wind_profile_pack(include_bytes!(
                "../../../phase8/examples/firestorm-calm.kwp8"
            ))
            .unwrap(),
        )
    }

    fn crosswind(
        _base: WindProfilePack,
        identity: u32,
        speed_mps: i32,
        layered: bool,
        gust: bool,
    ) -> WindProfilePack {
        let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
        knots[0] = WindKnot {
            altitude: SpatialPosition::ZERO,
            east: SpatialWind::from_raw(speed_mps << 22),
            north: SpatialWind::ZERO,
        };
        knots[1] = WindKnot {
            altitude: SpatialPosition::from_raw(1_000 << 13),
            east: SpatialWind::from_raw((speed_mps + if layered { 2 } else { 0 }) << 22),
            north: SpatialWind::ZERO,
        };
        WindProfilePack {
            identity,
            gust_seed: if gust { 0x1020_3040 } else { 0 },
            gust_cadence: SpatialTime::from_raw(1 << 18),
            gust_amplitude_east: SpatialWind::from_raw((if gust { 1 } else { 0 }) << 22),
            gust_amplitude_north: SpatialWind::from_raw((if gust { 1 } else { 0 }) << 22),
            max_gust: SpatialWind::from_raw((if gust { 2 } else { 0 }) << 22),
            knot_count: 2,
            knots,
        }
    }

    #[test]
    fn crosswind_layered_and_gust_missions_are_repeatable() {
        let (vehicle, motor, mission, calm) = fixtures();
        for (index, (speed_mps, layered, gust)) in [
            (2, false, false),
            (2, true, false),
            (2, true, true),
            (5, false, false),
        ]
        .into_iter()
        .enumerate()
        {
            let wind = crosswind(calm, 0x8000_0000 + index as u32, speed_mps, layered, gust);
            let bound = SpatialMissionPack {
                wind_identity: wind.identity,
                ..mission
            };
            let a = run_phase8_mission(
                &vehicle,
                &motor,
                bound,
                &wind,
                SpatialMissionVariation::NOMINAL,
            )
            .unwrap();
            let b = run_phase8_mission(
                &vehicle,
                &motor,
                bound,
                &wind,
                SpatialMissionVariation::NOMINAL,
            )
            .unwrap();
            assert_eq!(a, b);
            assert_eq!(
                a.outcome,
                EvaluationOutcome::GroundContact,
                "case {index}: {a:?}"
            );
            assert!(a.final_snapshot.state.position.x() > 0);
            assert!(a.max_wind_raw_q22 >= speed_mps << 22);
        }
    }
    #[test]
    fn cardinal_crosswinds_are_symmetric_and_complete() {
        let (vehicle, motor, mission, _) = fixtures();
        for (index, (east, north)) in [(2, 0), (-2, 0), (0, 2), (0, -2)].into_iter().enumerate() {
            let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
            knots[0] = WindKnot {
                altitude: SpatialPosition::ZERO,
                east: SpatialWind::from_raw(east << 22),
                north: SpatialWind::from_raw(north << 22),
            };
            knots[1] = WindKnot {
                altitude: SpatialPosition::from_raw(1_000 << 13),
                east: SpatialWind::from_raw(east << 22),
                north: SpatialWind::from_raw(north << 22),
            };
            let wind = WindProfilePack {
                identity: 0x8100_0000 + index as u32,
                gust_seed: 0,
                gust_cadence: SpatialTime::from_raw(1 << 18),
                gust_amplitude_east: SpatialWind::ZERO,
                gust_amplitude_north: SpatialWind::ZERO,
                max_gust: SpatialWind::ZERO,
                knot_count: 2,
                knots,
            };
            let bound = SpatialMissionPack {
                wind_identity: wind.identity,
                ..mission
            };
            let mut machine = Phase8MissionMachine::new(&vehicle, &motor, bound, &wind).unwrap();
            while !machine.is_complete() {
                match machine.step() {
                    Ok(_) | Err(Phase8MissionError::Complete) => {}
                    Err(error) => panic!(
                        "cardinal case {index} at {:?}: {error:?}",
                        machine.snapshot()
                    ),
                }
            }
            let result = machine.result().unwrap();
            assert_eq!(
                result.outcome,
                EvaluationOutcome::GroundContact,
                "cardinal case {index}: {result:?}"
            );
        }
    }
    #[test]
    fn nominal_firestorm_completes_repeatably() {
        let (vehicle, motor, mission, wind) = fixtures();
        let a = run_phase8_mission(
            &vehicle,
            &motor,
            mission,
            &wind,
            SpatialMissionVariation::NOMINAL,
        )
        .unwrap();
        let b = run_phase8_mission(
            &vehicle,
            &motor,
            mission,
            &wind,
            SpatialMissionVariation::NOMINAL,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.outcome, EvaluationOutcome::GroundContact);
        assert!(a.rail_exit.time.raw() > 0);
        assert!(a.burnout.time > a.rail_exit.time);
        assert!(a.apogee.time > a.burnout.time);
        assert!(a.landing.time > a.main.time);
        assert!(a.max_altitude_raw_q13 > 100 << 13);
    }

    #[test]
    fn unstable_and_recovery_incomplete_fail_closed() {
        let (vehicle, motor, mission, wind) = fixtures();
        let unstable = SpatialMissionVariation {
            cp_offset_q28: -(1 << 28),
            ..SpatialMissionVariation::NOMINAL
        };
        let result = run_phase8_mission(&vehicle, &motor, mission, &wind, unstable).unwrap();
        assert_eq!(result.outcome, EvaluationOutcome::ModelEnvelopeExceeded);
        let short = SpatialMissionPack {
            max_mission_time: SpatialTime::from_raw(5 << 18),
            ..mission
        };
        let result = run_phase8_mission(
            &vehicle,
            &motor,
            short,
            &wind,
            SpatialMissionVariation::NOMINAL,
        )
        .unwrap();
        assert_eq!(result.outcome, EvaluationOutcome::RecoveryIncomplete);
    }
}
