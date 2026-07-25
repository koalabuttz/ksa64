//! Ordered Phase 8 host campaigns and strict append-only KRA8 archives.

use ksa64_core::evaluation::{EvaluationOutcome, EvaluationSummary, MetricSlot, ModelProfileId};
use ksa64_core::phase8_format::{
    seal_phase8_record, validate_phase8_record, write_phase8_header, Phase8RecordKind,
    KRA8_HEADER_LENGTH, KSR8_LENGTH,
};
use ksa64_core::phase8_mission::Phase8MissionError;
use ksa64_core::phase8_numeric::{HOBBY_SPATIAL_ENVIRONMENT_ID, HOBBY_SPATIAL_NUMERIC_CONTRACT_ID};
use ksa64_core::phase8_pack::{
    SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindProfilePack,
};
use ksa64_core::phase8_result::{encode_ksr8, parse_ksr8};
use ksa64_interface::crc32_ieee;
use ksa64_sim::evaluation::{evaluate, EvaluationError, EvaluationRequest};
use ksa64_sim::phase8_campaign::{
    derive_spatial_uncertainty, materialize_spatial_case, SpatialCampaignConfig,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

const ARCHIVE_RECORD_LENGTH: usize = 8 + KSR8_LENGTH;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialCampaignAggregate {
    pub runs: u32,
    pub ground_contacts: u32,
    pub model_envelope_exceeded: u32,
    pub minimum_apogee_raw: i32,
    pub maximum_apogee_raw: i32,
    pub mean_apogee_raw: i32,
    pub maximum_landing_distance_raw: i32,
    pub maximum_dynamic_pressure_raw: i32,
    pub records_crc32: u32,
}
pub struct SpatialCampaignResult {
    pub config: SpatialCampaignConfig,
    pub records: Vec<[u8; KSR8_LENGTH]>,
    pub aggregate: SpatialCampaignAggregate,
}
fn evaluate_run(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
    config: SpatialCampaignConfig,
    index: u32,
) -> [u8; KSR8_LENGTH] {
    let uncertainty = derive_spatial_uncertainty(config, index);
    let (mission, wind, variation) = materialize_spatial_case(mission, wind, uncertainty, index);
    let summary = match evaluate(EvaluationRequest::HobbySpatialV1 {
        vehicle,
        motor,
        mission,
        wind: &wind,
        variation,
        variation_checksum: uncertainty.checksum,
    }) {
        Ok(summary) => summary,
        Err(EvaluationError::HobbySpatial(error)) => {
            let mut summary = EvaluationSummary::empty(ModelProfileId::HobbySpatialV1);
            summary.outcome = match error {
                Phase8MissionError::Configuration => EvaluationOutcome::ConfigurationFault,
                Phase8MissionError::ModelEnvelopeExceeded => {
                    EvaluationOutcome::ModelEnvelopeExceeded
                }
                Phase8MissionError::Numeric | Phase8MissionError::Complete => {
                    EvaluationOutcome::NumericFault
                }
            };
            summary.numeric_faults = u8::from(matches!(error, Phase8MissionError::Numeric));
            summary.identities = [
                HOBBY_SPATIAL_NUMERIC_CONTRACT_ID,
                HOBBY_SPATIAL_ENVIRONMENT_ID,
                vehicle.identity,
                motor.identity,
                mission.identity,
                wind.identity,
            ];
            summary.source_checksums = [0, mission.case_seed, uncertainty.checksum, 0, 0];
            summary
        }
        Err(_) => unreachable!("spatial request returned non-spatial error"),
    };
    let mut record = [0u8; KSR8_LENGTH];
    encode_ksr8(summary, &mut record).expect("valid spatial summary");
    record
}
fn aggregate(records: &[[u8; KSR8_LENGTH]]) -> SpatialCampaignAggregate {
    let mut ground = 0;
    let mut envelope = 0;
    let mut min_apogee = i32::MAX;
    let mut max_apogee = i32::MIN;
    let mut apogee_sum = 0i128;
    let mut max_landing = 0;
    let mut max_q = 0;
    let mut bytes = Vec::with_capacity(records.len() * KSR8_LENGTH);
    for record in records {
        let summary = parse_ksr8(record).expect("campaign KSR8").summary;
        ground += u32::from(summary.outcome == EvaluationOutcome::GroundContact);
        envelope += u32::from(summary.outcome == EvaluationOutcome::ModelEnvelopeExceeded);
        let apogee = summary.metric(MetricSlot::ApogeeAltitude).unwrap_or(0);
        min_apogee = min_apogee.min(apogee);
        max_apogee = max_apogee.max(apogee);
        apogee_sum += apogee as i128;
        max_landing = max_landing.max(summary.metric(MetricSlot::LandingDistance).unwrap_or(0));
        max_q = max_q.max(summary.metric(MetricSlot::MaxDynamicPressure).unwrap_or(0));
        bytes.extend_from_slice(record)
    }
    SpatialCampaignAggregate {
        runs: records.len() as u32,
        ground_contacts: ground,
        model_envelope_exceeded: envelope,
        minimum_apogee_raw: min_apogee,
        maximum_apogee_raw: max_apogee,
        mean_apogee_raw: (apogee_sum / records.len() as i128) as i32,
        maximum_landing_distance_raw: max_landing,
        maximum_dynamic_pressure_raw: max_q,
        records_crc32: crc32_ieee(&bytes),
    }
}
pub fn run_spatial_campaign(
    vehicle: SpatialVehiclePack,
    motor: SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: WindProfilePack,
    config: SpatialCampaignConfig,
    workers: usize,
) -> SpatialCampaignResult {
    assert!(config.is_valid());
    let worker_count = workers.max(1).min(config.run_count as usize);
    let next = AtomicU32::new(0);
    let slots = Mutex::new(vec![None; config.run_count as usize]);
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= config.run_count {
                    break;
                }
                let record = evaluate_run(&vehicle, &motor, mission, &wind, config, index);
                slots.lock().unwrap()[index as usize] = Some(record);
            });
        }
    });
    let records = slots
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|record| record.expect("ordered slot"))
        .collect::<Vec<_>>();
    let aggregate = aggregate(&records);
    SpatialCampaignResult {
        config,
        records,
        aggregate,
    }
}
fn wu32(o: &mut [u8], p: usize, v: u32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn ru32(i: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(i[p..p + 4].try_into().unwrap())
}
pub fn encode_kra8(campaign: &SpatialCampaignResult) -> Vec<u8> {
    let mut output = vec![0u8; KRA8_HEADER_LENGTH + campaign.records.len() * ARCHIVE_RECORD_LENGTH];
    write_phase8_header(
        &mut output[..KRA8_HEADER_LENGTH],
        Phase8RecordKind::ArchiveHeader,
        campaign.aggregate.records_crc32,
    )
    .unwrap();
    wu32(&mut output, 32, campaign.config.master_seed);
    wu32(&mut output, 36, campaign.config.run_count);
    wu32(&mut output, 40, ARCHIVE_RECORD_LENGTH as u32);
    wu32(&mut output, 44, campaign.aggregate.records_crc32);
    wu32(&mut output, 48, campaign.aggregate.ground_contacts);
    wu32(&mut output, 52, campaign.aggregate.model_envelope_exceeded);
    seal_phase8_record(&mut output[..KRA8_HEADER_LENGTH]).unwrap();
    for (index, record) in campaign.records.iter().enumerate() {
        let offset = KRA8_HEADER_LENGTH + index * ARCHIVE_RECORD_LENGTH;
        wu32(&mut output, offset, index as u32);
        wu32(&mut output, offset + 4, KSR8_LENGTH as u32);
        output[offset + 8..offset + 8 + KSR8_LENGTH].copy_from_slice(record)
    }
    output
}
pub fn validate_kra8(input: &[u8]) -> bool {
    if input.len() < KRA8_HEADER_LENGTH
        || validate_phase8_record(
            &input[..KRA8_HEADER_LENGTH],
            Phase8RecordKind::ArchiveHeader,
        )
        .is_err()
    {
        return false;
    }
    let count = ru32(input, 36) as usize;
    if ru32(input, 40) as usize != ARCHIVE_RECORD_LENGTH
        || input.len() != KRA8_HEADER_LENGTH + count * ARCHIVE_RECORD_LENGTH
    {
        return false;
    }
    let mut bytes = Vec::with_capacity(count * KSR8_LENGTH);
    for index in 0..count {
        let offset = KRA8_HEADER_LENGTH + index * ARCHIVE_RECORD_LENGTH;
        if ru32(input, offset) as usize != index || ru32(input, offset + 4) as usize != KSR8_LENGTH
        {
            return false;
        }
        let record = &input[offset + 8..offset + 8 + KSR8_LENGTH];
        if parse_ksr8(record).is_err() {
            return false;
        }
        bytes.extend_from_slice(record)
    }
    crc32_ieee(&bytes) == ru32(input, 44)
}
pub fn summary_for_record(record: &[u8; KSR8_LENGTH]) -> EvaluationSummary {
    parse_ksr8(record).expect("validated KSR8").summary
}
#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase8_pack::{
        parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
        parse_wind_profile_pack,
    };
    #[test]
    fn worker_counts_and_corruption_are_exact() {
        let v =
            parse_spatial_vehicle_pack(include_bytes!("../../phase8/examples/firestorm54.kvp8"))
                .unwrap();
        let m =
            parse_spatial_motor_pack(include_bytes!("../../phase8/examples/aerotech-i211w.kmp8"))
                .unwrap();
        let c =
            parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
                .unwrap();
        let w =
            parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
                .unwrap();
        let config = SpatialCampaignConfig {
            master_seed: 0x4b53_4138,
            run_count: 8,
        };
        let a = encode_kra8(&run_spatial_campaign(v, m, c, w, config, 1));
        let b = encode_kra8(&run_spatial_campaign(v, m, c, w, config, 4));
        assert_eq!(a, b);
        assert!(validate_kra8(&a));
        let mut bad = a;
        bad[100] ^= 1;
        assert!(!validate_kra8(&bad));
    }
}
