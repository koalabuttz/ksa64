//! Deterministic bounded-stage execution for the Phase 2 planar mission.

use crate::aerodynamics::{evaluate_aerodynamics, AeroConfig};
use crate::numeric::{add, multiply_scaled, subtract, NumericStatus};
use crate::phase2_numeric::sqrt_floor_u32;
use crate::phase2_quantities::{DynamicPressure, PlanarAcceleration};
use crate::phase2_scenario::Phase2Scenario;
use crate::planar::{classify_orbit, OrbitSolution, PlanarTruthState, PlanarWorld, StagePhase};
use crate::planar_dynamics::{advance_planar_state, evaluate_planar_forces};
use crate::planar_environment::RotatingEarthEnvironment;
use crate::quantities::{Force, Mass};

pub const EVENT_IGNITION: u16 = 1 << 0;
pub const EVENT_CUTOFF: u16 = 1 << 1;
pub const EVENT_SEPARATION: u16 = 1 << 2;
pub const EVENT_MAX_Q: u16 = 1 << 3;
pub const EVENT_IMPACT: u16 = 1 << 4;
pub const EVENT_END: u16 = 1 << 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2MissionError {
    InitialState,
    Configuration,
    NumericFault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2MissionOutcome {
    DurationComplete,
    Impact,
}

#[derive(Clone, Copy, Debug)]
pub struct Phase2MissionResult {
    truth: PlanarTruthState,
    outcome: Phase2MissionOutcome,
    event_history: u16,
    max_dynamic_pressure: DynamicPressure,
    max_proper_acceleration: PlanarAcceleration,
    cutoff_step: u32,
    cutoff_orbit: Option<OrbitSolution>,
    terminal_orbit: Option<OrbitSolution>,
    state_checksum: u32,
}

impl Phase2MissionResult {
    pub const fn truth(self) -> PlanarTruthState {
        self.truth
    }
    pub const fn outcome(self) -> Phase2MissionOutcome {
        self.outcome
    }
    pub const fn event_history(self) -> u16 {
        self.event_history
    }
    pub const fn max_dynamic_pressure(self) -> DynamicPressure {
        self.max_dynamic_pressure
    }
    pub const fn max_proper_acceleration(self) -> PlanarAcceleration {
        self.max_proper_acceleration
    }
    pub const fn cutoff_step(self) -> u32 {
        self.cutoff_step
    }
    pub const fn cutoff_orbit(self) -> Option<OrbitSolution> {
        self.cutoff_orbit
    }
    pub const fn terminal_orbit(self) -> Option<OrbitSolution> {
        self.terminal_orbit
    }
    pub const fn state_checksum(self) -> u32 {
        self.state_checksum
    }
}

pub const PLANAR_CHECKSUM_OFFSET: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;

#[inline]
fn hash_word(mut checksum: u32, word: u32) -> u32 {
    let mut shift = 0u8;
    while shift < 32 {
        checksum ^= (word >> shift) & 0xff;
        checksum = checksum.wrapping_mul(FNV_PRIME);
        shift += 8;
    }
    checksum
}

pub fn hash_planar_truth(mut checksum: u32, truth: &PlanarTruthState) -> u32 {
    checksum = hash_word(checksum, truth.step());
    checksum = hash_word(checksum, truth.time().raw() as u32);
    checksum = hash_word(checksum, truth.radius().raw() as u32);
    checksum = hash_word(checksum, truth.downrange().raw() as u32);
    checksum = hash_word(checksum, truth.radial_velocity().raw() as u32);
    checksum = hash_word(checksum, truth.specific_angular_momentum().raw() as u32);
    checksum = hash_word(checksum, truth.radial_acceleration().raw() as u32);
    checksum = hash_word(checksum, truth.tangential_acceleration().raw() as u32);
    checksum = hash_word(checksum, truth.total_mass().raw() as u32);
    checksum = hash_word(checksum, truth.active_propellant().raw() as u32);
    checksum = hash_word(checksum, truth.active_stage() as u32);
    hash_word(checksum, truth.stage_phase() as u32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase2Observation {
    truth: PlanarTruthState,
    pitch: crate::phase2_quantities::PitchAngle,
    mach: crate::phase2_quantities::Mach,
    dynamic_pressure: DynamicPressure,
    events: u16,
    state_checksum: u32,
}

impl Phase2Observation {
    pub const fn truth(self) -> PlanarTruthState {
        self.truth
    }
    pub const fn pitch(self) -> crate::phase2_quantities::PitchAngle {
        self.pitch
    }
    pub const fn mach(self) -> crate::phase2_quantities::Mach {
        self.mach
    }
    pub const fn dynamic_pressure(self) -> DynamicPressure {
        self.dynamic_pressure
    }
    pub const fn events(self) -> u16 {
        self.events
    }
    pub const fn state_checksum(self) -> u32 {
        self.state_checksum
    }
}

pub trait Phase2Observer {
    type Error;
    fn observe(&mut self, observation: Phase2Observation) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2ExecutionError<E> {
    Mission(Phase2MissionError),
    Observer(E),
}
fn proper_acceleration_q28(
    radial_force_q12: i32,
    tangential_force_q12: i32,
    mass_q12: i32,
    status: &mut NumericStatus,
) -> i32 {
    let radial = crate::numeric::divide_scaled(radial_force_q12, mass_q12, 28, status);
    let tangential = crate::numeric::divide_scaled(tangential_force_q12, mass_q12, 28, status);
    let radial2_q28 = multiply_scaled(radial, radial, 28, status);
    let tangential2_q28 = multiply_scaled(tangential, tangential, 28, status);
    let magnitude2_q28 = add(radial2_q28, tangential2_q28, status);
    sqrt_floor_u32((magnitude2_q28.max(0) as u32) << 4) as i32 * (1 << 12)
}

#[allow(clippy::too_many_arguments)]
fn replace_vehicle_state(
    truth: PlanarTruthState,
    total_mass: Mass,
    active_propellant: Mass,
    active_stage: u8,
    stage_phase: StagePhase,
) -> PlanarTruthState {
    truth.successor(
        truth.step(),
        truth.time(),
        truth.radius(),
        truth.downrange(),
        truth.radial_velocity(),
        truth.specific_angular_momentum(),
        truth.radial_acceleration(),
        truth.tangential_acceleration(),
        total_mass,
        active_propellant,
        active_stage,
        stage_phase,
    )
}

fn execute_phase2_mission_internal<const CHECKSUM: bool, O: Phase2Observer>(
    scenario: &Phase2Scenario,
    observer: &mut O,
) -> Result<Phase2MissionResult, Phase2ExecutionError<O::Error>> {
    let world = PlanarWorld::simple_earth(scenario.timestep());
    let environment = RotatingEarthEnvironment::new();
    let mut status = NumericStatus::CLEAR;
    let mut truth = scenario
        .initial_truth(&mut status)
        .ok_or(Phase2ExecutionError::Mission(
            Phase2MissionError::InitialState,
        ))?;
    if !scenario.pitch_program().is_valid(scenario.timestep()) {
        return Err(Phase2ExecutionError::Mission(
            Phase2MissionError::Configuration,
        ));
    }
    let mut phase_steps = 0u32;
    let mut burn_steps = 0u32;
    let mut max_q = 0i32;
    let mut max_proper = 0i32;
    let mut events = if truth.stage_phase() == StagePhase::Burning {
        EVENT_IGNITION
    } else {
        0
    };
    let mut cutoff_step = 0u32;
    let mut cutoff_orbit = None;
    let mut outcome = Phase2MissionOutcome::DurationComplete;
    let mut checksum = PLANAR_CHECKSUM_OFFSET;
    let initial_pitch = scenario.pitch_program().pitch_at(truth.time(), &mut status);
    observer
        .observe(Phase2Observation {
            truth,
            pitch: initial_pitch,
            mach: crate::phase2_quantities::Mach::ZERO,
            dynamic_pressure: DynamicPressure::ZERO,
            events: 0,
            state_checksum: checksum,
        })
        .map_err(Phase2ExecutionError::Observer)?;

    while truth.step() < scenario.steps() {
        let mut step_events = 0u16;
        let stage = scenario
            .stage(truth.active_stage())
            .ok_or(Phase2ExecutionError::Mission(
                Phase2MissionError::Configuration,
            ))?;
        let table =
            scenario
                .aero_table(stage.aero_table_index())
                .ok_or(Phase2ExecutionError::Mission(
                    Phase2MissionError::Configuration,
                ))?;
        let sample = environment.sample(truth.radius(), &mut status);
        let aero = evaluate_aerodynamics(
            world,
            truth,
            sample,
            AeroConfig::new(stage.reference_area(), table),
            &mut status,
        );
        if aero.dynamic_pressure().raw() > max_q {
            max_q = aero.dynamic_pressure().raw();
            events |= EVENT_MAX_Q;
        }
        let pitch = scenario.pitch_program().pitch_at(truth.time(), &mut status);
        let thrust = if truth.stage_phase() == StagePhase::Burning {
            stage.thrust()
        } else {
            Force::ZERO
        };
        let forces = evaluate_planar_forces(world, truth, thrust, pitch, aero, &mut status).ok_or(
            Phase2ExecutionError::Mission(Phase2MissionError::NumericFault),
        )?;
        let radial_force = add(
            forces.radial_thrust().raw(),
            forces.radial_drag().raw(),
            &mut status,
        );
        let tangential_force = add(
            forces.tangential_thrust().raw(),
            forces.tangential_drag().raw(),
            &mut status,
        );
        let proper = proper_acceleration_q28(
            radial_force,
            tangential_force,
            truth.total_mass().raw(),
            &mut status,
        );
        max_proper = max_proper.max(proper);
        let mut successor = advance_planar_state(world, truth, forces, &mut status)
            .map_err(|_| Phase2ExecutionError::Mission(Phase2MissionError::NumericFault))?;

        let mut mass = truth.total_mass().raw();
        let mut propellant = truth.active_propellant().raw();
        let mut active_stage = truth.active_stage();
        let mut phase = truth.stage_phase();
        match phase {
            StagePhase::Burning => {
                let planned = multiply_scaled(
                    stage.mass_flow().raw(),
                    scenario.timestep().raw(),
                    20,
                    &mut status,
                );
                let consumed = planned.min(propellant);
                mass = subtract(mass, consumed, &mut status);
                propellant = subtract(propellant, consumed, &mut status);
                burn_steps += 1;
                if burn_steps >= stage.burn_steps() || propellant == 0 {
                    phase = if stage.separate() {
                        StagePhase::CoastBeforeSeparation
                    } else {
                        StagePhase::Complete
                    };
                    phase_steps = 0;
                    events |= EVENT_CUTOFF;
                    step_events |= EVENT_CUTOFF;
                    cutoff_step = successor.step();
                }
            }
            StagePhase::CoastBeforeSeparation => {
                phase_steps += 1;
                if phase_steps >= stage.separation_delay_steps() as u32 {
                    mass = subtract(
                        mass,
                        add(stage.dry_mass().raw(), propellant, &mut status),
                        &mut status,
                    );
                    events |= EVENT_SEPARATION;
                    step_events |= EVENT_SEPARATION;
                    active_stage += 1;
                    let next =
                        scenario
                            .stage(active_stage)
                            .ok_or(Phase2ExecutionError::Mission(
                                Phase2MissionError::Configuration,
                            ))?;
                    propellant = next.propellant_mass().raw();
                    phase_steps = 0;
                    burn_steps = 0;
                    phase = if next.ignition_delay_steps() == 0 {
                        events |= EVENT_IGNITION;
                        step_events |= EVENT_IGNITION;
                        StagePhase::Burning
                    } else {
                        StagePhase::CoastBeforeIgnition
                    };
                }
            }
            StagePhase::CoastBeforeIgnition => {
                phase_steps += 1;
                if phase_steps >= stage.ignition_delay_steps() as u32 {
                    phase = StagePhase::Burning;
                    phase_steps = 0;
                    burn_steps = 0;
                    events |= EVENT_IGNITION;
                    step_events |= EVENT_IGNITION;
                }
            }
            StagePhase::Complete => {}
        }
        successor = replace_vehicle_state(
            successor,
            Mass::from_raw(mass),
            Mass::from_raw(propellant),
            active_stage,
            phase,
        );
        if phase == StagePhase::Complete && cutoff_orbit.is_none() {
            cutoff_orbit = classify_orbit(world, successor, &mut status);
        }
        if !status.is_clear() {
            return Err(Phase2ExecutionError::Mission(
                Phase2MissionError::NumericFault,
            ));
        }
        if CHECKSUM {
            checksum = hash_planar_truth(checksum, &successor);
        }
        let impacted = successor.radius().raw() < world.radius().raw()
            || (successor.radius().raw() == world.radius().raw()
                && successor.radial_velocity().raw() <= 0);
        let terminal = impacted || successor.step() >= scenario.steps();
        if impacted {
            outcome = Phase2MissionOutcome::Impact;
            events |= EVENT_IMPACT;
            step_events |= EVENT_IMPACT;
        }
        if terminal {
            events |= EVENT_END;
            step_events |= EVENT_END;
        }
        observer
            .observe(Phase2Observation {
                truth: successor,
                pitch,
                mach: aero.mach(),
                dynamic_pressure: aero.dynamic_pressure(),
                events: step_events,
                state_checksum: checksum,
            })
            .map_err(Phase2ExecutionError::Observer)?;
        truth = successor;
        if terminal {
            break;
        }
    }
    events |= EVENT_END;
    let terminal_orbit = classify_orbit(world, truth, &mut status);
    if !status.is_clear() {
        return Err(Phase2ExecutionError::Mission(
            Phase2MissionError::NumericFault,
        ));
    }
    Ok(Phase2MissionResult {
        truth,
        outcome,
        event_history: events,
        max_dynamic_pressure: DynamicPressure::from_raw(max_q),
        max_proper_acceleration: PlanarAcceleration::from_raw(max_proper),
        cutoff_step,
        cutoff_orbit,
        terminal_orbit,
        state_checksum: if CHECKSUM { checksum } else { 0 },
    })
}
pub fn execute_phase2_mission_observed<O: Phase2Observer>(
    scenario: &Phase2Scenario,
    observer: &mut O,
) -> Result<Phase2MissionResult, Phase2ExecutionError<O::Error>> {
    execute_phase2_mission_internal::<true, _>(scenario, observer)
}
struct NullPhase2Observer;

impl Phase2Observer for NullPhase2Observer {
    type Error = core::convert::Infallible;
    fn observe(&mut self, _observation: Phase2Observation) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub fn execute_phase2_mission(
    scenario: &Phase2Scenario,
) -> Result<Phase2MissionResult, Phase2MissionError> {
    let mut observer = NullPhase2Observer;
    match execute_phase2_mission_internal::<false, _>(scenario, &mut observer) {
        Ok(result) => Ok(result),
        Err(Phase2ExecutionError::Mission(error)) => Err(error),
        Err(Phase2ExecutionError::Observer(never)) => match never {},
    }
}
