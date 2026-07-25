//! Strict KST8 header and canonical 160-byte spatial telemetry frames.

use crate::phase8_format::{
    seal_phase8_record, validate_phase8_record, write_phase8_header, Phase8RecordError,
    Phase8RecordKind, KST8_FRAME_LENGTH, KST8_HEADER_LENGTH,
};
use crate::phase8_mission::{
    HobbySpatialPhase, Phase8MissionSnapshot, SpatialAeroState, SpatialMassProperties,
};
use crate::phase8_numeric::{
    BodyAngularRate, EnuAcceleration, EnuPosition, EnuVelocity, SpatialInertia, SpatialMass,
    SpatialMomentArm, SpatialTime, HOBBY_SPATIAL_ENVIRONMENT_ID,
};
use crate::phase8_pack::{
    SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindProfilePack,
};
use crate::phase8_world::HobbySpatialState;
use crate::scenario::crc32_ieee;
use crate::spatial_numeric::QuaternionQ30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kst8Error {
    Record(Phase8RecordError),
    Length,
    Identity,
    FrameLength,
    Phase,
    Reserved,
    Checksum,
}
impl From<Phase8RecordError> for Kst8Error {
    fn from(value: Phase8RecordError) -> Self {
        Self::Record(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialTelemetryHeader {
    pub stream_identity: u32,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub mission_identity: u32,
    pub wind_identity: u32,
    pub telemetry_period: SpatialTime,
    pub max_mission_time: SpatialTime,
    pub case_seed: u32,
    pub variation_checksum: u32,
}
fn r32(i: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(i[o..o + 4].try_into().unwrap())
}
fn ru32(i: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(i[o..o + 4].try_into().unwrap())
}
fn w32(o: &mut [u8], p: usize, v: i32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn wu32(o: &mut [u8], p: usize, v: u32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn zeros(i: &[u8], a: usize, b: usize) -> bool {
    i[a..b].iter().all(|v| *v == 0)
}

pub fn spatial_stream_identity(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
    variation_checksum: u32,
) -> u32 {
    let mut hash = 2_166_136_261u32;
    for word in [
        vehicle.identity,
        motor.identity,
        mission.identity,
        wind.identity,
        HOBBY_SPATIAL_ENVIRONMENT_ID,
        mission.case_seed,
        variation_checksum,
    ] {
        for byte in word.to_le_bytes() {
            hash = (hash ^ byte as u32).wrapping_mul(16_777_619);
        }
    }
    hash
}
pub fn encode_kst8_header(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
    variation_checksum: u32,
    output: &mut [u8; KST8_HEADER_LENGTH],
) -> Result<(), Kst8Error> {
    let identity = spatial_stream_identity(vehicle, motor, mission, wind, variation_checksum);
    write_phase8_header(output, Phase8RecordKind::TelemetryHeader, identity)?;
    for (offset, value) in [
        (32, vehicle.identity),
        (36, motor.identity),
        (40, mission.identity),
        (44, wind.identity),
        (48, HOBBY_SPATIAL_ENVIRONMENT_ID),
        (64, mission.case_seed),
        (68, variation_checksum),
    ] {
        wu32(output, offset, value);
    }
    output[52..54].copy_from_slice(&(KST8_FRAME_LENGTH as u16).to_le_bytes());
    w32(output, 56, mission.telemetry_period.raw());
    w32(output, 60, mission.max_mission_time.raw());
    seal_phase8_record(output)?;
    Ok(())
}
pub fn parse_kst8_header(input: &[u8]) -> Result<SpatialTelemetryHeader, Kst8Error> {
    let common = validate_phase8_record(input, Phase8RecordKind::TelemetryHeader)?;
    if u16::from_le_bytes([input[52], input[53]]) as usize != KST8_FRAME_LENGTH {
        return Err(Kst8Error::FrameLength);
    }
    if !zeros(input, 54, 56)
        || !zeros(input, 72, KST8_HEADER_LENGTH - 4)
        || ru32(input, 48) != HOBBY_SPATIAL_ENVIRONMENT_ID
    {
        return Err(Kst8Error::Reserved);
    }
    Ok(SpatialTelemetryHeader {
        stream_identity: common.identity,
        vehicle_identity: ru32(input, 32),
        motor_identity: ru32(input, 36),
        mission_identity: ru32(input, 40),
        wind_identity: ru32(input, 44),
        telemetry_period: SpatialTime::from_raw(r32(input, 56)),
        max_mission_time: SpatialTime::from_raw(r32(input, 60)),
        case_seed: ru32(input, 64),
        variation_checksum: ru32(input, 68),
    })
}
pub fn encode_kst8_frame(
    snapshot: Phase8MissionSnapshot,
    step: u32,
    checksum: u32,
    output: &mut [u8; KST8_FRAME_LENGTH],
) -> Result<(), Kst8Error> {
    output.fill(0);
    wu32(output, 0, step);
    w32(output, 4, snapshot.state.time.raw());
    for (i, v) in [
        snapshot.state.position.x(),
        snapshot.state.position.y(),
        snapshot.state.position.z(),
    ]
    .iter()
    .enumerate()
    {
        w32(output, 8 + i * 4, *v)
    }
    for (i, v) in [
        snapshot.state.velocity.x(),
        snapshot.state.velocity.y(),
        snapshot.state.velocity.z(),
    ]
    .iter()
    .enumerate()
    {
        w32(output, 20 + i * 4, *v)
    }
    for (i, v) in [
        snapshot.state.acceleration.x(),
        snapshot.state.acceleration.y(),
        snapshot.state.acceleration.z(),
    ]
    .iter()
    .enumerate()
    {
        w32(output, 32 + i * 4, *v)
    }
    for (i, v) in [
        snapshot.state.attitude.w(),
        snapshot.state.attitude.x(),
        snapshot.state.attitude.y(),
        snapshot.state.attitude.z(),
    ]
    .iter()
    .enumerate()
    {
        w32(output, 44 + i * 4, *v)
    }
    for (i, v) in [
        snapshot.state.angular_rate.x(),
        snapshot.state.angular_rate.y(),
        snapshot.state.angular_rate.z(),
    ]
    .iter()
    .enumerate()
    {
        w32(output, 60 + i * 4, *v)
    }
    for (o, v) in [
        (72, snapshot.mass.mass.raw()),
        (76, snapshot.mass.propellant_remaining.raw()),
        (80, snapshot.thrust_q13),
        (84, snapshot.aero.mach_q24),
        (88, snapshot.aero.angle_of_attack_q28),
        (92, snapshot.aero.dynamic_pressure_q13),
        (96, snapshot.aero.static_margin_q24),
    ] {
        w32(output, o, v)
    }
    for (i, v) in snapshot.wind_q22.iter().enumerate() {
        w32(output, 100 + i * 4, *v)
    }
    output[112] = snapshot.phase as u8;
    output[114..116].copy_from_slice(&snapshot.events.to_le_bytes());
    wu32(output, 116, checksum);
    let frame_checksum = crc32_ieee(&output[..KST8_FRAME_LENGTH - 4]);
    wu32(output, KST8_FRAME_LENGTH - 4, frame_checksum);
    Ok(())
}
fn phase(v: u8) -> Result<HobbySpatialPhase, Kst8Error> {
    match v {
        0 => Ok(HobbySpatialPhase::ConstrainedPowered),
        1 => Ok(HobbySpatialPhase::PoweredFlight),
        2 => Ok(HobbySpatialPhase::Coast),
        3 => Ok(HobbySpatialPhase::DrogueRecovery),
        4 => Ok(HobbySpatialPhase::MainRecovery),
        5 => Ok(HobbySpatialPhase::Complete),
        6 => Ok(HobbySpatialPhase::Failed),
        _ => Err(Kst8Error::Phase),
    }
}
pub fn parse_kst8_frame(input: &[u8]) -> Result<(Phase8MissionSnapshot, u32, u32), Kst8Error> {
    if input.len() != KST8_FRAME_LENGTH {
        return Err(Kst8Error::Length);
    }
    if input[113] != 0 || !zeros(input, 120, KST8_FRAME_LENGTH - 4) {
        return Err(Kst8Error::Reserved);
    }
    if ru32(input, KST8_FRAME_LENGTH - 4) != crc32_ieee(&input[..KST8_FRAME_LENGTH - 4]) {
        return Err(Kst8Error::Checksum);
    }
    let state = HobbySpatialState {
        time: SpatialTime::from_raw(r32(input, 4)),
        position: EnuPosition::new(r32(input, 8), r32(input, 12), r32(input, 16)),
        velocity: EnuVelocity::new(r32(input, 20), r32(input, 24), r32(input, 28)),
        acceleration: EnuAcceleration::new(r32(input, 32), r32(input, 36), r32(input, 40)),
        attitude: QuaternionQ30::new(
            r32(input, 44),
            r32(input, 48),
            r32(input, 52),
            r32(input, 56),
        ),
        angular_rate: BodyAngularRate::new(r32(input, 60), r32(input, 64), r32(input, 68)),
    };
    let snapshot = Phase8MissionSnapshot {
        state,
        phase: phase(input[112])?,
        events: u16::from_le_bytes([input[114], input[115]]),
        mass: SpatialMassProperties {
            mass: SpatialMass::from_raw(r32(input, 72)),
            propellant_remaining: SpatialMass::from_raw(r32(input, 76)),
            cg_from_nose: SpatialMomentArm::ZERO,
            inertia: [SpatialInertia::ZERO; 3],
        },
        thrust_q13: r32(input, 80),
        aero: SpatialAeroState {
            mach_q24: r32(input, 84),
            angle_of_attack_q28: r32(input, 88),
            dynamic_pressure_q13: r32(input, 92),
            axial_drag_q13: 0,
            normal_force_q13: 0,
            static_margin_q24: r32(input, 96),
        },
        wind_q22: [r32(input, 100), r32(input, 104), r32(input, 108)],
    };
    Ok((snapshot, ru32(input, 0), ru32(input, 116)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase8_world::HobbySpatialState;
    #[test]
    fn frame_rejects_corruption() {
        let snapshot = Phase8MissionSnapshot {
            state: HobbySpatialState::at_rest(EnuPosition::ZERO, QuaternionQ30::IDENTITY),
            phase: HobbySpatialPhase::Coast,
            events: 3,
            mass: SpatialMassProperties {
                mass: SpatialMass::from_raw(1),
                cg_from_nose: SpatialMomentArm::ZERO,
                inertia: [SpatialInertia::ZERO; 3],
                propellant_remaining: SpatialMass::ZERO,
            },
            thrust_q13: 0,
            aero: SpatialAeroState::ZERO,
            wind_q22: [0; 3],
        };
        let mut bytes = [0u8; KST8_FRAME_LENGTH];
        encode_kst8_frame(snapshot, 1, 2, &mut bytes).unwrap();
        assert_eq!(parse_kst8_frame(&bytes).unwrap().1, 1);
        bytes[20] ^= 1;
        assert!(parse_kst8_frame(&bytes).is_err());
    }
}
