//! Reusable one-step planar vehicle world for Phase 3 composition.

use ksa64_core::aerodynamics::{evaluate_aerodynamics, AeroConfig};
use ksa64_core::numeric::{add, multiply_scaled, subtract, NumericStatus};
use ksa64_core::phase2_mission::{
    hash_planar_truth, EVENT_CUTOFF, EVENT_IGNITION, EVENT_SEPARATION, PLANAR_CHECKSUM_OFFSET,
};
use ksa64_core::phase2_quantities::{DynamicPressure, Mach, PitchAngle, ReferenceArea};
use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_core::planar::{PlanarTruthState, PlanarWorld, StagePhase};
use ksa64_core::planar_dynamics::{advance_planar_state, evaluate_planar_forces_phase3};
use ksa64_core::planar_environment::RotatingEarthEnvironment;
use ksa64_core::quantities::{Density, Force, Mass};
use ksa64_interface::EngineAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldError {
    Configuration,
    NumericFault,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldCommand {
    pub pitch: u16,
    pub engine_action: EngineAction,
    pub separate: bool,
    pub abort_safeing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldSnapshot {
    pub truth: PlanarTruthState,
    pub pitch: PitchAngle,
    pub mach: Mach,
    pub dynamic_pressure: DynamicPressure,
    pub events: u16,
    pub truth_checksum: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldParameters {
    pub payload_mass_ppm: i32,
    pub stage_thrust_ppm: [i32; 2],
    pub atmosphere_density_ppm: i32,
    pub drag_ppm: i32,
}
impl WorldParameters {
    pub const DEFAULT: Self = Self {
        payload_mass_ppm: 0,
        stage_thrust_ppm: [0, 0],
        atmosphere_density_ppm: 0,
        drag_ppm: 0,
    };
    pub const fn is_valid(self) -> bool {
        self.payload_mass_ppm >= -500_000
            && self.payload_mass_ppm <= 500_000
            && self.stage_thrust_ppm[0] >= -500_000
            && self.stage_thrust_ppm[0] <= 500_000
            && self.stage_thrust_ppm[1] >= -500_000
            && self.stage_thrust_ppm[1] <= 500_000
            && self.atmosphere_density_ppm >= -500_000
            && self.atmosphere_density_ppm <= 500_000
            && self.drag_ppm >= -500_000
            && self.drag_ppm <= 500_000
    }
}

fn scale_ppm(value: i32, delta_ppm: i32) -> Option<i32> {
    if delta_ppm == 0 {
        return Some(value);
    }
    let scaled = (value as i64 * (1_000_000i64 + delta_ppm as i64)) / 1_000_000i64;
    if scaled < i32::MIN as i64 || scaled > i32::MAX as i64 {
        None
    } else {
        Some(scaled as i32)
    }
}

pub struct WorldMachine<'a> {
    scenario: &'a Phase2Scenario,
    world: PlanarWorld,
    environment: RotatingEarthEnvironment,
    truth: PlanarTruthState,
    phase_steps: u32,
    burn_steps: u32,
    truth_checksum: u32,
    parameters: WorldParameters,
}

impl<'a> WorldMachine<'a> {
    pub fn new_compatibility(scenario: &'a Phase2Scenario) -> Result<Self, WorldError> {
        Self::new_compatibility_parameterized(scenario, WorldParameters::DEFAULT)
    }

    pub fn new_compatibility_parameterized(
        scenario: &'a Phase2Scenario,
        parameters: WorldParameters,
    ) -> Result<Self, WorldError> {
        if !parameters.is_valid() {
            return Err(WorldError::Configuration);
        }
        let mut status = NumericStatus::CLEAR;
        let mut truth = scenario
            .initial_truth(&mut status)
            .ok_or(WorldError::NumericFault)?;
        if parameters.payload_mass_ppm != 0 {
            let nominal_payload = scenario.payload_mass().raw();
            let varied_payload = scale_ppm(nominal_payload, parameters.payload_mass_ppm)
                .ok_or(WorldError::NumericFault)?;
            let varied_mass = add(
                truth.total_mass().raw(),
                varied_payload - nominal_payload,
                &mut status,
            );
            truth = truth.with_vehicle_state(
                Mass::from_raw(varied_mass),
                truth.active_propellant(),
                truth.active_stage(),
                truth.stage_phase(),
            );
        }
        if !status.is_clear() {
            return Err(WorldError::NumericFault);
        }
        Ok(Self {
            scenario,
            world: PlanarWorld::simple_earth(scenario.timestep()),
            environment: RotatingEarthEnvironment::new(),
            truth,
            phase_steps: 0,
            burn_steps: 0,
            truth_checksum: PLANAR_CHECKSUM_OFFSET,
            parameters,
        })
    }

    pub fn new_commanded(scenario: &'a Phase2Scenario) -> Result<Self, WorldError> {
        Self::new_commanded_parameterized(scenario, WorldParameters::DEFAULT)
    }

    pub fn new_commanded_parameterized(
        scenario: &'a Phase2Scenario,
        parameters: WorldParameters,
    ) -> Result<Self, WorldError> {
        let mut machine = Self::new_compatibility_parameterized(scenario, parameters)?;
        machine.truth = machine.truth.with_vehicle_state(
            machine.truth.total_mass(),
            machine.truth.active_propellant(),
            machine.truth.active_stage(),
            StagePhase::CoastBeforeIgnition,
        );
        Ok(machine)
    }

    pub const fn truth(&self) -> PlanarTruthState {
        self.truth
    }
    pub const fn truth_checksum(&self) -> u32 {
        self.truth_checksum
    }

    fn integrate(
        &mut self,
        pitch: PitchAngle,
    ) -> Result<(PlanarTruthState, Mach, DynamicPressure), WorldError> {
        let mut status = NumericStatus::CLEAR;
        let stage = self
            .scenario
            .stage(self.truth.active_stage())
            .ok_or(WorldError::Configuration)?;
        let table = self
            .scenario
            .aero_table(stage.aero_table_index())
            .ok_or(WorldError::Configuration)?;
        let mut sample = self.environment.sample(self.truth.radius(), &mut status);
        if self.parameters.atmosphere_density_ppm != 0 {
            let density = scale_ppm(
                sample.density().raw(),
                self.parameters.atmosphere_density_ppm,
            )
            .ok_or(WorldError::NumericFault)?;
            sample = sample.with_density(Density::from_raw(density));
        }
        let area = scale_ppm(stage.reference_area().raw(), self.parameters.drag_ppm)
            .map(ReferenceArea::from_raw)
            .ok_or(WorldError::NumericFault)?;
        let aero = evaluate_aerodynamics(
            self.world,
            self.truth,
            sample,
            AeroConfig::new(area, table),
            &mut status,
        );
        let thrust = if self.truth.stage_phase() == StagePhase::Burning {
            let stage_index = self.truth.active_stage() as usize;
            let ppm = if stage_index < self.parameters.stage_thrust_ppm.len() {
                self.parameters.stage_thrust_ppm[stage_index]
            } else {
                0
            };
            Force::from_raw(scale_ppm(stage.thrust().raw(), ppm).ok_or(WorldError::NumericFault)?)
        } else {
            Force::ZERO
        };
        let forces =
            evaluate_planar_forces_phase3(self.world, self.truth, thrust, pitch, aero, &mut status)
                .ok_or(WorldError::NumericFault)?;
        let successor = advance_planar_state(self.world, self.truth, forces, &mut status)
            .map_err(|_| WorldError::NumericFault)?;
        if !status.is_clear() {
            return Err(WorldError::NumericFault);
        }
        Ok((successor, aero.mach(), aero.dynamic_pressure()))
    }

    fn consume_burning_propellant(
        &self,
        mass: i32,
        propellant: i32,
        status: &mut NumericStatus,
    ) -> (i32, i32) {
        let stage = self
            .scenario
            .stage(self.truth.active_stage())
            .expect("validated stage");
        let planned = multiply_scaled(
            stage.mass_flow().raw(),
            self.scenario.timestep().raw(),
            20,
            status,
        );
        let consumed = planned.min(propellant);
        (
            subtract(mass, consumed, status),
            subtract(propellant, consumed, status),
        )
    }

    pub fn step_compatibility(&mut self, pitch: PitchAngle) -> Result<WorldSnapshot, WorldError> {
        if self.truth.step() >= self.scenario.steps() {
            return Err(WorldError::Complete);
        }
        let stage = self
            .scenario
            .stage(self.truth.active_stage())
            .ok_or(WorldError::Configuration)?;
        let (successor, mach, dynamic_pressure) = self.integrate(pitch)?;
        let mut status = NumericStatus::CLEAR;
        let mut mass = self.truth.total_mass().raw();
        let mut propellant = self.truth.active_propellant().raw();
        let mut active_stage = self.truth.active_stage();
        let mut phase = self.truth.stage_phase();
        let mut events = 0u16;
        match phase {
            StagePhase::Burning => {
                (mass, propellant) = self.consume_burning_propellant(mass, propellant, &mut status);
                self.burn_steps += 1;
                if self.burn_steps >= stage.burn_steps() || propellant == 0 {
                    phase = if stage.separate() {
                        StagePhase::CoastBeforeSeparation
                    } else {
                        StagePhase::Complete
                    };
                    self.phase_steps = 0;
                    events |= EVENT_CUTOFF;
                }
            }
            StagePhase::CoastBeforeSeparation => {
                self.phase_steps += 1;
                if self.phase_steps >= stage.separation_delay_steps() as u32 {
                    mass = subtract(
                        mass,
                        add(stage.dry_mass().raw(), propellant, &mut status),
                        &mut status,
                    );
                    active_stage += 1;
                    let next = self
                        .scenario
                        .stage(active_stage)
                        .ok_or(WorldError::Configuration)?;
                    propellant = next.propellant_mass().raw();
                    self.phase_steps = 0;
                    self.burn_steps = 0;
                    events |= EVENT_SEPARATION;
                    phase = if next.ignition_delay_steps() == 0 {
                        events |= EVENT_IGNITION;
                        StagePhase::Burning
                    } else {
                        StagePhase::CoastBeforeIgnition
                    };
                }
            }
            StagePhase::CoastBeforeIgnition => {
                self.phase_steps += 1;
                if self.phase_steps >= stage.ignition_delay_steps() as u32 {
                    phase = StagePhase::Burning;
                    self.phase_steps = 0;
                    self.burn_steps = 0;
                    events |= EVENT_IGNITION;
                }
            }
            StagePhase::Complete => {}
        }
        if !status.is_clear() {
            return Err(WorldError::NumericFault);
        }
        self.truth = successor.with_vehicle_state(
            Mass::from_raw(mass),
            Mass::from_raw(propellant),
            active_stage,
            phase,
        );
        self.truth_checksum = hash_planar_truth(self.truth_checksum, &self.truth);
        Ok(WorldSnapshot {
            truth: self.truth,
            pitch,
            mach,
            dynamic_pressure,
            events,
            truth_checksum: self.truth_checksum,
        })
    }

    fn apply_command(&mut self, command: WorldCommand, events: &mut u16) -> Result<(), WorldError> {
        let stage = self
            .scenario
            .stage(self.truth.active_stage())
            .ok_or(WorldError::Configuration)?;
        let safe_cutoff = command.abort_safeing || command.engine_action == EngineAction::Cutoff;
        if safe_cutoff && self.truth.stage_phase() == StagePhase::Burning {
            let phase = if stage.separate() {
                StagePhase::CoastBeforeSeparation
            } else {
                StagePhase::Complete
            };
            self.truth = self.truth.with_vehicle_state(
                self.truth.total_mass(),
                self.truth.active_propellant(),
                self.truth.active_stage(),
                phase,
            );
            self.phase_steps = 0;
            *events |= EVENT_CUTOFF;
        }
        if command.engine_action == EngineAction::Ignite
            && self.truth.stage_phase() == StagePhase::CoastBeforeIgnition
            && self.phase_steps >= stage.ignition_delay_steps() as u32
        {
            self.truth = self.truth.with_vehicle_state(
                self.truth.total_mass(),
                self.truth.active_propellant(),
                self.truth.active_stage(),
                StagePhase::Burning,
            );
            self.phase_steps = 0;
            self.burn_steps = 0;
            *events |= EVENT_IGNITION;
        }
        if command.separate
            && self.truth.stage_phase() == StagePhase::CoastBeforeSeparation
            && self.phase_steps >= stage.separation_delay_steps() as u32
        {
            let mut status = NumericStatus::CLEAR;
            let mass = subtract(
                self.truth.total_mass().raw(),
                add(
                    stage.dry_mass().raw(),
                    self.truth.active_propellant().raw(),
                    &mut status,
                ),
                &mut status,
            );
            let active_stage = self.truth.active_stage() + 1;
            let next = self
                .scenario
                .stage(active_stage)
                .ok_or(WorldError::Configuration)?;
            if !status.is_clear() {
                return Err(WorldError::NumericFault);
            }
            self.truth = self.truth.with_vehicle_state(
                Mass::from_raw(mass),
                next.propellant_mass(),
                active_stage,
                StagePhase::CoastBeforeIgnition,
            );
            self.phase_steps = 0;
            self.burn_steps = 0;
            *events |= EVENT_SEPARATION;
        }
        Ok(())
    }

    pub fn step_commanded(&mut self, command: WorldCommand) -> Result<WorldSnapshot, WorldError> {
        if self.truth.step() >= self.scenario.steps() {
            return Err(WorldError::Complete);
        }
        let mut events = 0u16;
        self.apply_command(command, &mut events)?;
        let pitch = PitchAngle::from_raw(command.pitch);
        if !pitch.is_phase3_valid() {
            return Err(WorldError::Configuration);
        }
        let stage = self
            .scenario
            .stage(self.truth.active_stage())
            .ok_or(WorldError::Configuration)?;
        let was_burning = self.truth.stage_phase() == StagePhase::Burning;
        let (successor, mach, dynamic_pressure) = self.integrate(pitch)?;
        let mut status = NumericStatus::CLEAR;
        let mut mass = self.truth.total_mass().raw();
        let mut propellant = self.truth.active_propellant().raw();
        let mut phase = self.truth.stage_phase();
        if was_burning {
            (mass, propellant) = self.consume_burning_propellant(mass, propellant, &mut status);
            self.burn_steps += 1;
            if self.burn_steps >= stage.burn_steps() || propellant == 0 {
                phase = if stage.separate() {
                    StagePhase::CoastBeforeSeparation
                } else {
                    StagePhase::Complete
                };
                self.phase_steps = 0;
                events |= EVENT_CUTOFF;
            }
        } else if matches!(
            phase,
            StagePhase::CoastBeforeIgnition | StagePhase::CoastBeforeSeparation
        ) {
            self.phase_steps += 1;
        }
        if !status.is_clear() {
            return Err(WorldError::NumericFault);
        }
        self.truth = successor.with_vehicle_state(
            Mass::from_raw(mass),
            Mass::from_raw(propellant),
            self.truth.active_stage(),
            phase,
        );
        self.truth_checksum = hash_planar_truth(self.truth_checksum, &self.truth);
        Ok(WorldSnapshot {
            truth: self.truth,
            pitch,
            mach,
            dynamic_pressure,
            events,
            truth_checksum: self.truth_checksum,
        })
    }
}
