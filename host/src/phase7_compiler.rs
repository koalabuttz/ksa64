//! Offline compiler from reviewable Phase 7 JSON into bounded binary packs.

use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KMP7_MAX_KNOTS, KVP7_LENGTH};
use ksa64_core::phase7_numeric::*;
use ksa64_core::phase7_pack::{
    encode_mission_pack, encode_motor_pack, encode_vehicle_pack, HobbyMissionPack, MotorKnot,
    MotorPack, Phase7PackError, VerticalVehiclePack,
};
use ksa64_core::scenario::fnv1a_32;
use serde::Deserialize;

#[derive(Debug)]
pub enum CompileError {
    Json(serde_json::Error),
    Schema,
    Profile,
    Decimal,
    Range,
    Count,
    Identity,
    Pack(Phase7PackError),
}

impl From<serde_json::Error> for CompileError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<Phase7PackError> for CompileError {
    fn from(value: Phase7PackError) -> Self {
        Self::Pack(value)
    }
}

#[derive(Deserialize)]
struct VehicleSource {
    schema: String,
    identity: String,
    profile: String,
    dry_mass_kg: String,
    length_m: String,
    diameter_m: String,
    reference_area_m2: String,
    body_cd: String,
    drogue_cda_m2: String,
    main_cda_m2: String,
}

#[derive(Deserialize)]
struct MotorSource {
    schema: String,
    identity: String,
    loaded_mass_kg: String,
    propellant_mass_kg: String,
    total_impulse_ns: String,
    burn_time_s: String,
    knots: Vec<[String; 2]>,
}

#[derive(Deserialize)]
struct MissionSource {
    schema: String,
    identity: String,
    vehicle_identity: String,
    motor_identity: String,
    environment: String,
    launch_altitude_m: String,
    rail_length_m: String,
    main_deployment_altitude_m: String,
    drogue_inflation_time_s: String,
    main_inflation_time_s: String,
    max_mission_time_s: String,
    telemetry_period_s: String,
    minimum_rail_exit_velocity_mps: String,
}

pub struct CompiledPacks {
    pub vehicle: [u8; KVP7_LENGTH],
    pub motor: [u8; KMP7_LENGTH],
    pub mission: [u8; KMC7_LENGTH],
}

fn parse_scaled(text: &str, fractional_bits: u8) -> Result<i32, CompileError> {
    let text = text.trim();
    if text.is_empty() || text.contains(['e', 'E', '+']) {
        return Err(CompileError::Decimal);
    }
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let mut parts = unsigned.split('.');
    let whole = parts.next().ok_or(CompileError::Decimal)?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|value| value.is_ascii_digit())
        || !fraction.bytes().all(|value| value.is_ascii_digit())
        || fraction.len() > 30
    {
        return Err(CompileError::Decimal);
    }
    let whole: i128 = whole.parse().map_err(|_| CompileError::Decimal)?;
    let fraction_value: i128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().map_err(|_| CompileError::Decimal)?
    };
    let denominator = 10i128.pow(fraction.len() as u32);
    let numerator = whole
        .checked_mul(denominator)
        .and_then(|value| value.checked_add(fraction_value))
        .ok_or(CompileError::Range)?;
    let scaled_numerator = numerator
        .checked_mul(1i128 << fractional_bits)
        .ok_or(CompileError::Range)?;
    let rounded = (scaled_numerator + denominator / 2) / denominator;
    let signed = if negative { -rounded } else { rounded };
    i32::try_from(signed).map_err(|_| CompileError::Range)
}

fn identity(text: &str) -> Result<u32, CompileError> {
    if text.is_empty() || !text.is_ascii() {
        return Err(CompileError::Identity);
    }
    let result = fnv1a_32(text.as_bytes());
    if result == 0 {
        Err(CompileError::Identity)
    } else {
        Ok(result)
    }
}

pub fn compile_sources(
    vehicle_json: &[u8],
    motor_json: &[u8],
    mission_json: &[u8],
) -> Result<CompiledPacks, CompileError> {
    let vehicle: VehicleSource = serde_json::from_slice(vehicle_json)?;
    let motor: MotorSource = serde_json::from_slice(motor_json)?;
    let mission: MissionSource = serde_json::from_slice(mission_json)?;
    if vehicle.schema != "ksa64.vehicle-source-v1"
        || motor.schema != "ksa64.motor-source-v1"
        || mission.schema != "ksa64.mission-source-v1"
    {
        return Err(CompileError::Schema);
    }
    if vehicle.profile != "HobbyVerticalV1" {
        return Err(CompileError::Profile);
    }
    let vehicle_identity = identity(&vehicle.identity)?;
    let motor_identity = identity(&motor.identity)?;
    if mission.vehicle_identity != vehicle.identity || mission.motor_identity != motor.identity {
        return Err(CompileError::Identity);
    }
    if identity(&mission.environment)? != HOBBY_ENVIRONMENT_ID {
        return Err(CompileError::Identity);
    }
    let vehicle_pack = VerticalVehiclePack {
        identity: vehicle_identity,
        dry_mass: HobbyMass::from_raw(parse_scaled(
            &vehicle.dry_mass_kg,
            HOBBY_MASS_FRACTIONAL_BITS,
        )?),
        length: HobbyAltitude::from_raw(parse_scaled(
            &vehicle.length_m,
            HOBBY_ALTITUDE_FRACTIONAL_BITS,
        )?),
        diameter: HobbyAltitude::from_raw(parse_scaled(
            &vehicle.diameter_m,
            HOBBY_ALTITUDE_FRACTIONAL_BITS,
        )?),
        reference_area: HobbyArea::from_raw(parse_scaled(
            &vehicle.reference_area_m2,
            HOBBY_AREA_FRACTIONAL_BITS,
        )?),
        body_cd_q16: parse_scaled(&vehicle.body_cd, 16)?,
        drogue_cda: HobbyRecoveryCda::from_raw(parse_scaled(
            &vehicle.drogue_cda_m2,
            HOBBY_RECOVERY_CDA_FRACTIONAL_BITS,
        )?),
        main_cda: HobbyRecoveryCda::from_raw(parse_scaled(
            &vehicle.main_cda_m2,
            HOBBY_RECOVERY_CDA_FRACTIONAL_BITS,
        )?),
    };
    if motor.knots.len() > KMP7_MAX_KNOTS {
        return Err(CompileError::Count);
    }
    let mut knots = [MotorKnot::ZERO; KMP7_MAX_KNOTS];
    for (index, knot) in motor.knots.iter().enumerate() {
        knots[index] = MotorKnot {
            time: HobbyTime::from_raw(parse_scaled(&knot[0], HOBBY_TIME_FRACTIONAL_BITS)?),
            thrust_raw_q13: parse_scaled(&knot[1], HOBBY_FORCE_FRACTIONAL_BITS)?,
        };
    }
    let motor_pack = MotorPack {
        identity: motor_identity,
        loaded_mass: HobbyMass::from_raw(parse_scaled(
            &motor.loaded_mass_kg,
            HOBBY_MASS_FRACTIONAL_BITS,
        )?),
        propellant_mass: HobbyMass::from_raw(parse_scaled(
            &motor.propellant_mass_kg,
            HOBBY_MASS_FRACTIONAL_BITS,
        )?),
        total_impulse_raw_q16: parse_scaled(&motor.total_impulse_ns, 16)?,
        burn_time: HobbyTime::from_raw(parse_scaled(
            &motor.burn_time_s,
            HOBBY_TIME_FRACTIONAL_BITS,
        )?),
        knot_count: motor.knots.len() as u8,
        knots,
    };
    let mission_pack = HobbyMissionPack {
        identity: identity(&mission.identity)?,
        vehicle_identity,
        motor_identity,
        environment_identity: HOBBY_ENVIRONMENT_ID,
        launch_altitude: HobbyAltitude::from_raw(parse_scaled(
            &mission.launch_altitude_m,
            HOBBY_ALTITUDE_FRACTIONAL_BITS,
        )?),
        rail_length: HobbyAltitude::from_raw(parse_scaled(
            &mission.rail_length_m,
            HOBBY_ALTITUDE_FRACTIONAL_BITS,
        )?),
        main_deployment_altitude: HobbyAltitude::from_raw(parse_scaled(
            &mission.main_deployment_altitude_m,
            HOBBY_ALTITUDE_FRACTIONAL_BITS,
        )?),
        drogue_inflation_time: HobbyTime::from_raw(parse_scaled(
            &mission.drogue_inflation_time_s,
            HOBBY_TIME_FRACTIONAL_BITS,
        )?),
        main_inflation_time: HobbyTime::from_raw(parse_scaled(
            &mission.main_inflation_time_s,
            HOBBY_TIME_FRACTIONAL_BITS,
        )?),
        max_mission_time: HobbyTime::from_raw(parse_scaled(
            &mission.max_mission_time_s,
            HOBBY_TIME_FRACTIONAL_BITS,
        )?),
        telemetry_period: HobbyTime::from_raw(parse_scaled(
            &mission.telemetry_period_s,
            HOBBY_TIME_FRACTIONAL_BITS,
        )?),
        minimum_rail_exit_velocity: HobbyVelocity::from_raw(parse_scaled(
            &mission.minimum_rail_exit_velocity_mps,
            HOBBY_VELOCITY_FRACTIONAL_BITS,
        )?),
    };
    let mut compiled = CompiledPacks {
        vehicle: [0; KVP7_LENGTH],
        motor: [0; KMP7_LENGTH],
        mission: [0; KMC7_LENGTH],
    };
    encode_vehicle_pack(vehicle_pack, &mut compiled.vehicle)?;
    encode_motor_pack(&motor_pack, &mut compiled.motor)?;
    encode_mission_pack(mission_pack, &mut compiled.mission)?;
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_decimal_parser_rounds_without_binary_float() {
        assert_eq!(parse_scaled("0.01", 18).unwrap(), 2_621);
        assert_eq!(parse_scaled("-0.5", 13).unwrap(), -4_096);
        assert!(parse_scaled("1e3", 16).is_err());
    }
}
