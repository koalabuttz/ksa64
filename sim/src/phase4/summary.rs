//! Fixed 128-byte KSR4 run summaries.

use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_core::planar::{OrbitClass, PlanarTruthState};
use ksa64_interface::crc32_ieee;

use crate::mission::{MissionOutcome, MissionResult};

use super::campaign::RunSpec;
use super::contracts::{KSR4_MAGIC, RUN_SUMMARY_LENGTH};
use super::PHASE4_CONTRACT_ID;

pub const KSR4_VERSION: u16 = 4;
const CRC_OFFSET: usize = RUN_SUMMARY_LENGTH - 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RunOutcome {
    StableOrbit = 0,
    Suborbital = 1,
    Impact = 2,
    Escape = 3,
    Abort = 4,
    Error = 5,
}
impl RunOutcome {
    pub const COUNT: usize = 6;
    pub const fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::StableOrbit),
            1 => Some(Self::Suborbital),
            2 => Some(Self::Impact),
            3 => Some(Self::Escape),
            4 => Some(Self::Abort),
            5 => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunSummary {
    pub campaign_crc32: u32,
    pub scenario_id: u32,
    pub run_index: u32,
    pub sensor_seed: u32,
    pub variation_checksum: u32,
    pub outcome: RunOutcome,
    pub terminal_step: u32,
    pub terminal_radius_q12: i32,
    pub terminal_downrange_q32: i32,
    pub terminal_radial_velocity_q24: i32,
    pub terminal_angular_momentum_q14: i32,
    pub terminal_mass_q12: i32,
    pub cutoff_step: u32,
    pub cutoff_radius_q12: i32,
    pub cutoff_downrange_q32: i32,
    pub cutoff_radial_velocity_q24: i32,
    pub cutoff_angular_momentum_q14: i32,
    pub max_dynamic_pressure_q16: i32,
    pub max_proper_acceleration_q28: i32,
    pub navigation_position_error_q12: i32,
    pub navigation_velocity_error_q24: i32,
    pub truth_checksum: u32,
    pub sensor_checksum: u32,
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
    pub alarms: u16,
    pub flight_mode: u8,
    pub active_stage: u8,
}

fn abs_i32(value: i32) -> i32 {
    value.checked_abs().unwrap_or(i32::MAX)
}
fn tangential_velocity_q24(state: PlanarTruthState) -> i32 {
    let value = ((state.specific_angular_momentum().raw() as i64) << 22)
        / state.radius().raw().max(1) as i64;
    value.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}
fn downrange_error_q12(truth: PlanarTruthState, estimate_q32: i32) -> i32 {
    let turns = truth.downrange().raw().wrapping_sub(estimate_q32) as i64;
    let circumference_factor_q12 = 25_736i64; // 2*pi in Q12.
    let metres_q12 =
        (((turns * truth.radius().raw() as i64) >> 32) * circumference_factor_q12) >> 12;
    abs_i32(metres_q12.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

impl RunSummary {
    pub fn from_result(
        scenario: &Phase2Scenario,
        campaign_crc32: u32,
        run: RunSpec,
        result: MissionResult,
    ) -> Self {
        let outcome = if result.outcome == MissionOutcome::Abort {
            RunOutcome::Abort
        } else if result.outcome == MissionOutcome::Impact {
            RunOutcome::Impact
        } else {
            match result.orbit.map(|orbit| orbit.class()) {
                Some(OrbitClass::StableOrbit) => RunOutcome::StableOrbit,
                Some(OrbitClass::Suborbital) => RunOutcome::Suborbital,
                Some(OrbitClass::Impact) | None => RunOutcome::Impact,
                Some(OrbitClass::Escape) => RunOutcome::Escape,
            }
        };
        let nav = result.cutoff_navigation;
        let radius_error = abs_i32(
            result
                .cutoff_truth
                .radius()
                .raw()
                .saturating_sub(nav.radius_q12),
        );
        let position_error =
            radius_error.max(downrange_error_q12(result.cutoff_truth, nav.downrange_q32));
        let radial_error = abs_i32(
            result
                .cutoff_truth
                .radial_velocity()
                .raw()
                .saturating_sub(nav.radial_velocity_q24),
        );
        let tangential_error = abs_i32(
            tangential_velocity_q24(result.cutoff_truth)
                .saturating_sub(nav.tangential_velocity_q24),
        );
        Self {
            campaign_crc32,
            scenario_id: scenario.scenario_id(),
            run_index: run.index,
            sensor_seed: run.sensor_seed,
            variation_checksum: run.variation.checksum(),
            outcome,
            terminal_step: result.truth.step(),
            terminal_radius_q12: result.truth.radius().raw(),
            terminal_downrange_q32: result.truth.downrange().raw(),
            terminal_radial_velocity_q24: result.truth.radial_velocity().raw(),
            terminal_angular_momentum_q14: result.truth.specific_angular_momentum().raw(),
            terminal_mass_q12: result.truth.total_mass().raw(),
            cutoff_step: result.cutoff_step,
            cutoff_radius_q12: result.cutoff_truth.radius().raw(),
            cutoff_downrange_q32: result.cutoff_truth.downrange().raw(),
            cutoff_radial_velocity_q24: result.cutoff_truth.radial_velocity().raw(),
            cutoff_angular_momentum_q14: result.cutoff_truth.specific_angular_momentum().raw(),
            max_dynamic_pressure_q16: result.max_dynamic_pressure.raw(),
            max_proper_acceleration_q28: result.max_proper_acceleration.raw(),
            navigation_position_error_q12: position_error,
            navigation_velocity_error_q24: radial_error.max(tangential_error),
            truth_checksum: result.truth_checksum,
            sensor_checksum: result.sensor_checksum,
            navigation_checksum: result.nav_checksum,
            flight_checksum: result.flight_checksum,
            alarms: result.flight_status.alarms,
            flight_mode: result.flight_status.mode as u8,
            active_stage: result.truth.active_stage(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryError {
    Length,
    Magic,
    Version,
    Contract,
    Reserved,
    Enum,
    Checksum,
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_i32(out: &mut [u8], offset: usize, value: i32) {
    put_u32(out, offset, value as u32);
}
fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn get_i32(bytes: &[u8], offset: usize) -> i32 {
    get_u32(bytes, offset) as i32
}

pub fn write_ksr4(summary: &RunSummary, out: &mut [u8]) -> Result<(), SummaryError> {
    if out.len() != RUN_SUMMARY_LENGTH {
        return Err(SummaryError::Length);
    }
    out.fill(0);
    out[0..4].copy_from_slice(&KSR4_MAGIC);
    put_u16(out, 4, KSR4_VERSION);
    put_u16(out, 6, RUN_SUMMARY_LENGTH as u16);
    put_u32(out, 8, PHASE4_CONTRACT_ID);
    put_u32(out, 12, summary.campaign_crc32);
    put_u32(out, 16, summary.scenario_id);
    put_u32(out, 20, summary.run_index);
    put_u32(out, 24, summary.sensor_seed);
    put_u32(out, 28, summary.variation_checksum);
    out[32] = summary.outcome as u8;
    out[33] = summary.active_stage;
    out[34] = summary.flight_mode;
    put_u32(out, 36, summary.terminal_step);
    put_i32(out, 40, summary.terminal_radius_q12);
    put_i32(out, 44, summary.terminal_downrange_q32);
    put_i32(out, 48, summary.terminal_radial_velocity_q24);
    put_i32(out, 52, summary.terminal_angular_momentum_q14);
    put_i32(out, 56, summary.terminal_mass_q12);
    put_u32(out, 60, summary.cutoff_step);
    put_i32(out, 64, summary.cutoff_radius_q12);
    put_i32(out, 68, summary.cutoff_downrange_q32);
    put_i32(out, 72, summary.cutoff_radial_velocity_q24);
    put_i32(out, 76, summary.cutoff_angular_momentum_q14);
    put_i32(out, 80, summary.max_dynamic_pressure_q16);
    put_i32(out, 84, summary.max_proper_acceleration_q28);
    put_i32(out, 88, summary.navigation_position_error_q12);
    put_i32(out, 92, summary.navigation_velocity_error_q24);
    put_u32(out, 96, summary.truth_checksum);
    put_u32(out, 100, summary.sensor_checksum);
    put_u32(out, 104, summary.navigation_checksum);
    put_u32(out, 108, summary.flight_checksum);
    put_u16(out, 112, summary.alarms);
    put_u32(out, CRC_OFFSET, crc32_ieee(&out[..CRC_OFFSET]));
    Ok(())
}

pub fn parse_ksr4(bytes: &[u8]) -> Result<RunSummary, SummaryError> {
    if bytes.len() != RUN_SUMMARY_LENGTH {
        return Err(SummaryError::Length);
    }
    if bytes[0..4] != KSR4_MAGIC {
        return Err(SummaryError::Magic);
    }
    if get_u16(bytes, 4) != KSR4_VERSION || get_u16(bytes, 6) != RUN_SUMMARY_LENGTH as u16 {
        return Err(SummaryError::Version);
    }
    if get_u32(bytes, 8) != PHASE4_CONTRACT_ID {
        return Err(SummaryError::Contract);
    }
    if bytes[35] != 0 || bytes[114..CRC_OFFSET].iter().any(|&value| value != 0) {
        return Err(SummaryError::Reserved);
    }
    let outcome = RunOutcome::from_byte(bytes[32]).ok_or(SummaryError::Enum)?;
    if crc32_ieee(&bytes[..CRC_OFFSET]) != get_u32(bytes, CRC_OFFSET) {
        return Err(SummaryError::Checksum);
    }
    Ok(RunSummary {
        campaign_crc32: get_u32(bytes, 12),
        scenario_id: get_u32(bytes, 16),
        run_index: get_u32(bytes, 20),
        sensor_seed: get_u32(bytes, 24),
        variation_checksum: get_u32(bytes, 28),
        outcome,
        active_stage: bytes[33],
        flight_mode: bytes[34],
        terminal_step: get_u32(bytes, 36),
        terminal_radius_q12: get_i32(bytes, 40),
        terminal_downrange_q32: get_i32(bytes, 44),
        terminal_radial_velocity_q24: get_i32(bytes, 48),
        terminal_angular_momentum_q14: get_i32(bytes, 52),
        terminal_mass_q12: get_i32(bytes, 56),
        cutoff_step: get_u32(bytes, 60),
        cutoff_radius_q12: get_i32(bytes, 64),
        cutoff_downrange_q32: get_i32(bytes, 68),
        cutoff_radial_velocity_q24: get_i32(bytes, 72),
        cutoff_angular_momentum_q14: get_i32(bytes, 76),
        max_dynamic_pressure_q16: get_i32(bytes, 80),
        max_proper_acceleration_q28: get_i32(bytes, 84),
        navigation_position_error_q12: get_i32(bytes, 88),
        navigation_velocity_error_q24: get_i32(bytes, 92),
        truth_checksum: get_u32(bytes, 96),
        sensor_checksum: get_u32(bytes, 100),
        navigation_checksum: get_u32(bytes, 104),
        flight_checksum: get_u32(bytes, 108),
        alarms: get_u16(bytes, 112),
    })
}
