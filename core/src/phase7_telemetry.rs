//! Strict KST7 header and canonical 96-byte telemetry frames.

use crate::phase7_format::{
    seal_phase7_record, validate_phase7_record, write_phase7_header, Phase7RecordError,
    Phase7RecordKind, KST7_FRAME_LENGTH, KST7_HEADER_LENGTH,
};
use crate::phase7_mission::{HobbyFlightPhase, HobbyMissionObservation};
use crate::phase7_numeric::{HobbyDynamicPressure, HobbyMach, HobbyTime, HOBBY_ENVIRONMENT_ID};
use crate::phase7_pack::{HobbyMissionPack, MotorPack, VerticalVehiclePack};
use crate::scenario::crc32_ieee;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kst7Error {
    Record(Phase7RecordError),
    Length,
    Identity,
    FrameLength,
    Phase,
    Flags,
    Reserved,
    Checksum,
}

impl From<Phase7RecordError> for Kst7Error {
    fn from(value: Phase7RecordError) -> Self {
        Self::Record(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyTelemetryHeader {
    pub stream_identity: u32,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub mission_identity: u32,
    pub environment_identity: u32,
    pub telemetry_period: HobbyTime,
    pub max_mission_time: HobbyTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyTelemetryFrame {
    pub observation: HobbyMissionObservation,
}

fn r32(input: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn ru32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}
fn w32(output: &mut [u8], offset: usize, value: i32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn wu32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn zeros(input: &[u8], start: usize, end: usize) -> bool {
    input[start..end].iter().all(|value| *value == 0)
}

pub fn hobby_stream_identity(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
) -> u32 {
    let mut hash = 2_166_136_261u32;
    for word in [
        vehicle.identity,
        motor.identity,
        mission.identity,
        HOBBY_ENVIRONMENT_ID,
    ] {
        for byte in word.to_le_bytes() {
            hash = (hash ^ byte as u32).wrapping_mul(16_777_619);
        }
    }
    hash
}

pub fn encode_kst7_header(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
    output: &mut [u8; KST7_HEADER_LENGTH],
) -> Result<(), Kst7Error> {
    let identity = hobby_stream_identity(vehicle, motor, mission);
    write_phase7_header(output, Phase7RecordKind::TelemetryHeader, identity)?;
    wu32(output, 32, vehicle.identity);
    wu32(output, 36, motor.identity);
    wu32(output, 40, mission.identity);
    wu32(output, 44, HOBBY_ENVIRONMENT_ID);
    w32(output, 48, mission.telemetry_period.raw());
    output[52..54].copy_from_slice(&(KST7_FRAME_LENGTH as u16).to_le_bytes());
    w32(output, 56, mission.max_mission_time.raw());
    seal_phase7_record(output)?;
    Ok(())
}

pub fn parse_kst7_header(input: &[u8]) -> Result<HobbyTelemetryHeader, Kst7Error> {
    let common = validate_phase7_record(input, Phase7RecordKind::TelemetryHeader)?;
    if u16::from_le_bytes([input[52], input[53]]) as usize != KST7_FRAME_LENGTH {
        return Err(Kst7Error::FrameLength);
    }
    if !zeros(input, 54, 56) || !zeros(input, 60, KST7_HEADER_LENGTH - 4) {
        return Err(Kst7Error::Reserved);
    }
    if ru32(input, 44) != HOBBY_ENVIRONMENT_ID {
        return Err(Kst7Error::Identity);
    }
    Ok(HobbyTelemetryHeader {
        stream_identity: common.identity,
        vehicle_identity: ru32(input, 32),
        motor_identity: ru32(input, 36),
        mission_identity: ru32(input, 40),
        environment_identity: ru32(input, 44),
        telemetry_period: HobbyTime::from_raw(r32(input, 48)),
        max_mission_time: HobbyTime::from_raw(r32(input, 56)),
    })
}

pub fn encode_kst7_frame(
    frame: HobbyTelemetryFrame,
    output: &mut [u8; KST7_FRAME_LENGTH],
) -> Result<(), Kst7Error> {
    output.fill(0);
    let observation = frame.observation;
    wu32(output, 0, observation.state.step);
    w32(output, 4, observation.state.time.raw());
    w32(output, 8, observation.state.altitude.raw());
    w32(output, 12, observation.state.velocity.raw());
    w32(output, 16, observation.state.acceleration.raw());
    w32(output, 20, observation.state.mass.raw());
    w32(output, 24, observation.state.propellant.raw());
    w32(output, 28, observation.state.impulse_consumed_q16);
    w32(output, 32, observation.thrust_raw_q13);
    w32(output, 36, observation.dynamic_pressure.raw());
    w32(output, 40, observation.mach.map_or(0, |value| value.raw()));
    wu32(output, 44, observation.events);
    wu32(output, 48, observation.checksum);
    w32(output, 56, observation.state.phase_start_time.raw());
    output[52] = observation.state.phase as u8;
    output[53] = u8::from(observation.mach.is_some());
    let checksum = crc32_ieee(&output[..KST7_FRAME_LENGTH - 4]);
    wu32(output, KST7_FRAME_LENGTH - 4, checksum);
    Ok(())
}

fn parse_phase(value: u8) -> Result<HobbyFlightPhase, Kst7Error> {
    match value {
        0 => Ok(HobbyFlightPhase::ConstrainedPowered),
        1 => Ok(HobbyFlightPhase::Powered),
        2 => Ok(HobbyFlightPhase::Coast),
        3 => Ok(HobbyFlightPhase::DrogueInflating),
        4 => Ok(HobbyFlightPhase::DrogueDescent),
        5 => Ok(HobbyFlightPhase::MainInflating),
        6 => Ok(HobbyFlightPhase::MainDescent),
        7 => Ok(HobbyFlightPhase::Complete),
        _ => Err(Kst7Error::Phase),
    }
}

pub fn parse_kst7_frame(input: &[u8]) -> Result<HobbyTelemetryFrame, Kst7Error> {
    if input.len() != KST7_FRAME_LENGTH {
        return Err(Kst7Error::Length);
    }
    if input[53] > 1 {
        return Err(Kst7Error::Flags);
    }
    if !zeros(input, 54, 56) || !zeros(input, 60, KST7_FRAME_LENGTH - 4) {
        return Err(Kst7Error::Reserved);
    }
    if ru32(input, KST7_FRAME_LENGTH - 4) != crc32_ieee(&input[..KST7_FRAME_LENGTH - 4]) {
        return Err(Kst7Error::Checksum);
    }
    let mach = if input[53] == 1 {
        Some(HobbyMach::from_raw(r32(input, 40)))
    } else {
        None
    };
    Ok(HobbyTelemetryFrame {
        observation: HobbyMissionObservation {
            state: crate::phase7_mission::HobbyVerticalState {
                step: ru32(input, 0),
                time: HobbyTime::from_raw(r32(input, 4)),
                altitude: crate::phase7_numeric::HobbyAltitude::from_raw(r32(input, 8)),
                velocity: crate::phase7_numeric::HobbyVelocity::from_raw(r32(input, 12)),
                acceleration: crate::phase7_numeric::HobbyAcceleration::from_raw(r32(input, 16)),
                mass: crate::phase7_numeric::HobbyMass::from_raw(r32(input, 20)),
                propellant: crate::phase7_numeric::HobbyMass::from_raw(r32(input, 24)),
                impulse_consumed_q16: r32(input, 28),
                phase: parse_phase(input[52])?,
                phase_start_time: HobbyTime::from_raw(r32(input, 56)),
            },
            thrust_raw_q13: r32(input, 32),
            dynamic_pressure: HobbyDynamicPressure::from_raw(r32(input, 36)),
            mach,
            events: ru32(input, 44),
            checksum: ru32(input, 48),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_corruption_fails_closed() {
        let state = crate::phase7_mission::HobbyVerticalState {
            step: 1,
            time: HobbyTime::from_raw(2),
            altitude: crate::phase7_numeric::HobbyAltitude::from_raw(3),
            velocity: crate::phase7_numeric::HobbyVelocity::from_raw(4),
            acceleration: crate::phase7_numeric::HobbyAcceleration::from_raw(5),
            mass: crate::phase7_numeric::HobbyMass::from_raw(6),
            propellant: crate::phase7_numeric::HobbyMass::from_raw(7),
            impulse_consumed_q16: 8,
            phase: HobbyFlightPhase::Powered,
            phase_start_time: HobbyTime::ZERO,
        };
        let frame = HobbyTelemetryFrame {
            observation: HobbyMissionObservation {
                state,
                thrust_raw_q13: 9,
                dynamic_pressure: HobbyDynamicPressure::from_raw(10),
                mach: Some(HobbyMach::from_raw(11)),
                events: 12,
                checksum: 13,
            },
        };
        let mut bytes = [0u8; KST7_FRAME_LENGTH];
        encode_kst7_frame(frame, &mut bytes).unwrap();
        assert_eq!(parse_kst7_frame(&bytes).unwrap().observation.state.step, 1);
        bytes[20] ^= 1;
        assert_eq!(parse_kst7_frame(&bytes), Err(Kst7Error::Checksum));
    }
}
