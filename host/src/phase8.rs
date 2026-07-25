//! Host trace/export wrapper for the one portable Phase 8 mission machine.

use ksa64_core::phase8_format::KWP8_MAX_WIND_KNOTS;
use ksa64_core::phase8_mission::{
    Phase8MissionError, Phase8MissionMachine, Phase8MissionResult, Phase8MissionSnapshot,
    SpatialMissionVariation,
};
use ksa64_core::phase8_numeric::{SpatialPosition, SpatialWind};
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack, SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindKnot,
    WindProfilePack,
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Phase8TracePoint {
    pub time_s: f64,
    pub phase: u8,
    pub events: u16,
    pub position_m: [f64; 3],
    pub velocity_mps: [f64; 3],
    pub acceleration_mps2: [f64; 3],
    pub quaternion: [f64; 4],
    pub angular_rate_rad_s: [f64; 3],
    pub mass_kg: f64,
    pub propellant_kg: f64,
    pub thrust_n: f64,
    pub mach: f64,
    pub angle_of_attack_deg: f64,
    pub dynamic_pressure_pa: f64,
    pub static_margin_calibers: f64,
    pub wind_mps: [f64; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct Phase8RunEvidence {
    pub schema: &'static str,
    pub vehicle_identity: u32,
    pub motor_identity: u32,
    pub mission_identity: u32,
    pub wind_identity: u32,
    pub outcome: u8,
    pub checksum: u32,
    pub rail_exit_time_s: f64,
    pub burnout_time_s: f64,
    pub apogee_time_s: f64,
    pub drogue_time_s: f64,
    pub main_time_s: f64,
    pub landing_time_s: f64,
    pub apogee_m: f64,
    pub landing_position_m: [f64; 3],
    pub max_speed_mps: f64,
    pub max_acceleration_mps2: f64,
    pub max_dynamic_pressure_pa: f64,
    pub max_angle_of_attack_deg: f64,
    pub max_angular_rate_rad_s: f64,
    pub max_wind_mps: f64,
    pub trace: Vec<Phase8TracePoint>,
}

fn point(snapshot: Phase8MissionSnapshot) -> Phase8TracePoint {
    let state = snapshot.state;
    Phase8TracePoint {
        time_s: f64::from(state.time.raw()) / f64::from(1 << 18),
        phase: snapshot.phase as u8,
        events: snapshot.events,
        position_m: [
            f64::from(state.position.x()) / f64::from(1 << 13),
            f64::from(state.position.y()) / f64::from(1 << 13),
            f64::from(state.position.z()) / f64::from(1 << 13),
        ],
        velocity_mps: [
            f64::from(state.velocity.x()) / f64::from(1 << 19),
            f64::from(state.velocity.y()) / f64::from(1 << 19),
            f64::from(state.velocity.z()) / f64::from(1 << 19),
        ],
        acceleration_mps2: [
            f64::from(state.acceleration.x()) / f64::from(1 << 19),
            f64::from(state.acceleration.y()) / f64::from(1 << 19),
            f64::from(state.acceleration.z()) / f64::from(1 << 19),
        ],
        quaternion: [
            f64::from(state.attitude.w()) / f64::from(1 << 30),
            f64::from(state.attitude.x()) / f64::from(1 << 30),
            f64::from(state.attitude.y()) / f64::from(1 << 30),
            f64::from(state.attitude.z()) / f64::from(1 << 30),
        ],
        angular_rate_rad_s: [
            f64::from(state.angular_rate.x()) / f64::from(1 << 24),
            f64::from(state.angular_rate.y()) / f64::from(1 << 24),
            f64::from(state.angular_rate.z()) / f64::from(1 << 24),
        ],
        mass_kg: f64::from(snapshot.mass.mass.raw()) / f64::from(1 << 21),
        propellant_kg: f64::from(snapshot.mass.propellant_remaining.raw()) / f64::from(1 << 21),
        thrust_n: f64::from(snapshot.thrust_q13) / f64::from(1 << 13),
        mach: f64::from(snapshot.aero.mach_q24) / f64::from(1 << 24),
        angle_of_attack_deg: f64::from(snapshot.aero.angle_of_attack_q28) / f64::from(1 << 28)
            * 180.0
            / std::f64::consts::PI,
        dynamic_pressure_pa: f64::from(snapshot.aero.dynamic_pressure_q13) / f64::from(1 << 13),
        static_margin_calibers: f64::from(snapshot.aero.static_margin_q24) / f64::from(1 << 24),
        wind_mps: snapshot
            .wind_q22
            .map(|value| f64::from(value) / f64::from(1 << 22)),
    }
}

fn evidence(
    result: Phase8MissionResult,
    identities: [u32; 4],
    trace: Vec<Phase8TracePoint>,
) -> Phase8RunEvidence {
    let seconds = |raw: i32| f64::from(raw) / f64::from(1 << 18);
    let position = |raw: i32| f64::from(raw) / f64::from(1 << 13);
    Phase8RunEvidence {
        schema: "ksa64.phase8-run-evidence-v1",
        vehicle_identity: identities[0],
        motor_identity: identities[1],
        mission_identity: identities[2],
        wind_identity: identities[3],
        outcome: result.outcome as u8,
        checksum: result.checksum,
        rail_exit_time_s: seconds(result.rail_exit.time.raw()),
        burnout_time_s: seconds(result.burnout.time.raw()),
        apogee_time_s: seconds(result.apogee.time.raw()),
        drogue_time_s: seconds(result.drogue.time.raw()),
        main_time_s: seconds(result.main.time.raw()),
        landing_time_s: seconds(result.landing.time.raw()),
        apogee_m: position(result.max_altitude_raw_q13),
        landing_position_m: [
            position(result.landing.position.x()),
            position(result.landing.position.y()),
            position(result.landing.position.z()),
        ],
        max_speed_mps: f64::from(result.max_speed_raw_q19) / f64::from(1 << 19),
        max_acceleration_mps2: f64::from(result.max_acceleration_raw_q19) / f64::from(1 << 19),
        max_dynamic_pressure_pa: f64::from(result.max_dynamic_pressure_raw_q13)
            / f64::from(1 << 13),
        max_angle_of_attack_deg: f64::from(result.max_aoa_raw_q28) / f64::from(1 << 28) * 180.0
            / std::f64::consts::PI,
        max_angular_rate_rad_s: f64::from(result.max_angular_rate_raw_q24) / f64::from(1 << 24),
        max_wind_mps: f64::from(result.max_wind_raw_q22) / f64::from(1 << 22),
        trace,
    }
}

pub fn run_phase8_evidence(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
    variation: SpatialMissionVariation,
) -> Result<Phase8RunEvidence, Phase8MissionError> {
    let mut machine =
        Phase8MissionMachine::new_with_variation(vehicle, motor, mission, wind, variation)?;
    let mut trace = Vec::new();
    let mut next_trace_raw = 0;
    while !machine.is_complete() {
        match machine.step() {
            Ok(snapshot) => {
                if snapshot.state.time.raw() >= next_trace_raw || snapshot.events != 0 {
                    trace.push(point(snapshot));
                    next_trace_raw = snapshot.state.time.raw() + mission.telemetry_period.raw();
                }
            }
            Err(Phase8MissionError::Complete | Phase8MissionError::ModelEnvelopeExceeded) => {}
            Err(error) => return Err(error),
        }
    }
    let result = machine.result().ok_or(Phase8MissionError::Numeric)?;
    Ok(evidence(
        result,
        [
            vehicle.identity,
            motor.identity,
            mission.identity,
            wind.identity,
        ],
        trace,
    ))
}

fn checked_in_packs() -> Result<
    (
        SpatialVehiclePack,
        SpatialMotorPack,
        SpatialMissionPack,
        WindProfilePack,
    ),
    Phase8MissionError,
> {
    Ok((
        parse_spatial_vehicle_pack(include_bytes!("../../phase8/examples/firestorm54.kvp8"))
            .map_err(|_| Phase8MissionError::Configuration)?,
        parse_spatial_motor_pack(include_bytes!("../../phase8/examples/aerotech-i211w.kmp8"))
            .map_err(|_| Phase8MissionError::Configuration)?,
        parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
            .map_err(|_| Phase8MissionError::Configuration)?,
        parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
            .map_err(|_| Phase8MissionError::Configuration)?,
    ))
}

pub fn run_checked_in_phase8() -> Result<Phase8RunEvidence, Phase8MissionError> {
    let (vehicle, motor, mission, wind) = checked_in_packs()?;
    run_phase8_evidence(
        &vehicle,
        &motor,
        mission,
        &wind,
        SpatialMissionVariation::NOMINAL,
    )
}

pub fn run_checked_in_phase8_crosswind(
    east_wind_mps: i32,
) -> Result<Phase8RunEvidence, Phase8MissionError> {
    if !(0..=25).contains(&east_wind_mps) {
        return Err(Phase8MissionError::Configuration);
    }
    let (vehicle, motor, mut mission, _) = checked_in_packs()?;
    let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
    knots[0] = WindKnot {
        altitude: SpatialPosition::ZERO,
        east: SpatialWind::from_raw(east_wind_mps << 22),
        north: SpatialWind::ZERO,
    };
    knots[1] = WindKnot {
        altitude: SpatialPosition::from_raw(100_000 << 13),
        east: SpatialWind::from_raw(east_wind_mps << 22),
        north: SpatialWind::ZERO,
    };
    let wind = WindProfilePack {
        identity: 0x3557_0000 | east_wind_mps as u32,
        gust_seed: 0,
        gust_cadence: ksa64_core::phase8_numeric::SpatialTime::from_raw(1 << 18),
        gust_amplitude_east: SpatialWind::ZERO,
        gust_amplitude_north: SpatialWind::ZERO,
        max_gust: SpatialWind::ZERO,
        knot_count: 2,
        knots,
    };
    mission.wind_identity = wind.identity;
    run_phase8_evidence(
        &vehicle,
        &motor,
        mission,
        &wind,
        SpatialMissionVariation::NOMINAL,
    )
}
