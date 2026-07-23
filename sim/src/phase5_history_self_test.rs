use crate::phase4::storage::ReuPreference;
use crate::phase5_history::{
    parse_kph5_point, validate_kph5, write_kph5, Phase5HistoryHeader, Phase5HistoryPoint,
    Phase5StoragePlan, KPH5_HEADER_LENGTH, KPH5_POINT_LENGTH,
};
use ksa64_interface::crc32_ieee;

pub const PHASE5_HISTORY_PROBE_SIGNATURE: u32 = 0xb578_3bf2;

pub fn phase5_history_probe_signature() -> u32 {
    let points = [
        Phase5HistoryPoint {
            step: 0,
            position_quarter_km: [25510, 0, 13850],
            dynamic_pressure_sixteenth_kpa: 0,
            navigation_error_quarter_km: 0,
            events: 0,
            alarms: 0,
        },
        Phase5HistoryPoint {
            step: 32,
            position_quarter_km: [25511, 3, 13851],
            dynamic_pressure_sixteenth_kpa: 217,
            navigation_error_quarter_km: 1,
            events: 1,
            alarms: 0,
        },
        Phase5HistoryPoint {
            step: 3133,
            position_quarter_km: [20965, 3780, 15330],
            dynamic_pressure_sixteenth_kpa: 0,
            navigation_error_quarter_km: 2,
            events: 7,
            alarms: 4,
        },
    ];
    let header = Phase5HistoryHeader {
        campaign_seed: 0x4b53_4135,
        run_index: 0,
        sensor_seed: 0x5a00_0000,
        variation_checksum: 0,
        stride: 32,
        point_count: 0,
        terminal_step: 3133,
        points_crc32: 0,
    };
    let mut bytes = [0u8; KPH5_HEADER_LENGTH + 3 * KPH5_POINT_LENGTH];
    if write_kph5(header, &points, &mut bytes).is_err() {
        return 0;
    }
    let parsed = match validate_kph5(&bytes) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    if parse_kph5_point(&bytes[KPH5_HEADER_LENGTH..KPH5_HEADER_LENGTH + KPH5_POINT_LENGTH])
        != Ok(points[0])
    {
        return 0;
    }
    let stock = match Phase5StoragePlan::compute(0, ReuPreference::Auto, 256, 3134, 99) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let reu = match Phase5StoragePlan::compute(2048, ReuPreference::Auto, 256, 3134, 393) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    crc32_ieee(&bytes)
        ^ parsed.points_crc32.rotate_left(5)
        ^ stock.used_bytes.rotate_left(11)
        ^ reu.used_bytes.rotate_left(17)
        ^ reu.full_histories
}

pub fn run_phase5_history_self_tests() -> u8 {
    u8::from(phase5_history_probe_signature() != PHASE5_HISTORY_PROBE_SIGNATURE)
}
