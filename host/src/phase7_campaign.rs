//! Ordered host campaigns, candidate manifests, and append-only KRA7 archives.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use ksa64_core::evaluation::{EvaluationOutcome, EvaluationSummary, MetricSlot};
use ksa64_core::phase7_format::{
    seal_phase7_record, validate_phase7_record, write_phase7_header, Phase7RecordKind,
    KCL7_HEADER_LENGTH, KRA7_HEADER_LENGTH, KSR7_LENGTH,
};
use ksa64_core::phase7_pack::{HobbyMissionPack, MotorPack, VerticalVehiclePack};
use ksa64_core::phase7_result::{encode_ksr7, parse_ksr7};
use ksa64_interface::crc32_ieee;
use ksa64_sim::evaluation::{evaluate, EvaluationRequest};
use ksa64_sim::phase7_campaign::{
    derive_hobby_uncertainty, materialize_design, materialize_uncertainty, HobbyCampaignConfig,
    HobbyDesignVector,
};

const CANDIDATE_RECORD_LENGTH: usize = 20;
const ARCHIVE_RECORD_LENGTH: usize = 8 + KSR7_LENGTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyCampaignAggregate {
    pub runs: u32,
    pub successful_recoveries: u32,
    pub minimum_apogee_raw: i32,
    pub maximum_apogee_raw: i32,
    pub mean_apogee_raw: i32,
    pub minimum_impact_velocity_raw: i32,
    pub maximum_impact_velocity_raw: i32,
    pub records_crc32: u32,
}

pub struct HobbyCampaignResult {
    pub config: HobbyCampaignConfig,
    pub records: Vec<[u8; KSR7_LENGTH]>,
    pub aggregate: HobbyCampaignAggregate,
}

fn evaluate_run(
    vehicle: VerticalVehiclePack,
    motor: MotorPack,
    mission: HobbyMissionPack,
    config: HobbyCampaignConfig,
    index: u32,
) -> [u8; KSR7_LENGTH] {
    let variation = derive_hobby_uncertainty(config, index);
    let (vehicle, motor, mission) = materialize_uncertainty(vehicle, motor, mission, variation);
    let mut summary = evaluate(EvaluationRequest::HobbyVerticalV1 {
        vehicle,
        motor: &motor,
        mission,
    })
    .expect("validated materialized campaign case");
    summary.identities[5] = variation.checksum;
    let mut record = [0u8; KSR7_LENGTH];
    encode_ksr7(summary, &mut record).expect("validated campaign summary");
    record
}

fn aggregate(records: &[[u8; KSR7_LENGTH]]) -> HobbyCampaignAggregate {
    let mut success = 0u32;
    let mut minimum_apogee = i32::MAX;
    let mut maximum_apogee = i32::MIN;
    let mut apogee_sum = 0i128;
    let mut minimum_impact = i32::MAX;
    let mut maximum_impact = i32::MIN;
    let mut bytes = Vec::with_capacity(records.len() * KSR7_LENGTH);
    for record in records {
        let summary = parse_ksr7(record)
            .expect("campaign created valid KSR7")
            .summary;
        if summary.outcome == EvaluationOutcome::GroundContact {
            success += 1;
        }
        let apogee = summary.metric(MetricSlot::ApogeeAltitude).unwrap_or(0);
        let impact = summary.metric(MetricSlot::ImpactVelocity).unwrap_or(0);
        minimum_apogee = minimum_apogee.min(apogee);
        maximum_apogee = maximum_apogee.max(apogee);
        apogee_sum += apogee as i128;
        minimum_impact = minimum_impact.min(impact);
        maximum_impact = maximum_impact.max(impact);
        bytes.extend_from_slice(record);
    }
    HobbyCampaignAggregate {
        runs: records.len() as u32,
        successful_recoveries: success,
        minimum_apogee_raw: minimum_apogee,
        maximum_apogee_raw: maximum_apogee,
        mean_apogee_raw: (apogee_sum / records.len() as i128) as i32,
        minimum_impact_velocity_raw: minimum_impact,
        maximum_impact_velocity_raw: maximum_impact,
        records_crc32: crc32_ieee(&bytes),
    }
}

pub fn run_hobby_campaign(
    vehicle: VerticalVehiclePack,
    motor: MotorPack,
    mission: HobbyMissionPack,
    design: HobbyDesignVector,
    config: HobbyCampaignConfig,
    workers: usize,
) -> HobbyCampaignResult {
    assert!(config.is_valid());
    let (vehicle, mission) = materialize_design(vehicle, mission, design);
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
                let record = evaluate_run(vehicle, motor, mission, config, index);
                slots.lock().unwrap()[index as usize] = Some(record);
            });
        }
    });
    let records: Vec<_> = slots
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|record| record.expect("worker filled ordered slot"))
        .collect();
    let aggregate = aggregate(&records);
    HobbyCampaignResult {
        config,
        records,
        aggregate,
    }
}

fn w16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn w32(output: &mut [u8], offset: usize, value: i32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn wu32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn ru32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}

pub fn encode_candidate_list(
    base_vehicle_identity: u32,
    base_mission_identity: u32,
    candidates: &[HobbyDesignVector],
) -> Vec<u8> {
    assert!(candidates.len() <= u16::MAX as usize);
    let length = KCL7_HEADER_LENGTH + candidates.len() * CANDIDATE_RECORD_LENGTH + 4;
    assert!(length <= u16::MAX as usize);
    let identity = base_vehicle_identity ^ base_mission_identity.rotate_left(13);
    let mut output = vec![0u8; length];
    write_phase7_header(&mut output, Phase7RecordKind::CandidateList, identity).unwrap();
    w16(&mut output, 32, candidates.len() as u16);
    w16(&mut output, 34, CANDIDATE_RECORD_LENGTH as u16);
    wu32(&mut output, 36, base_vehicle_identity);
    wu32(&mut output, 40, base_mission_identity);
    for (index, candidate) in candidates.iter().enumerate() {
        let offset = KCL7_HEADER_LENGTH + index * CANDIDATE_RECORD_LENGTH;
        wu32(&mut output, offset, index as u32);
        w32(&mut output, offset + 4, candidate.dry_mass_scale_ppm);
        w32(&mut output, offset + 8, candidate.body_drag_scale_ppm);
        w32(
            &mut output,
            offset + 12,
            candidate.main_deployment_altitude_raw,
        );
        w32(&mut output, offset + 16, candidate.rail_length_raw);
    }
    seal_phase7_record(&mut output).unwrap();
    output
}

pub fn validate_candidate_list(input: &[u8]) -> bool {
    if validate_phase7_record(input, Phase7RecordKind::CandidateList).is_err()
        || input.len() < KCL7_HEADER_LENGTH + 4
    {
        return false;
    }
    let count = u16::from_le_bytes([input[32], input[33]]) as usize;
    let record_length = u16::from_le_bytes([input[34], input[35]]) as usize;
    record_length == CANDIDATE_RECORD_LENGTH
        && input.len() == KCL7_HEADER_LENGTH + count * record_length + 4
        && input[44..KCL7_HEADER_LENGTH]
            .iter()
            .all(|value| *value == 0)
}

pub fn encode_kra7(campaign: &HobbyCampaignResult) -> Vec<u8> {
    let length = KRA7_HEADER_LENGTH + campaign.records.len() * ARCHIVE_RECORD_LENGTH;
    let mut output = vec![0u8; length];
    write_phase7_header(
        &mut output[..KRA7_HEADER_LENGTH],
        Phase7RecordKind::ArchiveHeader,
        campaign.aggregate.records_crc32,
    )
    .unwrap();
    wu32(&mut output, 32, campaign.config.master_seed);
    wu32(&mut output, 36, campaign.config.run_count);
    wu32(&mut output, 40, ARCHIVE_RECORD_LENGTH as u32);
    wu32(&mut output, 44, campaign.aggregate.records_crc32);
    seal_phase7_record(&mut output[..KRA7_HEADER_LENGTH]).unwrap();
    for (index, record) in campaign.records.iter().enumerate() {
        let offset = KRA7_HEADER_LENGTH + index * ARCHIVE_RECORD_LENGTH;
        wu32(&mut output, offset, index as u32);
        wu32(&mut output, offset + 4, KSR7_LENGTH as u32);
        output[offset + 8..offset + 8 + KSR7_LENGTH].copy_from_slice(record);
    }
    output
}

pub fn validate_kra7(input: &[u8]) -> bool {
    if input.len() < KRA7_HEADER_LENGTH
        || validate_phase7_record(
            &input[..KRA7_HEADER_LENGTH],
            Phase7RecordKind::ArchiveHeader,
        )
        .is_err()
    {
        return false;
    }
    let count = ru32(input, 36) as usize;
    if ru32(input, 40) as usize != ARCHIVE_RECORD_LENGTH
        || input.len() != KRA7_HEADER_LENGTH + count * ARCHIVE_RECORD_LENGTH
    {
        return false;
    }
    let mut records = Vec::with_capacity(count * KSR7_LENGTH);
    for index in 0..count {
        let offset = KRA7_HEADER_LENGTH + index * ARCHIVE_RECORD_LENGTH;
        if ru32(input, offset) as usize != index || ru32(input, offset + 4) as usize != KSR7_LENGTH
        {
            return false;
        }
        let record = &input[offset + 8..offset + 8 + KSR7_LENGTH];
        if parse_ksr7(record).is_err() {
            return false;
        }
        records.extend_from_slice(record);
    }
    crc32_ieee(&records) == ru32(input, 44)
}

pub fn summary_for_record(record: &[u8; KSR7_LENGTH]) -> EvaluationSummary {
    parse_ksr7(record)
        .expect("validated campaign record")
        .summary
}
