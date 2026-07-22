//! Canonical allocation-free Phase 1 telemetry serialization.

use crate::mission::{
    execute_vertical_mission, ExecutionError, MissionFailure, MissionObservation, MissionObserver,
    MissionSummary,
};
use crate::quantities::{Acceleration, Altitude, Mass, Time, Velocity};
use crate::scenario::{crc32_ieee, Scenario, NUMERIC_CONTRACT_ID};
use crate::vehicle::VerticalTruthState;

pub const TELEMETRY_VERSION: u16 = 1;
pub const TELEMETRY_HEADER_LENGTH: usize = 32;
pub const TELEMETRY_FRAME_LENGTH: usize = 40;

const HEADER_MAGIC: [u8; 4] = *b"KST1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TelemetryWriteError {
    Length,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct TelemetryStatus(u16);

impl TelemetryStatus {
    pub const ENGINE_ACTIVE: u16 = 0x0001;
    pub const CLEAR: Self = Self(0);

    pub const fn from_engine_active(engine_active: bool) -> Self {
        Self(if engine_active {
            Self::ENGINE_ACTIVE
        } else {
            0
        })
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct TelemetryEvents(u16);

impl TelemetryEvents {
    pub const ENGINE_CUTOFF: u16 = 0x0001;
    pub const PROPELLANT_DEPLETED: u16 = 0x0002;
    pub const NUMERIC_FAULT: u16 = 0x0004;
    pub const END_OF_RUN: u16 = 0x0008;
    pub const NONE: Self = Self(0);

    pub const fn new(
        engine_cutoff: bool,
        propellant_depleted: bool,
        numeric_fault: bool,
        end_of_run: bool,
    ) -> Self {
        let mut bits = 0u16;
        if engine_cutoff {
            bits |= Self::ENGINE_CUTOFF;
        }
        if propellant_depleted {
            bits |= Self::PROPELLANT_DEPLETED;
        }
        if numeric_fault {
            bits |= Self::NUMERIC_FAULT;
        }
        if end_of_run {
            bits |= Self::END_OF_RUN;
        }
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TelemetryFrame {
    step: u32,
    time: Time,
    altitude: Altitude,
    velocity: Velocity,
    acceleration: Acceleration,
    total_mass: Mass,
    propellant: Mass,
    status: TelemetryStatus,
    events: TelemetryEvents,
    state_checksum: u32,
}

impl TelemetryFrame {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        step: u32,
        time: Time,
        altitude: Altitude,
        velocity: Velocity,
        acceleration: Acceleration,
        total_mass: Mass,
        propellant: Mass,
        status: TelemetryStatus,
        events: TelemetryEvents,
        state_checksum: u32,
    ) -> Self {
        Self {
            step,
            time,
            altitude,
            velocity,
            acceleration,
            total_mass,
            propellant,
            status,
            events,
            state_checksum,
        }
    }

    pub const fn from_truth(
        truth: VerticalTruthState,
        status: TelemetryStatus,
        events: TelemetryEvents,
        state_checksum: u32,
    ) -> Self {
        Self::new(
            truth.step(),
            truth.time(),
            truth.altitude(),
            truth.velocity(),
            truth.acceleration(),
            truth.total_mass(),
            truth.propellant(),
            status,
            events,
            state_checksum,
        )
    }
}

#[inline]
fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[inline]
fn write_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[inline]
fn write_i32(output: &mut [u8], offset: usize, value: i32) {
    write_u32(output, offset, value as u32);
}

pub fn write_telemetry_header(
    scenario: &Scenario,
    output: &mut [u8],
) -> Result<(), TelemetryWriteError> {
    if output.len() != TELEMETRY_HEADER_LENGTH {
        return Err(TelemetryWriteError::Length);
    }

    output[0..4].copy_from_slice(&HEADER_MAGIC);
    write_u16(output, 4, TELEMETRY_VERSION);
    write_u16(output, 6, TELEMETRY_HEADER_LENGTH as u16);
    write_u16(output, 8, TELEMETRY_FRAME_LENGTH as u16);
    write_u16(output, 10, 0);
    write_u32(output, 12, NUMERIC_CONTRACT_ID);
    write_u32(output, 16, scenario.scenario_id());
    write_i32(output, 20, scenario.timestep().raw());
    write_u16(output, 24, scenario.telemetry_stride());
    write_u16(output, 26, 0);
    let checksum = crc32_ieee(&output[..28]);
    write_u32(output, 28, checksum);
    Ok(())
}

pub fn write_telemetry_frame(
    frame: &TelemetryFrame,
    output: &mut [u8],
) -> Result<(), TelemetryWriteError> {
    if output.len() != TELEMETRY_FRAME_LENGTH {
        return Err(TelemetryWriteError::Length);
    }

    write_u32(output, 0, frame.step);
    write_i32(output, 4, frame.time.raw());
    write_i32(output, 8, frame.altitude.raw());
    write_i32(output, 12, frame.velocity.raw());
    write_i32(output, 16, frame.acceleration.raw());
    write_i32(output, 20, frame.total_mass.raw());
    write_i32(output, 24, frame.propellant.raw());
    write_u16(output, 28, frame.status.bits());
    write_u16(output, 30, frame.events.bits());
    write_u32(output, 32, frame.state_checksum);
    let checksum = crc32_ieee(&output[..36]);
    write_u32(output, 36, checksum);
    Ok(())
}

/// Receives canonical records without requiring allocation or storage policy in the core.
pub trait TelemetrySink {
    type Error;

    fn write_header(&mut self, header: &[u8; TELEMETRY_HEADER_LENGTH]) -> Result<(), Self::Error>;

    fn write_frame(&mut self, frame: &[u8; TELEMETRY_FRAME_LENGTH]) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TelemetryMissionSummary {
    mission: MissionSummary,
    frames_written: u32,
}

impl TelemetryMissionSummary {
    pub const fn mission(self) -> MissionSummary {
        self.mission
    }

    pub const fn frames_written(self) -> u32 {
        self.frames_written
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TelemetryMissionFailure<E> {
    Header(E),
    Frame {
        error: E,
        last_truth: VerticalTruthState,
        checksum: u32,
        cutoff_events: u16,
        frames_written: u32,
    },
    Simulation {
        failure: MissionFailure,
        fault_frame_written: bool,
        frames_written: u32,
    },
    SimulationAndFrame {
        failure: MissionFailure,
        error: E,
        frames_written: u32,
    },
}

struct MissionTelemetryEmitter<'a, S: TelemetrySink> {
    sink: &'a mut S,
    stride: u32,
    pending_events: TelemetryEvents,
    frames_written: u32,
}

impl<S: TelemetrySink> MissionTelemetryEmitter<'_, S> {
    fn emit(
        &mut self,
        truth: VerticalTruthState,
        engine_active: bool,
        events: TelemetryEvents,
        checksum: u32,
    ) -> Result<(), S::Error> {
        let frame = TelemetryFrame::from_truth(
            truth,
            TelemetryStatus::from_engine_active(engine_active),
            events,
            checksum,
        );
        let mut output = [0u8; TELEMETRY_FRAME_LENGTH];
        let write_result = write_telemetry_frame(&frame, &mut output);
        debug_assert_eq!(write_result, Ok(()));
        self.sink.write_frame(&output)?;
        self.frames_written += 1;
        Ok(())
    }

    fn emit_fault(&mut self, scenario: &Scenario, failure: MissionFailure) -> Result<(), S::Error> {
        let truth = failure.last_truth();
        let events = self
            .pending_events
            .union(TelemetryEvents::new(false, false, true, true));
        let engine_active =
            truth.propellant().raw() > 0 && truth.time() < scenario.vehicle().burn_duration();
        self.emit(truth, engine_active, events, failure.checksum())
    }
}

impl<S: TelemetrySink> MissionObserver for MissionTelemetryEmitter<'_, S> {
    type Error = S::Error;

    fn observe(&mut self, observation: MissionObservation) -> Result<(), Self::Error> {
        self.pending_events = self.pending_events.union(TelemetryEvents::new(
            observation.engine_cutoff,
            observation.propellant_depleted,
            false,
            observation.end_of_run,
        ));
        let step = observation.truth.step();
        let stride_boundary = (step / self.stride) * self.stride == step;
        let should_emit = step == 0 || stride_boundary || observation.end_of_run;
        if should_emit {
            self.emit(
                observation.truth,
                observation.engine_active,
                self.pending_events,
                observation.checksum,
            )?;
            self.pending_events = TelemetryEvents::NONE;
        }
        Ok(())
    }
}

/// Executes one checked mission and emits its canonical telemetry stream.
///
/// The initial truth is always emitted. Successors are emitted at the scenario stride,
/// with a final off-stride frame when needed. Events accumulate until a frame accepts them.
pub fn run_vertical_mission_with_telemetry<S: TelemetrySink>(
    scenario: &Scenario,
    sink: &mut S,
) -> Result<TelemetryMissionSummary, TelemetryMissionFailure<S::Error>> {
    let mut header = [0u8; TELEMETRY_HEADER_LENGTH];
    let write_result = write_telemetry_header(scenario, &mut header);
    debug_assert_eq!(write_result, Ok(()));
    sink.write_header(&header)
        .map_err(TelemetryMissionFailure::Header)?;

    let mut emitter = MissionTelemetryEmitter {
        sink,
        stride: scenario.telemetry_stride() as u32,
        pending_events: TelemetryEvents::NONE,
        frames_written: 0,
    };

    match execute_vertical_mission::<true, _>(scenario, &mut emitter) {
        Ok(summary) => Ok(TelemetryMissionSummary {
            mission: MissionSummary {
                final_truth: summary.final_truth,
                checksum: summary.checksum,
                cutoff_events: summary.cutoff_events,
            },
            frames_written: emitter.frames_written,
        }),
        Err(ExecutionError::Observer {
            error,
            last_truth,
            checksum,
            cutoff_events,
        }) => Err(TelemetryMissionFailure::Frame {
            error,
            last_truth,
            checksum,
            cutoff_events,
            frames_written: emitter.frames_written,
        }),
        Err(ExecutionError::Dynamics(failure)) => {
            let failure = MissionFailure {
                last_truth: failure.last_truth,
                checksum: failure.checksum,
                cutoff_events: failure.cutoff_events,
                numeric_status: failure.numeric_status,
                cause: failure.cause,
            };
            match emitter.emit_fault(scenario, failure) {
                Ok(()) => Err(TelemetryMissionFailure::Simulation {
                    failure,
                    fault_frame_written: true,
                    frames_written: emitter.frames_written,
                }),
                Err(error) => Err(TelemetryMissionFailure::SimulationAndFrame {
                    failure,
                    error,
                    frames_written: emitter.frames_written,
                }),
            }
        }
    }
}
