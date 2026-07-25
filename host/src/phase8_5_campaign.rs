//! Deterministic ordered Phase 8.5 avionics campaign.
use crate::phase8_5::checked_in_reference;
use ksa64_core::phase8_5_contract::{write_avionics_summary, KAS8_LENGTH};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::keyed_word_raw;
use ksa64_sim::phase8_5::{
    evaluate_with_avionics, AvionicsEvaluationRequest, LocalAvionicsVariation,
};
use ksa64_sim::phase8_campaign::{
    derive_spatial_uncertainty, materialize_spatial_case, SpatialCampaignConfig,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

pub const PHASE85_CAMPAIGN_RUNS: u32 = 64;
pub const PHASE85_CAMPAIGN_SEED: u32 = 0x4b53_4185;
pub const PHASE85_ARCHIVE_RECORD_LENGTH: usize = 8 + KAS8_LENGTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase85CampaignAggregate {
    pub runs: u32,
    pub completed: u32,
    pub recovery_incomplete: u32,
    pub model_envelope_exceeded: u32,
    pub alarmed: u32,
    pub saturated: u32,
    pub maximum_navigation_error_q13: i32,
    pub maximum_attitude_error_turn16: i16,
    pub records_crc32: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phase85CampaignResult {
    pub records: Vec<[u8; KAS8_LENGTH]>,
    pub variation_checksums: Vec<u32>,
    pub aggregate: Phase85CampaignAggregate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase85CampaignError {
    Configuration,
    Evaluation,
    Encoding,
}

fn uniform_i32(word: u32, minimum: i32, maximum: i32) -> i32 {
    let span = (i64::from(maximum) - i64::from(minimum) + 1) as u64;
    minimum + (((u64::from(word) * span) >> 32) as i32)
}
fn draw(run: u32, parameter: u8, minimum: i32, maximum: i32) -> i32 {
    uniform_i32(
        keyed_word_raw(PHASE85_CAMPAIGN_SEED, run, parameter, 0, 0),
        minimum,
        maximum,
    )
}
fn derive_avionics_case(run: u32) -> (LocalAvionicsVariation, [i32; 4], u32) {
    if run == 0 {
        return (LocalAvionicsVariation::NOMINAL, [0; 4], 0);
    }
    let mut value = LocalAvionicsVariation::NOMINAL;
    value.seed = keyed_word_raw(PHASE85_CAMPAIGN_SEED, run, 16, 0, 0);
    for axis in 0..3 {
        value.imu_delta_bias[axis] = draw(run, 17 + axis as u8, -12, 12) as i16;
        value.gyro_bias[axis] = draw(run, 20 + axis as u8, -8, 8) as i16;
        value.gps_position_bias_q13[axis] = draw(run, 24 + axis as u8, -50 << 13, 50 << 13);
        value.gps_velocity_bias_q19[axis] = draw(run, 27 + axis as u8, -262_144, 262_144);
    }
    value.barometer_bias_q13 = draw(run, 23, -25 << 13, 25 << 13);
    value.sensor_noise_scale = draw(run, 30, 0, 512) as u16;
    value.aid_delay_epochs = draw(run, 31, 0, 2) as u8;
    value.clock_drift_ppm = draw(run, 32, -250, 250) as i16;
    value.deployment_actuation_delay_epochs = draw(run, 33, 0, 4) as u8;
    let dropout = keyed_word_raw(PHASE85_CAMPAIGN_SEED, run, 34, 0, 0);
    if dropout & 7 == 0 {
        value.barometer_dropout_start = 64 + (dropout as u16 % 320);
        value.barometer_dropout_epochs = 4 + ((dropout >> 16) as u8 & 7);
    }
    if dropout & 0x38 == 0 {
        value.gps_dropout_start = 32 + ((dropout >> 8) as u16 % 256);
        value.gps_dropout_epochs = 8 + ((dropout >> 24) as u8 & 15);
    }
    if dropout & 0x1c0 == 0 {
        value.link_dropout_start = 48 + ((dropout >> 12) as u16 % 256);
        value.link_dropout_epochs = 1 + ((dropout >> 28) as u8 & 1);
    }
    let actuator = [
        draw(run, 35, -1, 1),
        draw(run, 36, 900_000, 1_100_000),
        draw(run, 37, 900_000, 1_100_000),
        draw(run, 38, 900_000, 1_100_000),
    ];
    let mut bytes = [0u8; 96];
    bytes[0..4].copy_from_slice(&value.seed.to_le_bytes());
    let mut at = 4;
    for raw in value.imu_delta_bias.into_iter().chain(value.gyro_bias) {
        bytes[at..at + 2].copy_from_slice(&raw.to_le_bytes());
        at += 2;
    }
    for raw in value
        .gps_position_bias_q13
        .into_iter()
        .chain(value.gps_velocity_bias_q19)
        .chain([value.barometer_bias_q13])
        .chain(actuator)
    {
        bytes[at..at + 4].copy_from_slice(&raw.to_le_bytes());
        at += 4;
    }
    bytes[at..at + 2].copy_from_slice(&value.sensor_noise_scale.to_le_bytes());
    at += 2;
    bytes[at] = value.aid_delay_epochs;
    bytes[at + 1] = value.deployment_actuation_delay_epochs;
    bytes[at + 2..at + 4].copy_from_slice(&value.clock_drift_ppm.to_le_bytes());
    at += 4;
    bytes[at..at + 2].copy_from_slice(&value.barometer_dropout_start.to_le_bytes());
    bytes[at + 2] = value.barometer_dropout_epochs;
    bytes[at + 3] = value.gps_dropout_epochs;
    bytes[at + 4..at + 6].copy_from_slice(&value.gps_dropout_start.to_le_bytes());
    bytes[at + 6..at + 8].copy_from_slice(&value.link_dropout_start.to_le_bytes());
    bytes[at + 8] = value.link_dropout_epochs;
    (value, actuator, crc32_ieee(&bytes))
}
fn scale(value: i32, ppm: i32) -> i32 {
    ((i64::from(value) * i64::from(ppm)) / 1_000_000)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
fn evaluate_run(index: u32) -> Result<([u8; KAS8_LENGTH], u32), Phase85CampaignError> {
    let mut reference =
        checked_in_reference(true).map_err(|_| Phase85CampaignError::Configuration)?;
    let spatial_config = SpatialCampaignConfig {
        master_seed: PHASE85_CAMPAIGN_SEED,
        run_count: PHASE85_CAMPAIGN_RUNS,
    };
    let spatial = derive_spatial_uncertainty(spatial_config, index);
    let (mission, wind, physical) =
        materialize_spatial_case(reference.mission, &reference.wind, spatial, index);
    let (avionics, actuator, avionics_checksum) = derive_avionics_case(index);
    if index != 0 {
        reference.capability.lag_releases =
            (i32::from(reference.capability.lag_releases) + actuator[0]).clamp(0, 8) as u8;
        reference.capability.slew_q16_deg_per_s =
            scale(reference.capability.slew_q16_deg_per_s, actuator[1]);
        reference.capability.proportional_gain_q15 =
            scale(reference.capability.proportional_gain_q15, actuator[2]);
        reference.capability.derivative_gain_q15 =
            scale(reference.capability.derivative_gain_q15, actuator[2]);
        reference.capability.gimbal_limit_q16_deg =
            scale(reference.capability.gimbal_limit_q16_deg, actuator[3]);
    }
    let variation_checksum = spatial.checksum.rotate_left(7) ^ avionics_checksum;
    let summary = evaluate_with_avionics(AvionicsEvaluationRequest {
        vehicle: &reference.vehicle,
        motor: &reference.motor,
        mission,
        wind: &wind,
        variation: physical,
        variation_checksum,
        avionics: reference.avionics,
        capability: reference.capability,
        uncertainty_case: avionics,
    })
    .map_err(|_| Phase85CampaignError::Evaluation)?;
    let mut record = [0u8; KAS8_LENGTH];
    write_avionics_summary(summary, &mut record).map_err(|_| Phase85CampaignError::Encoding)?;
    Ok((record, variation_checksum))
}
fn aggregate(records: &[[u8; KAS8_LENGTH]]) -> Phase85CampaignAggregate {
    use ksa64_core::evaluation::EvaluationOutcome;
    use ksa64_core::phase8_5_contract::parse_avionics_summary;
    let mut aggregate = Phase85CampaignAggregate {
        runs: records.len() as u32,
        completed: 0,
        recovery_incomplete: 0,
        model_envelope_exceeded: 0,
        alarmed: 0,
        saturated: 0,
        maximum_navigation_error_q13: 0,
        maximum_attitude_error_turn16: 0,
        records_crc32: 0,
    };
    let mut bytes = Vec::with_capacity(records.len() * KAS8_LENGTH);
    for record in records {
        let parsed = parse_avionics_summary(record).expect("campaign KAS8");
        aggregate.completed += u32::from(matches!(
            parsed.outcome,
            EvaluationOutcome::Complete | EvaluationOutcome::GroundContact
        ));
        aggregate.recovery_incomplete +=
            u32::from(parsed.outcome == EvaluationOutcome::RecoveryIncomplete);
        aggregate.model_envelope_exceeded +=
            u32::from(parsed.outcome == EvaluationOutcome::ModelEnvelopeExceeded);
        aggregate.alarmed += u32::from(parsed.alarms != 0);
        aggregate.saturated += u32::from(parsed.saturation_count != 0);
        aggregate.maximum_navigation_error_q13 = aggregate
            .maximum_navigation_error_q13
            .max(parsed.max_navigation_error_q13);
        aggregate.maximum_attitude_error_turn16 = aggregate
            .maximum_attitude_error_turn16
            .max(parsed.max_attitude_error_turn16);
        bytes.extend_from_slice(record);
    }
    aggregate.records_crc32 = crc32_ieee(&bytes);
    aggregate
}
pub fn run_phase85_campaign(workers: usize) -> Result<Phase85CampaignResult, Phase85CampaignError> {
    let workers = workers.max(1).min(PHASE85_CAMPAIGN_RUNS as usize);
    let next = AtomicU32::new(0);
    let slots = Mutex::new(vec![None; PHASE85_CAMPAIGN_RUNS as usize]);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= PHASE85_CAMPAIGN_RUNS {
                    break;
                }
                slots.lock().unwrap()[index as usize] = Some(evaluate_run(index));
            });
        }
    });
    let mut records = Vec::with_capacity(PHASE85_CAMPAIGN_RUNS as usize);
    let mut variation_checksums = Vec::with_capacity(PHASE85_CAMPAIGN_RUNS as usize);
    for slot in slots.into_inner().unwrap() {
        let (record, variation) = slot.expect("ordered campaign slot")?;
        records.push(record);
        variation_checksums.push(variation);
    }
    let aggregate = aggregate(&records);
    Ok(Phase85CampaignResult {
        records,
        variation_checksums,
        aggregate,
    })
}
pub fn encode_phase85_campaign(result: &Phase85CampaignResult) -> Vec<u8> {
    let mut output = vec![0u8; result.records.len() * PHASE85_ARCHIVE_RECORD_LENGTH];
    for (index, (record, variation)) in result
        .records
        .iter()
        .zip(&result.variation_checksums)
        .enumerate()
    {
        let offset = index * PHASE85_ARCHIVE_RECORD_LENGTH;
        output[offset..offset + 4].copy_from_slice(&(index as u32).to_le_bytes());
        output[offset + 4..offset + 8].copy_from_slice(&variation.to_le_bytes());
        output[offset + 8..offset + PHASE85_ARCHIVE_RECORD_LENGTH].copy_from_slice(record);
    }
    output
}
pub fn validate_phase85_campaign(input: &[u8]) -> bool {
    use ksa64_core::phase8_5_contract::parse_avionics_summary;
    if input.len() != PHASE85_CAMPAIGN_RUNS as usize * PHASE85_ARCHIVE_RECORD_LENGTH {
        return false;
    }
    for index in 0..PHASE85_CAMPAIGN_RUNS as usize {
        let offset = index * PHASE85_ARCHIVE_RECORD_LENGTH;
        if u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap()) != index as u32
            || parse_avionics_summary(&input[offset + 8..offset + PHASE85_ARCHIVE_RECORD_LENGTH])
                .is_err()
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_zero_is_nominal_and_worker_counts_are_exact() {
        let serial = run_phase85_campaign(1).unwrap();
        let parallel = run_phase85_campaign(4).unwrap();
        assert_eq!(serial, parallel);
        assert_eq!(serial.variation_checksums[0], 0);
        let bytes = encode_phase85_campaign(&serial);
        assert!(validate_phase85_campaign(&bytes));
        let mut corrupt = bytes.clone();
        corrupt[8 + 80] ^= 0x40;
        assert!(!validate_phase85_campaign(&corrupt));
    }
}
