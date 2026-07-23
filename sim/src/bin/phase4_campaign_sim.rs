#![no_std]
#![no_main]

use core::panic::PanicInfo;

use ksa64_sim::phase4::aggregate::StreamingMetric;
use ksa64_sim::phase4::campaign::{derive_run, reviewed_campaign_config};
use ksa64_sim::phase4::contracts::RUN_SUMMARY_LENGTH;
use ksa64_sim::phase4::mission::mission_parameters;
use ksa64_sim::phase4::summary::{parse_ksr4, write_ksr4, RunOutcome, RunSummary};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> isize {
    let config = reviewed_campaign_config(64);
    if config.validate().is_err() {
        return 1;
    }
    let run = match derive_run(&config, 0) {
        Ok(run) => run,
        Err(_) => return 2,
    };
    let parameters = match mission_parameters(run) {
        Some(parameters) => parameters,
        None => return 3,
    };
    if parameters.sensor_seed != 0x4b53_4133 || parameters.world.payload_mass_ppm != 0 {
        return 4;
    }
    let mut metric = StreamingMetric::EMPTY;
    metric.update(1);
    metric.update(2);
    metric.update(3);
    if metric.mean() != 2 || metric.sample_variance() != 1 {
        return 5;
    }
    let summary = RunSummary {
        campaign_crc32: 1,
        scenario_id: 2,
        run_index: 3,
        sensor_seed: 4,
        variation_checksum: 5,
        outcome: RunOutcome::StableOrbit,
        terminal_step: 6,
        terminal_radius_q12: 7,
        terminal_downrange_q32: 8,
        terminal_radial_velocity_q24: 9,
        terminal_angular_momentum_q14: 10,
        terminal_mass_q12: 11,
        cutoff_step: 12,
        cutoff_radius_q12: 13,
        cutoff_downrange_q32: 14,
        cutoff_radial_velocity_q24: 15,
        cutoff_angular_momentum_q14: 16,
        max_dynamic_pressure_q16: 17,
        max_proper_acceleration_q28: 18,
        navigation_position_error_q12: 19,
        navigation_velocity_error_q24: 20,
        truth_checksum: 21,
        sensor_checksum: 22,
        navigation_checksum: 23,
        flight_checksum: 24,
        alarms: 25,
        flight_mode: 5,
        active_stage: 1,
    };
    let mut bytes = [0u8; RUN_SUMMARY_LENGTH];
    if write_ksr4(&summary, &mut bytes).is_err() || parse_ksr4(&bytes) != Ok(summary) {
        return 6;
    }
    0
}
