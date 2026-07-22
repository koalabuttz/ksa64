//! Deterministic Phase 1 mission execution and canonical exact-state checksums.

use crate::dynamics::{advance_vertical_state, VerticalStepError};
use crate::environment::SimpleEarthEnvironment;
use crate::numeric::NumericStatus;
use crate::scenario::Scenario;
use crate::vehicle::VerticalTruthState;

pub const VERTICAL_CHECKSUM_OFFSET: u32 = 2_166_136_261;
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

/// Hashes canonical raw truth fields, each as four little-endian bytes.
pub fn hash_vertical_truth(mut checksum: u32, truth: &VerticalTruthState) -> u32 {
    checksum = hash_word(checksum, truth.step());
    checksum = hash_word(checksum, truth.time().raw() as u32);
    checksum = hash_word(checksum, truth.altitude().raw() as u32);
    checksum = hash_word(checksum, truth.velocity().raw() as u32);
    checksum = hash_word(checksum, truth.acceleration().raw() as u32);
    checksum = hash_word(checksum, truth.total_mass().raw() as u32);
    hash_word(checksum, truth.propellant().raw() as u32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DynamicsSummary {
    final_truth: VerticalTruthState,
    cutoff_events: u16,
}

impl DynamicsSummary {
    pub const fn final_truth(self) -> VerticalTruthState {
        self.final_truth
    }

    pub const fn completed_steps(self) -> u32 {
        self.final_truth.step()
    }

    pub const fn cutoff_events(self) -> u16 {
        self.cutoff_events
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DynamicsFailure {
    last_truth: VerticalTruthState,
    cutoff_events: u16,
    numeric_status: NumericStatus,
    cause: VerticalStepError,
}

impl DynamicsFailure {
    pub const fn last_truth(self) -> VerticalTruthState {
        self.last_truth
    }

    pub const fn cutoff_events(self) -> u16 {
        self.cutoff_events
    }

    pub const fn numeric_status(self) -> NumericStatus {
        self.numeric_status
    }

    pub const fn cause(self) -> VerticalStepError {
        self.cause
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionSummary {
    pub(crate) final_truth: VerticalTruthState,
    pub(crate) checksum: u32,
    pub(crate) cutoff_events: u16,
}

impl MissionSummary {
    pub const fn final_truth(self) -> VerticalTruthState {
        self.final_truth
    }

    pub const fn completed_steps(self) -> u32 {
        self.final_truth.step()
    }

    pub const fn checksum(self) -> u32 {
        self.checksum
    }

    pub const fn cutoff_events(self) -> u16 {
        self.cutoff_events
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionFailure {
    pub(crate) last_truth: VerticalTruthState,
    pub(crate) checksum: u32,
    pub(crate) cutoff_events: u16,
    pub(crate) numeric_status: NumericStatus,
    pub(crate) cause: VerticalStepError,
}

impl MissionFailure {
    pub const fn last_truth(self) -> VerticalTruthState {
        self.last_truth
    }

    pub const fn checksum(self) -> u32 {
        self.checksum
    }

    pub const fn cutoff_events(self) -> u16 {
        self.cutoff_events
    }

    pub const fn numeric_status(self) -> NumericStatus {
        self.numeric_status
    }

    pub const fn cause(self) -> VerticalStepError {
        self.cause
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ExecutionSummary {
    pub(crate) final_truth: VerticalTruthState,
    pub(crate) checksum: u32,
    pub(crate) cutoff_events: u16,
}

#[derive(Clone, Copy)]
pub(crate) struct ExecutionFailure {
    pub(crate) last_truth: VerticalTruthState,
    pub(crate) checksum: u32,
    pub(crate) cutoff_events: u16,
    pub(crate) numeric_status: NumericStatus,
    pub(crate) cause: VerticalStepError,
}

#[derive(Clone, Copy)]
pub(crate) struct MissionObservation {
    pub(crate) truth: VerticalTruthState,
    pub(crate) checksum: u32,
    pub(crate) engine_active: bool,
    pub(crate) engine_cutoff: bool,
    pub(crate) propellant_depleted: bool,
    pub(crate) end_of_run: bool,
}

pub(crate) trait MissionObserver {
    type Error;

    fn observe(&mut self, observation: MissionObservation) -> Result<(), Self::Error>;
}

pub(crate) enum ExecutionError<E> {
    Dynamics(ExecutionFailure),
    Observer {
        error: E,
        last_truth: VerticalTruthState,
        checksum: u32,
        cutoff_events: u16,
    },
}

struct NoopObserver;

impl MissionObserver for NoopObserver {
    type Error = core::convert::Infallible;

    #[inline]
    fn observe(&mut self, _observation: MissionObservation) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[inline]
fn engine_active(scenario: &Scenario, truth: VerticalTruthState) -> bool {
    truth.propellant().raw() > 0 && truth.time() < scenario.vehicle().burn_duration()
}

pub(crate) fn execute_vertical_mission<const CHECKSUM: bool, O: MissionObserver>(
    scenario: &Scenario,
    observer: &mut O,
) -> Result<ExecutionSummary, ExecutionError<O::Error>> {
    let environment = SimpleEarthEnvironment::from_scenario(scenario);
    let mut truth = VerticalTruthState::initial(scenario);
    let mut checksum = VERTICAL_CHECKSUM_OFFSET;
    let mut cutoff_events = 0u16;
    let mut status = NumericStatus::CLEAR;

    observer
        .observe(MissionObservation {
            truth,
            checksum,
            engine_active: engine_active(scenario, truth),
            engine_cutoff: false,
            propellant_depleted: false,
            end_of_run: false,
        })
        .map_err(|error| ExecutionError::Observer {
            error,
            last_truth: truth,
            checksum,
            cutoff_events,
        })?;

    while truth.step() < scenario.steps() {
        let previous_propellant = truth.propellant().raw();
        match advance_vertical_state(scenario, environment, &truth, &mut status) {
            Ok(step) => {
                truth = step.truth();
                if CHECKSUM {
                    checksum = hash_vertical_truth(checksum, &truth);
                }
                if step.engine_cutoff() {
                    cutoff_events += 1;
                }
                observer
                    .observe(MissionObservation {
                        truth,
                        checksum,
                        engine_active: engine_active(scenario, truth),
                        engine_cutoff: step.engine_cutoff(),
                        propellant_depleted: previous_propellant > 0
                            && truth.propellant().raw() == 0,
                        end_of_run: truth.step() == scenario.steps(),
                    })
                    .map_err(|error| ExecutionError::Observer {
                        error,
                        last_truth: truth,
                        checksum,
                        cutoff_events,
                    })?;
            }
            Err(cause) => {
                return Err(ExecutionError::Dynamics(ExecutionFailure {
                    last_truth: truth,
                    checksum,
                    cutoff_events,
                    numeric_status: status,
                    cause,
                }));
            }
        }
    }

    Ok(ExecutionSummary {
        final_truth: truth,
        checksum,
        cutoff_events,
    })
}

/// Executes the checked production dynamics without rolling validation work.
///
/// This is the baseline for timing simulation cost independently from checksumming.
pub fn run_vertical_dynamics(scenario: &Scenario) -> Result<DynamicsSummary, DynamicsFailure> {
    execute_vertical_mission::<false, _>(scenario, &mut NoopObserver)
        .map(|summary| DynamicsSummary {
            final_truth: summary.final_truth,
            cutoff_events: summary.cutoff_events,
        })
        .map_err(|failure| match failure {
            ExecutionError::Dynamics(failure) => DynamicsFailure {
                last_truth: failure.last_truth,
                cutoff_events: failure.cutoff_events,
                numeric_status: failure.numeric_status,
                cause: failure.cause,
            },
            ExecutionError::Observer { error, .. } => match error {},
        })
}

/// Executes the checked production dynamics with a rolling exact-state checksum.
pub fn run_vertical_mission(scenario: &Scenario) -> Result<MissionSummary, MissionFailure> {
    execute_vertical_mission::<true, _>(scenario, &mut NoopObserver)
        .map(|summary| MissionSummary {
            final_truth: summary.final_truth,
            checksum: summary.checksum,
            cutoff_events: summary.cutoff_events,
        })
        .map_err(|failure| match failure {
            ExecutionError::Dynamics(failure) => MissionFailure {
                last_truth: failure.last_truth,
                checksum: failure.checksum,
                cutoff_events: failure.cutoff_events,
                numeric_status: failure.numeric_status,
                cause: failure.cause,
            },
            ExecutionError::Observer { error, .. } => match error {},
        })
}
