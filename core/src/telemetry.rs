//! Canonical allocation-free Phase 1 telemetry serialization.

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
