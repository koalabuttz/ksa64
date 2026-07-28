//! Host-owned Phase 10 campaign, archive, and worker-pool services.

use crc32fast::Hasher;
use ksa64_core::evaluation::{EvaluationOutcome, EvaluationSummary, ModelProfileId};
use ksa64_core::phase10_contract::{GlobalSegment, ReferenceFrameId, PHASE10_CONTRACT_ID};
use ksa64_core::phase10_environment::CompiledAtmospherePack;
use ksa64_core::phase10_telemetry::{
    GlobalCampaignConfig, GlobalEvaluationSummary, KSC10_LENGTH, KSR10_LENGTH,
};
use ksa64_core::phase10_vehicle::{GlobalMissionPack, GlobalVehiclePack};
use ksa64_core::spatial_numeric::FixedVec3;
use ksa64_sim::phase10::{GlobalWorldError, GlobalWorldMachine};
use ksa64_sim::phase10_avionics::{reference_global_flight_config, GlobalSensorFaults};
use ksa64_sim::phase10_evaluation::{
    evaluate_global, GlobalEvaluationRequest, GLOBAL_TIME_POLICY_ID,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

pub const PHASE10_MASTER_SEED: u32 = 0x4b53_41a0;
pub const PHASE10_AVIONICS_IDENTITY: u32 = 0x10fc_0001;
pub const PHASE10_CAMPAIGN_IDENTITY: u32 = 0x10ca_0001;
pub const PHASE10_VARIATION_MASK: u32 = 0x000f_ffff;
pub const KRA10_HEADER_LENGTH: usize = 128;
pub const KRA10_RECORD_HEADER_LENGTH: usize = 16;
pub const KRA10_FOOTER_LENGTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalCampaignError {
    RunCount,
    WorkerCount,
    Evaluation {
        run_index: u32,
        error: GlobalWorldError,
    },
    ArchiveLength,
    ArchiveMagic,
    ArchiveIdentity,
    ArchiveReserved,
    ArchiveChecksum,
    ArchiveRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalVariation {
    pub checksum: u32,
    pub mass_scale_q15: i32,
    pub thrust_scale_q15: i32,
    pub aero_scale_q15: i32,
    pub density_scale_q15: i32,
    pub wind_east_q19_m_s: i32,
    pub wind_north_q19_m_s: i32,
    pub launch_azimuth_delta_q28: i32,
    pub rcs_scale_q15: i32,
    pub recovery_scale_q15: i32,
    pub main_altitude_delta_q12_km: i32,
    pub sensor_faults: GlobalSensorFaults,
}

impl GlobalVariation {
    pub const NOMINAL: Self = Self {
        checksum: 0,
        mass_scale_q15: 1 << 15,
        thrust_scale_q15: 1 << 15,
        aero_scale_q15: 1 << 15,
        density_scale_q15: 1 << 15,
        wind_east_q19_m_s: 0,
        wind_north_q19_m_s: 0,
        launch_azimuth_delta_q28: 0,
        rcs_scale_q15: 1 << 15,
        recovery_scale_q15: 1 << 15,
        main_altitude_delta_q12_km: 0,
        sensor_faults: GlobalSensorFaults::NONE,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalCampaignAggregate {
    pub runs: u32,
    pub ground_contacts: u32,
    pub physical_recoveries: u32,
    pub numeric_frame_time_faults: u32,
    pub model_envelope_exceeded: u32,
    pub minimum_apogee_q12_km: i32,
    pub maximum_apogee_q12_km: i32,
    pub maximum_downrange_q12_km: i32,
    pub maximum_navigation_error_q12_km: i32,
    pub summaries_crc32: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobalCampaignResult {
    pub config: GlobalCampaignConfig,
    pub records: Vec<[u8; KSR10_LENGTH]>,
    pub aggregate: GlobalCampaignAggregate,
}

pub use ksa64_session::global_fixtures::GlobalFixtureSet;

/// Compatibility extension preserving the historical
/// ksa64_host::phase10::* method surface while keeping campaign policy host-owned.
pub trait GlobalFixtureSetCampaignExt {
    fn campaign_config(&self, run_count: u16) -> GlobalCampaignConfig;
}

impl GlobalFixtureSetCampaignExt for GlobalFixtureSet {
    fn campaign_config(&self, run_count: u16) -> GlobalCampaignConfig {
        GlobalCampaignConfig {
            identity: PHASE10_CAMPAIGN_IDENTITY ^ u32::from(run_count),
            earth_identity: self.earth.identity,
            transform_identity: self.transforms.identity,
            atmosphere_identity: self.atmosphere.identity,
            vehicle_identity: self.vehicle.identity,
            mission_identity: self.mission.identity,
            avionics_identity: PHASE10_AVIONICS_IDENTITY,
            master_seed: PHASE10_MASTER_SEED,
            run_count,
            catalog_version: 1,
            variation_mask: PHASE10_VARIATION_MASK,
        }
    }
}

pub fn derive_global_variation(master_seed: u32, run_index: u32) -> GlobalVariation {
    if run_index == 0 {
        return GlobalVariation::NOMINAL;
    }
    let draw = |parameter: u32| keyed_signed(master_seed, run_index, parameter);
    let scale = |parameter: u32, amplitude: i32| {
        (1 << 15) + ((i64::from(draw(parameter)) * i64::from(amplitude)) >> 15) as i32
    };
    let mut faults = GlobalSensorFaults::NONE;
    for axis in 0..3 {
        faults.imu_delta_velocity_bias[axis] = ((draw(20 + axis as u32) * 8) >> 15) as i16;
        faults.imu_delta_angle_bias[axis] = ((draw(24 + axis as u32) * 4) >> 15) as i16;
        faults.gnss_position_bias_q12_km[axis] =
            ((i64::from(draw(28 + axis as u32)) * 410) >> 15) as i32;
        faults.gnss_velocity_bias_q24_km_s[axis] =
            ((i64::from(draw(32 + axis as u32)) * 16_777) >> 15) as i32;
    }
    faults.barometer_bias_q12_km = ((i64::from(draw(36)) * 82) >> 15) as i32;
    faults.clock_drift_ppm = ((draw(37) * 10) >> 15) as i16;
    if run_index.is_multiple_of(61) {
        faults.fast_dropout_start = 1_600;
        faults.fast_dropout_length = 2;
    }
    if run_index.is_multiple_of(47) {
        faults.gnss_dropout_start = 2_000;
        faults.gnss_dropout_length = 32;
    }
    let mut variation = GlobalVariation {
        checksum: 0,
        mass_scale_q15: scale(1, 328),
        thrust_scale_q15: scale(2, 655),
        aero_scale_q15: scale(3, 1_638),
        density_scale_q15: scale(4, 1_638),
        wind_east_q19_m_s: ((i64::from(draw(5)) * (2 << 19)) >> 15) as i32,
        wind_north_q19_m_s: ((i64::from(draw(6)) * (2 << 19)) >> 15) as i32,
        launch_azimuth_delta_q28: ((i64::from(draw(7)) * 2_342_023) >> 15) as i32,
        rcs_scale_q15: scale(8, 3_277),
        recovery_scale_q15: scale(9, 3_277),
        main_altitude_delta_q12_km: ((i64::from(draw(10)) * 82) >> 15) as i32,
        sensor_faults: faults,
    };
    variation.checksum = variation_checksum(variation, master_seed, run_index);
    variation
}

pub fn evaluate_global_campaign_run(
    fixtures: &GlobalFixtureSet,
    config: GlobalCampaignConfig,
    run_index: u32,
) -> Result<[u8; KSR10_LENGTH], GlobalCampaignError> {
    let variation = derive_global_variation(config.master_seed, run_index);
    let (atmosphere, vehicle, mission) = materialize_global_variation(fixtures, variation)?;
    let world = GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &atmosphere,
        &vehicle,
        mission,
    )
    .map_err(|error| GlobalCampaignError::Evaluation { run_index, error })?;
    let avionics = reference_global_flight_config(
        (0x10a0u16).wrapping_add(run_index as u16),
        world
            .active_state()
            .map_err(|error| GlobalCampaignError::Evaluation { run_index, error })?,
        mission,
    )
    .map_err(|error| GlobalCampaignError::Evaluation { run_index, error })?;
    let summary = match evaluate_global(GlobalEvaluationRequest {
        earth: &fixtures.earth,
        transforms: &fixtures.transforms,
        atmosphere: &atmosphere,
        vehicle: &vehicle,
        mission,
        avionics,
        uncertainty: variation.sensor_faults,
        case_seed: config.master_seed ^ run_index.rotate_left(13),
    }) {
        Ok(summary) => summary,
        Err(error) if error.is_model_envelope() => {
            envelope_summary(fixtures, atmosphere, vehicle, variation)
        }
        Err(error) => return Err(GlobalCampaignError::Evaluation { run_index, error }),
    };
    let mut record = [0; KSR10_LENGTH];
    summary
        .encode(&mut record)
        .map_err(|_| GlobalCampaignError::ArchiveRecord)?;
    Ok(record)
}

pub fn run_global_campaign(
    fixtures: &GlobalFixtureSet,
    run_count: u16,
    workers: usize,
) -> Result<GlobalCampaignResult, GlobalCampaignError> {
    if !matches!(run_count, 64 | 256) {
        return Err(GlobalCampaignError::RunCount);
    }
    if workers == 0 {
        return Err(GlobalCampaignError::WorkerCount);
    }
    let config = fixtures.campaign_config(run_count);
    let next = AtomicU32::new(0);
    let slots = Mutex::new(vec![None; run_count as usize]);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(run_count as usize) {
            scope.spawn(|| loop {
                let run_index = next.fetch_add(1, Ordering::Relaxed);
                if run_index >= u32::from(run_count) {
                    break;
                }
                let record = evaluate_global_campaign_run(fixtures, config, run_index);
                slots.lock().expect("campaign slot lock")[run_index as usize] = Some(record);
            });
        }
    });
    let records = slots
        .into_inner()
        .expect("campaign slots")
        .into_iter()
        .map(|slot| slot.expect("campaign worker result"))
        .collect::<Result<Vec<_>, _>>()?;
    let aggregate = aggregate(&records)?;
    Ok(GlobalCampaignResult {
        config,
        records,
        aggregate,
    })
}

pub fn encode_kra10(campaign: &GlobalCampaignResult) -> Result<Vec<u8>, GlobalCampaignError> {
    let count = campaign.records.len();
    if count != usize::from(campaign.config.run_count) {
        return Err(GlobalCampaignError::ArchiveLength);
    }
    let record_length = KRA10_RECORD_HEADER_LENGTH + KSR10_LENGTH;
    let total_length =
        KRA10_HEADER_LENGTH + KSC10_LENGTH + count * record_length + KRA10_FOOTER_LENGTH;
    let mut output = vec![0; total_length];
    output[..5].copy_from_slice(b"KRA10");
    p16(&mut output, 6, 10);
    p16(&mut output, 8, KRA10_HEADER_LENGTH as u16);
    p32(&mut output, 12, total_length as u32);
    p32(&mut output, 16, PHASE10_CONTRACT_ID);
    p32(&mut output, 20, campaign.config.identity);
    p16(&mut output, 24, campaign.config.run_count);
    p16(&mut output, 26, record_length as u16);
    p32(&mut output, 28, campaign.aggregate.summaries_crc32);
    p32(&mut output, 32, campaign.aggregate.ground_contacts);
    p32(
        &mut output,
        36,
        campaign.aggregate.numeric_frame_time_faults,
    );
    p32(&mut output, 40, campaign.aggregate.model_envelope_exceeded);
    let mut config_bytes = [0; KSC10_LENGTH];
    campaign
        .config
        .encode(&mut config_bytes)
        .map_err(|_| GlobalCampaignError::ArchiveRecord)?;
    output[KRA10_HEADER_LENGTH..KRA10_HEADER_LENGTH + KSC10_LENGTH].copy_from_slice(&config_bytes);
    let mut offset = KRA10_HEADER_LENGTH + KSC10_LENGTH;
    for (index, record) in campaign.records.iter().enumerate() {
        output[offset..offset + 5].copy_from_slice(b"KRR10");
        p16(&mut output, offset + 6, 10);
        p32(&mut output, offset + 8, index as u32);
        p32(&mut output, offset + 12, KSR10_LENGTH as u32);
        output[offset + KRA10_RECORD_HEADER_LENGTH..offset + record_length].copy_from_slice(record);
        offset += record_length;
    }
    let footer = total_length - KRA10_FOOTER_LENGTH;
    output[footer..footer + 5].copy_from_slice(b"KRE10");
    p16(&mut output, footer + 6, 10);
    p32(&mut output, footer + 8, campaign.config.identity);
    p32(&mut output, footer + 12, count as u32);
    p32(&mut output, footer + 16, campaign.aggregate.summaries_crc32);
    let header_crc = crc32(&output[..KRA10_HEADER_LENGTH - 4]);
    p32(&mut output, KRA10_HEADER_LENGTH - 4, header_crc);
    let archive_crc = crc32(&output[..footer + 20]);
    p32(&mut output, footer + 20, archive_crc);
    let footer_crc = crc32(&output[footer..footer + KRA10_FOOTER_LENGTH - 4]);
    p32(&mut output, footer + KRA10_FOOTER_LENGTH - 4, footer_crc);
    Ok(output)
}

pub fn validate_kra10(input: &[u8]) -> Result<GlobalCampaignAggregate, GlobalCampaignError> {
    if input.len() < KRA10_HEADER_LENGTH + KSC10_LENGTH + KRA10_FOOTER_LENGTH {
        return Err(GlobalCampaignError::ArchiveLength);
    }
    if &input[..5] != b"KRA10" || input[5] != 0 {
        return Err(GlobalCampaignError::ArchiveMagic);
    }
    if g16(input, 6) != 10
        || g16(input, 8) as usize != KRA10_HEADER_LENGTH
        || g32(input, 12) as usize != input.len()
        || g32(input, 16) != PHASE10_CONTRACT_ID
    {
        return Err(GlobalCampaignError::ArchiveIdentity);
    }
    if input[44..KRA10_HEADER_LENGTH - 4]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(GlobalCampaignError::ArchiveReserved);
    }
    if g32(input, KRA10_HEADER_LENGTH - 4) != crc32(&input[..KRA10_HEADER_LENGTH - 4]) {
        return Err(GlobalCampaignError::ArchiveChecksum);
    }
    let config = GlobalCampaignConfig::decode(
        &input[KRA10_HEADER_LENGTH..KRA10_HEADER_LENGTH + KSC10_LENGTH],
    )
    .map_err(|_| GlobalCampaignError::ArchiveRecord)?;
    if config.identity != g32(input, 20) || config.run_count != g16(input, 24) {
        return Err(GlobalCampaignError::ArchiveIdentity);
    }
    let record_length = KRA10_RECORD_HEADER_LENGTH + KSR10_LENGTH;
    if g16(input, 26) as usize != record_length {
        return Err(GlobalCampaignError::ArchiveLength);
    }
    let mut offset = KRA10_HEADER_LENGTH + KSC10_LENGTH;
    let mut records = Vec::with_capacity(config.run_count as usize);
    for index in 0..config.run_count as usize {
        if &input[offset..offset + 5] != b"KRR10"
            || input[offset + 5] != 0
            || g16(input, offset + 6) != 10
            || g32(input, offset + 8) as usize != index
            || g32(input, offset + 12) as usize != KSR10_LENGTH
        {
            return Err(GlobalCampaignError::ArchiveRecord);
        }
        let mut record = [0; KSR10_LENGTH];
        record.copy_from_slice(&input[offset + KRA10_RECORD_HEADER_LENGTH..offset + record_length]);
        GlobalEvaluationSummary::decode(&record).map_err(|_| GlobalCampaignError::ArchiveRecord)?;
        records.push(record);
        offset += record_length;
    }
    let footer = input.len() - KRA10_FOOTER_LENGTH;
    if offset != footer
        || &input[footer..footer + 5] != b"KRE10"
        || input[footer + 5] != 0
        || g16(input, footer + 6) != 10
        || g32(input, footer + 8) != config.identity
        || g32(input, footer + 12) as usize != records.len()
    {
        return Err(GlobalCampaignError::ArchiveRecord);
    }
    if input[footer + 24..footer + KRA10_FOOTER_LENGTH - 4]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(GlobalCampaignError::ArchiveReserved);
    }
    if g32(input, footer + 20) != crc32(&input[..footer + 20])
        || g32(input, footer + KRA10_FOOTER_LENGTH - 4)
            != crc32(&input[footer..footer + KRA10_FOOTER_LENGTH - 4])
    {
        return Err(GlobalCampaignError::ArchiveChecksum);
    }
    let aggregate = aggregate(&records)?;
    if aggregate.summaries_crc32 != g32(input, 28)
        || aggregate.summaries_crc32 != g32(input, footer + 16)
        || aggregate.ground_contacts != g32(input, 32)
        || aggregate.numeric_frame_time_faults != g32(input, 36)
        || aggregate.model_envelope_exceeded != g32(input, 40)
    {
        return Err(GlobalCampaignError::ArchiveChecksum);
    }
    Ok(aggregate)
}

pub fn materialize_global_variation(
    fixtures: &GlobalFixtureSet,
    variation: GlobalVariation,
) -> Result<(CompiledAtmospherePack, GlobalVehiclePack, GlobalMissionPack), GlobalCampaignError> {
    if variation == GlobalVariation::NOMINAL {
        return Ok((fixtures.atmosphere, fixtures.vehicle, fixtures.mission));
    }
    let mut atmosphere = fixtures.atmosphere;
    let mut vehicle = fixtures.vehicle;
    let mut mission = fixtures.mission;
    let identity_salt = variation.checksum.max(1);
    atmosphere.identity ^= identity_salt.rotate_left(3);
    vehicle.identity ^= identity_salt.rotate_left(7);
    mission.identity ^= identity_salt.rotate_left(11);
    vehicle.source_identity ^= identity_salt;
    mission.source_identity ^= identity_salt.rotate_left(5);
    vehicle.dry_mass_q21_kg = scale_raw(vehicle.dry_mass_q21_kg, variation.mass_scale_q15);
    vehicle.main_propellant_q21_kg =
        scale_raw(vehicle.main_propellant_q21_kg, variation.mass_scale_q15);
    vehicle.thrust_q13_n = scale_raw(vehicle.thrust_q13_n, variation.thrust_scale_q15);
    vehicle.main_mass_flow_q21_kg_s =
        scale_raw(vehicle.main_mass_flow_q21_kg_s, variation.thrust_scale_q15);
    vehicle.rcs_nominal_thrust_q13_n =
        scale_raw(vehicle.rcs_nominal_thrust_q13_n, variation.rcs_scale_q15);
    vehicle.drogue_cda_q24_m2 = scale_raw(vehicle.drogue_cda_q24_m2, variation.recovery_scale_q15);
    vehicle.main_cda_q24_m2 = scale_raw(vehicle.main_cda_q24_m2, variation.recovery_scale_q15);
    for knot in &mut vehicle.aero[..vehicle.aero_count as usize] {
        knot.axial_cd_q24 = scale_raw(knot.axial_cd_q24, variation.aero_scale_q15);
    }
    for knot in &mut atmosphere.knots[..atmosphere.count as usize] {
        knot.density_q28_kg_m3 =
            scale_raw(knot.density_q28_kg_m3, variation.density_scale_q15).max(0);
        knot.wind_enu_q19_m_s = FixedVec3::new(
            knot.wind_enu_q19_m_s
                .x()
                .saturating_add(variation.wind_east_q19_m_s),
            knot.wind_enu_q19_m_s
                .y()
                .saturating_add(variation.wind_north_q19_m_s),
            knot.wind_enu_q19_m_s.z(),
        );
    }
    mission.atmosphere_identity = atmosphere.identity;
    mission.vehicle_identity = vehicle.identity;
    mission.launch_azimuth_q28_rad = mission
        .launch_azimuth_q28_rad
        .saturating_add(variation.launch_azimuth_delta_q28);
    mission.main_deployment_altitude_q12_km = mission
        .main_deployment_altitude_q12_km
        .saturating_add(variation.main_altitude_delta_q12_km);
    atmosphere
        .validate()
        .map_err(|_| GlobalCampaignError::ArchiveRecord)?;
    vehicle
        .validate()
        .map_err(|_| GlobalCampaignError::ArchiveRecord)?;
    mission
        .validate()
        .map_err(|_| GlobalCampaignError::ArchiveRecord)?;
    Ok((atmosphere, vehicle, mission))
}

fn envelope_summary(
    fixtures: &GlobalFixtureSet,
    atmosphere: CompiledAtmospherePack,
    vehicle: GlobalVehiclePack,
    variation: GlobalVariation,
) -> GlobalEvaluationSummary {
    let mut common = EvaluationSummary::empty(ModelProfileId::GlobalEcef6DofV1);
    common.outcome = EvaluationOutcome::ModelEnvelopeExceeded;
    common.identities[0] = fixtures.earth.identity;
    common.identities[1] = fixtures.transforms.identity;
    common.identities[2] = atmosphere.identity;
    common.identities[3] = vehicle.identity;
    common.identities[4] = fixtures.mission.identity;
    common.source_checksums[0] = variation.checksum;
    GlobalEvaluationSummary {
        common,
        terminal_frame: ReferenceFrameId::LocalEnuV1,
        terminal_segment: GlobalSegment::LocalLaunch,
        transition_count: 0,
        earth_identity: fixtures.earth.identity,
        transform_identity: fixtures.transforms.identity,
        atmosphere_identity: atmosphere.identity,
        terminal_ecef_position_q12: [0; 3],
        terminal_ecef_velocity_q24: [0; 3],
        terminal_gcrf_position_q12: [0; 3],
        terminal_gcrf_velocity_q24: [0; 3],
        landing_geodetic_q28_q12: [0; 3],
        apogee_q12_km: 0,
        downrange_q12_km: 0,
        crossrange_q12_km: 0,
        max_navigation_position_error_q12_km: 0,
        max_navigation_velocity_error_q24_km_s: 0,
        max_dynamic_pressure_q14_pa: 0,
        max_acceleration_q28_km_s2: 0,
        max_mach_q24: 0,
        terminal_rcs_propellant_q21_kg: 0,
        time_identity: GLOBAL_TIME_POLICY_ID,
        transition_position_error_q12_km: 0,
        transition_velocity_error_q24_km_s: 0,
        transition_attitude_error_q30: 0,
        transition_angular_rate_error_q24: 0,
        global_checksums: [0; 8],
        transition_checksums: [0; 4],
    }
}

fn aggregate(
    records: &[[u8; KSR10_LENGTH]],
) -> Result<GlobalCampaignAggregate, GlobalCampaignError> {
    let mut ground_contacts = 0;
    let mut physical_recoveries = 0;
    let mut numeric_frame_time_faults = 0;
    let mut model_envelope_exceeded = 0;
    let mut minimum_apogee = i32::MAX;
    let mut maximum_apogee = i32::MIN;
    let mut maximum_downrange = 0;
    let mut maximum_navigation_error = 0;
    let mut bytes = Vec::with_capacity(records.len() * KSR10_LENGTH);
    for record in records {
        let summary = GlobalEvaluationSummary::decode(record)
            .map_err(|_| GlobalCampaignError::ArchiveRecord)?;
        let ground =
            summary.common.outcome == ksa64_core::evaluation::EvaluationOutcome::GroundContact;
        ground_contacts += u32::from(ground);
        physical_recoveries += u32::from(ground && summary.common.numeric_faults == 0);
        numeric_frame_time_faults += u32::from(summary.common.numeric_faults != 0);
        model_envelope_exceeded +=
            u32::from(summary.common.outcome == EvaluationOutcome::ModelEnvelopeExceeded);
        minimum_apogee = minimum_apogee.min(summary.apogee_q12_km);
        maximum_apogee = maximum_apogee.max(summary.apogee_q12_km);
        maximum_downrange = maximum_downrange.max(summary.downrange_q12_km.abs());
        maximum_navigation_error =
            maximum_navigation_error.max(summary.max_navigation_position_error_q12_km);
        bytes.extend_from_slice(record);
    }
    Ok(GlobalCampaignAggregate {
        runs: records.len() as u32,
        ground_contacts,
        physical_recoveries,
        numeric_frame_time_faults,
        model_envelope_exceeded,
        minimum_apogee_q12_km: minimum_apogee,
        maximum_apogee_q12_km: maximum_apogee,
        maximum_downrange_q12_km: maximum_downrange,
        maximum_navigation_error_q12_km: maximum_navigation_error,
        summaries_crc32: crc32(&bytes),
    })
}

fn variation_checksum(value: GlobalVariation, seed: u32, run_index: u32) -> u32 {
    let mut hash = 0x811c_9dc5;
    for word in [
        seed,
        run_index,
        value.mass_scale_q15 as u32,
        value.thrust_scale_q15 as u32,
        value.aero_scale_q15 as u32,
        value.density_scale_q15 as u32,
        value.wind_east_q19_m_s as u32,
        value.wind_north_q19_m_s as u32,
        value.launch_azimuth_delta_q28 as u32,
        value.rcs_scale_q15 as u32,
        value.recovery_scale_q15 as u32,
        value.main_altitude_delta_q12_km as u32,
    ] {
        for byte in word.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
        }
    }
    hash
}

fn keyed_signed(seed: u32, run_index: u32, parameter: u32) -> i32 {
    let mut value =
        seed ^ run_index.wrapping_mul(0x9e37_79b9) ^ parameter.wrapping_mul(0x85eb_ca6b);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    (value ^ (value >> 16)) as i16 as i32
}

fn scale_raw(value: i32, scale_q15: i32) -> i32 {
    ((i64::from(value) * i64::from(scale_q15) + (1 << 14)) >> 15) as i32
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

fn p16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn p32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn g16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn g32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("u32 field"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variation_is_keyed_and_run_zero_is_nominal() {
        assert_eq!(
            derive_global_variation(PHASE10_MASTER_SEED, 0),
            GlobalVariation::NOMINAL
        );
        let first = derive_global_variation(PHASE10_MASTER_SEED, 17);
        let second = derive_global_variation(PHASE10_MASTER_SEED, 17);
        assert_eq!(first, second);
        assert_ne!(first.checksum, 0);
    }

    #[test]
    fn archive_round_trip_rejects_corruption() {
        let fixtures = GlobalFixtureSet::embedded();
        let config = fixtures.campaign_config(64);
        let record = evaluate_global_campaign_run(&fixtures, config, 0).unwrap();
        let records = vec![record; 64];
        let campaign = GlobalCampaignResult {
            config,
            aggregate: aggregate(&records).unwrap(),
            records,
        };
        let archive = encode_kra10(&campaign).unwrap();
        assert_eq!(validate_kra10(&archive).unwrap(), campaign.aggregate);
        let mut corrupt = archive;
        corrupt[KRA10_HEADER_LENGTH + KSC10_LENGTH + KRA10_RECORD_HEADER_LENGTH + 7] ^= 1;
        assert!(validate_kra10(&corrupt).is_err());
    }
}
