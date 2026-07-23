//! Strict fixed-capacity Phase 3 configuration image (`KSC3`).

use crate::mission::MissionCase;
use ksa64_core::phase2_scenario::{
    parse_phase2_scenario, Phase2ScenarioError, KSA2A_NOMINAL_SCENARIO_ID,
    PHASE2_SCENARIO_IMAGE_LENGTH,
};
use ksa64_interface::crc32_ieee;

pub const PHASE3_CONFIG_LENGTH: usize = 96;
pub const PHASE3_CONFIG_VERSION: u16 = 3;
pub const PHASE3_CONFIG_CONTRACT_ID: u32 = 0x0300_0001;
const MAGIC: [u8; 4] = *b"KSC3";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Length,
    Magic,
    Version,
    Contract,
    BaseScenario,
    BaseChecksum,
    Reserved,
    Range,
    Checksum,
    Phase2(Phase2ScenarioError),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase3Config {
    pub base_scenario_id: u32,
    pub base_checksum: u32,
    pub seed: u32,
    pub case: MissionCase,
    pub actuator_lag_steps: u16,
    pub actuator_slew_raw: u16,
    pub actuator_max_pitch: u16,
    pub tracking_limit: u16,
    pub tracking_steps: u8,
    pub altimeter_divisor: u8,
    pub gps_divisor: u8,
    pub altimeter_ceiling_q12: i32,
    pub gps_acquire_step: u32,
    pub altimeter_dropout_start: u32,
    pub altimeter_dropout_end: u32,
    pub gps_outage_start: u32,
    pub gps_outage_end: u32,
    pub steering_stuck_step: u32,
    pub steering_jam_pitch: u16,
}
fn put_u16(o: &mut [u8], i: usize, v: u16) {
    o[i..i + 2].copy_from_slice(&v.to_le_bytes())
}
fn put_u32(o: &mut [u8], i: usize, v: u32) {
    o[i..i + 4].copy_from_slice(&v.to_le_bytes())
}
fn get_u16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn get_u32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn case_byte(case: MissionCase) -> u8 {
    match case {
        MissionCase::Nominal => 0,
        MissionCase::AltimeterDropout => 1,
        MissionCase::GpsOutage => 2,
        MissionCase::SteeringStuck => 3,
    }
}
fn parse_case(v: u8) -> Result<MissionCase, ConfigError> {
    match v {
        0 => Ok(MissionCase::Nominal),
        1 => Ok(MissionCase::AltimeterDropout),
        2 => Ok(MissionCase::GpsOutage),
        3 => Ok(MissionCase::SteeringStuck),
        _ => Err(ConfigError::Range),
    }
}
pub fn write_phase3_config(
    base: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH],
    case: MissionCase,
    out: &mut [u8],
) -> Result<(), ConfigError> {
    if out.len() != PHASE3_CONFIG_LENGTH {
        return Err(ConfigError::Length);
    }
    let scenario = parse_phase2_scenario(base).map_err(ConfigError::Phase2)?;
    if scenario.scenario_id() != KSA2A_NOMINAL_SCENARIO_ID {
        return Err(ConfigError::BaseScenario);
    }
    out.fill(0);
    out[..4].copy_from_slice(&MAGIC);
    put_u16(out, 4, PHASE3_CONFIG_VERSION);
    put_u16(out, 6, PHASE3_CONFIG_LENGTH as u16);
    put_u32(out, 8, PHASE3_CONFIG_CONTRACT_ID);
    put_u32(out, 12, scenario.scenario_id());
    put_u32(out, 16, crc32_ieee(base));
    put_u32(out, 20, case.seed());
    out[24] = case_byte(case);
    put_u16(out, 28, 4);
    put_u16(out, 30, 228);
    put_u16(out, 32, 20_025);
    put_u16(out, 34, 364);
    out[36] = 16;
    out[37] = 2;
    out[38] = 8;
    put_u32(out, 40, (80 * 4096) as u32);
    put_u32(out, 44, 960);
    if matches!(case, MissionCase::AltimeterDropout) {
        put_u32(out, 48, 360);
        put_u32(out, 52, 480)
    }
    if matches!(case, MissionCase::GpsOutage) {
        put_u32(out, 56, 2080);
        put_u32(out, 60, 2560)
    }
    if matches!(case, MissionCase::SteeringStuck) {
        put_u32(out, 64, 2080);
        put_u16(out, 68, 14_564)
    }
    put_u32(out, 92, crc32_ieee(&out[..92]));
    Ok(())
}
pub fn parse_phase3_config(
    bytes: &[u8],
    base: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH],
) -> Result<Phase3Config, ConfigError> {
    if bytes.len() != PHASE3_CONFIG_LENGTH {
        return Err(ConfigError::Length);
    }
    if bytes[..4] != MAGIC {
        return Err(ConfigError::Magic);
    }
    if get_u16(bytes, 4) != PHASE3_CONFIG_VERSION
        || get_u16(bytes, 6) as usize != PHASE3_CONFIG_LENGTH
    {
        return Err(ConfigError::Version);
    }
    if get_u32(bytes, 8) != PHASE3_CONFIG_CONTRACT_ID {
        return Err(ConfigError::Contract);
    }
    let scenario = parse_phase2_scenario(base).map_err(ConfigError::Phase2)?;
    if get_u32(bytes, 12) != scenario.scenario_id() {
        return Err(ConfigError::BaseScenario);
    }
    if get_u32(bytes, 16) != crc32_ieee(base) {
        return Err(ConfigError::BaseChecksum);
    }
    if bytes[25] != 0
        || bytes[26] != 0
        || bytes[27] != 0
        || bytes[39] != 0
        || bytes[70..92].iter().any(|&b| b != 0)
    {
        return Err(ConfigError::Reserved);
    }
    if crc32_ieee(&bytes[..92]) != get_u32(bytes, 92) {
        return Err(ConfigError::Checksum);
    }
    let config = Phase3Config {
        base_scenario_id: get_u32(bytes, 12),
        base_checksum: get_u32(bytes, 16),
        seed: get_u32(bytes, 20),
        case: parse_case(bytes[24])?,
        actuator_lag_steps: get_u16(bytes, 28),
        actuator_slew_raw: get_u16(bytes, 30),
        actuator_max_pitch: get_u16(bytes, 32),
        tracking_limit: get_u16(bytes, 34),
        tracking_steps: bytes[36],
        altimeter_divisor: bytes[37],
        gps_divisor: bytes[38],
        altimeter_ceiling_q12: get_u32(bytes, 40) as i32,
        gps_acquire_step: get_u32(bytes, 44),
        altimeter_dropout_start: get_u32(bytes, 48),
        altimeter_dropout_end: get_u32(bytes, 52),
        gps_outage_start: get_u32(bytes, 56),
        gps_outage_end: get_u32(bytes, 60),
        steering_stuck_step: get_u32(bytes, 64),
        steering_jam_pitch: get_u16(bytes, 68),
    };
    if config.seed == 0
        || config.actuator_lag_steps != 4
        || config.actuator_slew_raw != 228
        || config.actuator_max_pitch != 20_025
        || config.tracking_limit != 364
        || config.tracking_steps != 16
        || config.altimeter_divisor != 2
        || config.gps_divisor != 8
        || config.altimeter_ceiling_q12 != 80 * 4096
        || config.gps_acquire_step != 960
    {
        return Err(ConfigError::Range);
    }
    Ok(config)
}
