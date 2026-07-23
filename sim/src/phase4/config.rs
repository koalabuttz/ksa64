//! Strict fixed-capacity KSC4 campaign configuration.

use ksa64_core::phase2_scenario::{
    parse_phase2_scenario, Phase2ScenarioError, PHASE2_SCENARIO_IMAGE_LENGTH,
};
use ksa64_interface::crc32_ieee;

use crate::config::PHASE3_CONFIG_LENGTH;

use super::campaign::{
    CampaignConfig, CampaignError, DistributionKind, DistributionSpec, ParameterId,
};
use super::contracts::{
    CAMPAIGN_CONFIG_LENGTH, DISTRIBUTION_RECORD_LENGTH, KSC4_MAGIC, MAX_DISTRIBUTIONS,
};
use super::PHASE4_CONTRACT_ID;

pub const CAMPAIGN_CONFIG_VERSION: u16 = 4;
const HEADER_LENGTH: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignConfigError {
    Length,
    Magic,
    Version,
    Contract,
    BaseScenario,
    BaseChecksum,
    Phase3Checksum,
    Reserved,
    Record,
    Checksum,
    Campaign(CampaignError),
    Phase2(Phase2ScenarioError),
}

fn put_u16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes())
}
fn put_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes())
}
fn put_i32(out: &mut [u8], at: usize, value: i32) {
    put_u32(out, at, value as u32)
}
fn get_u16(input: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([input[at], input[at + 1]])
}
fn get_u32(input: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([input[at], input[at + 1], input[at + 2], input[at + 3]])
}
fn get_i32(input: &[u8], at: usize) -> i32 {
    get_u32(input, at) as i32
}

pub fn write_campaign_config(
    base: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH],
    phase3: &[u8; PHASE3_CONFIG_LENGTH],
    config: &CampaignConfig,
    out: &mut [u8],
) -> Result<(), CampaignConfigError> {
    if out.len() != CAMPAIGN_CONFIG_LENGTH {
        return Err(CampaignConfigError::Length);
    }
    config.validate().map_err(CampaignConfigError::Campaign)?;
    let scenario = parse_phase2_scenario(base).map_err(CampaignConfigError::Phase2)?;
    out.fill(0);
    out[..4].copy_from_slice(&KSC4_MAGIC);
    put_u16(out, 4, CAMPAIGN_CONFIG_VERSION);
    put_u16(out, 6, CAMPAIGN_CONFIG_LENGTH as u16);
    put_u32(out, 8, PHASE4_CONTRACT_ID);
    put_u32(out, 12, scenario.scenario_id());
    put_u32(
        out,
        16,
        crc32_ieee(&base[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]),
    );
    put_u32(out, 20, crc32_ieee(&phase3[..PHASE3_CONFIG_LENGTH - 4]));
    put_u32(out, 24, config.master_seed);
    put_u32(out, 28, config.run_count);
    out[32] = config.distribution_count;
    let mut index = 0;
    while index < config.distribution_count as usize {
        let spec = config.distributions[index];
        let at = HEADER_LENGTH + index * DISTRIBUTION_RECORD_LENGTH;
        out[at] = spec.parameter as u8;
        out[at + 1] = spec.kind as u8;
        out[at + 2] = spec.correlation_group;
        put_i32(out, at + 4, spec.minimum);
        put_i32(out, at + 8, spec.baseline);
        put_i32(out, at + 12, spec.maximum);
        put_i32(out, at + 16, spec.shape);
        let crc = crc32_ieee(&out[at..at + 20]);
        put_u32(out, at + 20, crc);
        index += 1;
    }
    put_u32(out, 120, crc32_ieee(&out[HEADER_LENGTH..]));
    put_u32(out, 124, crc32_ieee(&out[..124]));
    Ok(())
}

pub fn parse_campaign_config(
    bytes: &[u8],
    base: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH],
    phase3: &[u8; PHASE3_CONFIG_LENGTH],
) -> Result<CampaignConfig, CampaignConfigError> {
    if bytes.len() != CAMPAIGN_CONFIG_LENGTH {
        return Err(CampaignConfigError::Length);
    }
    if bytes[..4] != KSC4_MAGIC {
        return Err(CampaignConfigError::Magic);
    }
    if get_u16(bytes, 4) != CAMPAIGN_CONFIG_VERSION
        || get_u16(bytes, 6) as usize != CAMPAIGN_CONFIG_LENGTH
    {
        return Err(CampaignConfigError::Version);
    }
    if get_u32(bytes, 8) != PHASE4_CONTRACT_ID {
        return Err(CampaignConfigError::Contract);
    }
    let scenario = parse_phase2_scenario(base).map_err(CampaignConfigError::Phase2)?;
    if get_u32(bytes, 12) != scenario.scenario_id() {
        return Err(CampaignConfigError::BaseScenario);
    }
    if get_u32(bytes, 16) != crc32_ieee(&base[..PHASE2_SCENARIO_IMAGE_LENGTH - 4]) {
        return Err(CampaignConfigError::BaseChecksum);
    }
    if get_u32(bytes, 20) != crc32_ieee(&phase3[..PHASE3_CONFIG_LENGTH - 4]) {
        return Err(CampaignConfigError::Phase3Checksum);
    }
    if bytes[33..120].iter().any(|&byte| byte != 0) {
        return Err(CampaignConfigError::Reserved);
    }
    if crc32_ieee(&bytes[HEADER_LENGTH..]) != get_u32(bytes, 120)
        || crc32_ieee(&bytes[..124]) != get_u32(bytes, 124)
    {
        return Err(CampaignConfigError::Checksum);
    }
    let count = bytes[32] as usize;
    if count > MAX_DISTRIBUTIONS {
        return Err(CampaignConfigError::Record);
    }
    let mut config = CampaignConfig::empty(get_u32(bytes, 28));
    config.master_seed = get_u32(bytes, 24);
    let mut index = 0;
    while index < MAX_DISTRIBUTIONS {
        let at = HEADER_LENGTH + index * DISTRIBUTION_RECORD_LENGTH;
        if index >= count {
            if bytes[at..at + DISTRIBUTION_RECORD_LENGTH]
                .iter()
                .any(|&byte| byte != 0)
            {
                return Err(CampaignConfigError::Reserved);
            }
        } else {
            if bytes[at + 3] != 0 || crc32_ieee(&bytes[at..at + 20]) != get_u32(bytes, at + 20) {
                return Err(CampaignConfigError::Record);
            }
            let spec = DistributionSpec {
                parameter: ParameterId::from_byte(bytes[at]).ok_or(CampaignConfigError::Record)?,
                kind: DistributionKind::from_byte(bytes[at + 1])
                    .ok_or(CampaignConfigError::Record)?,
                correlation_group: bytes[at + 2],
                minimum: get_i32(bytes, at + 4),
                baseline: get_i32(bytes, at + 8),
                maximum: get_i32(bytes, at + 12),
                shape: get_i32(bytes, at + 16),
            };
            config.push(spec).map_err(CampaignConfigError::Campaign)?;
        }
        index += 1;
    }
    config.validate().map_err(CampaignConfigError::Campaign)?;
    Ok(config)
}
