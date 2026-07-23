use crate::phase5_campaign::{
    derive_phase5_run, parse_ksc5, parse_ksr5, reference_phase5_campaign_config, write_ksc5,
    write_ksr5, Phase5RunSummary, KSC5_LENGTH, KSR5_LENGTH,
};
use crate::phase5_mission::{Phase5MissionCase, Phase5MissionOutcome, Phase5MissionSummary};
use ksa64_interface::crc32_ieee;
pub const PHASE5_CAMPAIGN_PROBE_SIGNATURE: u32 = 0xc921_a2d2;
pub fn phase5_campaign_probe_signature() -> u32 {
    let c = reference_phase5_campaign_config();
    let mut cb = [0u8; KSC5_LENGTH];
    if write_ksc5(&c, &mut cb).is_err() || parse_ksc5(&cb) != Ok(c) {
        return 0;
    }
    let zero = match derive_phase5_run(&c, 0) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let sample = match derive_phase5_run(&c, 17) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    if zero.sensor_seed != 0x5a00_0000 || zero.variation.values().iter().any(|&v| v != 0) {
        return 0;
    }
    let summary = Phase5RunSummary {
        campaign_seed: c.master_seed,
        run_index: 0,
        sensor_seed: zero.sensor_seed,
        variation_checksum: zero.variation.checksum(),
        mission: Phase5MissionSummary {
            case: Phase5MissionCase::Nominal,
            outcome: Phase5MissionOutcome::StableOrbit,
            steps: 3133,
            terminal_position_q12: [21468577, 3871182, 15698368],
            terminal_velocity_q24: [-66327286, 89767125, 68337641],
            perigee_altitude_q12: 0,
            apogee_altitude_q12: 0,
            inclination_turn16: 0,
            max_dynamic_pressure_q16: 2861000,
            max_aoa_sine_q16: 13229,
            max_flexible_state_q24: 52314,
            max_nav_position_error_q12: 2781,
            events: 7,
            sensor_checksum: 1741708362,
            navigation_checksum: 2996014246,
            flight_checksum: 4068248986,
            summary_checksum: 557491580,
        },
    };
    let mut sb = [0u8; KSR5_LENGTH];
    if write_ksr5(&summary, &mut sb).is_err() || parse_ksr5(&sb) != Ok(summary) {
        return 0;
    }
    crc32_ieee(&cb) ^ sample.variation.checksum().rotate_left(7) ^ crc32_ieee(&sb).rotate_left(13)
}
pub fn run_phase5_campaign_self_tests() -> u8 {
    u8::from(phase5_campaign_probe_signature() != PHASE5_CAMPAIGN_PROBE_SIGNATURE)
}
