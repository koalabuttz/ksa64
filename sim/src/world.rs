//! Reusable one-step planar vehicle world for Phase 3 composition.

use ksa64_core::aerodynamics::{evaluate_aerodynamics, AeroConfig};
use ksa64_core::numeric::{add, multiply_scaled, subtract, NumericStatus};
use ksa64_core::phase2_mission::{
    hash_planar_truth, EVENT_CUTOFF, EVENT_IGNITION, EVENT_SEPARATION, PLANAR_CHECKSUM_OFFSET,
};
use ksa64_core::phase2_quantities::{DynamicPressure, Mach, PitchAngle};
use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_core::planar::{PlanarTruthState, PlanarWorld, StagePhase};
use ksa64_core::planar_dynamics::{advance_planar_state, evaluate_planar_forces};
use ksa64_core::planar_environment::RotatingEarthEnvironment;
use ksa64_core::quantities::{Force, Mass};
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

pub struct WorldMachine<'a> {
    scenario: &'a Phase2Scenario,
    world: PlanarWorld,
    environment: RotatingEarthEnvironment,
    truth: PlanarTruthState,
    phase_steps: u32,
    burn_steps: u32,
    truth_checksum: u32,
}

impl<'a> WorldMachine<'a> {
    pub fn new_compatibility(scenario: &'a Phase2Scenario) -> Result<Self, WorldError> {
        let mut status = NumericStatus::CLEAR;
        let truth = scenario
            .initial_truth(&mut status)
            .ok_or(WorldError::NumericFault)?;
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
        })
    }

    pub fn new_commanded(scenario: &'a Phase2Scenario) -> Result<Self, WorldError> {
        let mut machine = Self::new_compatibility(scenario)?;
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
        let sample = self.environment.sample(self.truth.radius(), &mut status);
        let aero = evaluate_aerodynamics(
            self.world,
            self.truth,
            sample,
            AeroConfig::new(stage.reference_area(), table),
            &mut status,
        );
        let thrust = if self.truth.stage_phase() == StagePhase::Burning {
            stage.thrust()
        } else {
            Force::ZERO
        };
        let forces =
            evaluate_planar_forces(self.world, self.truth, thrust, pitch, aero, &mut status)
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
