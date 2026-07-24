//! Deterministic Phase 5 spatial campaigns and strict KSR5 summaries.
use crate::phase4::campaign::{
    reviewed_campaign_config, sample_distribution, validate_distribution, CampaignError,
    DistributionKind, DistributionSpec, ParameterId, PARAMETER_COUNT,
};
use crate::phase5_mission::{
    run_phase5_parameterized, Phase5MissionOutcome, Phase5MissionParameters, Phase5MissionSummary,
};
use crate::phase5_sensors::Phase5SensorParameters;
use crate::phase5_vehicle::Phase5VehicleParameters;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase5_contract::{
    PHASE5_CAMPAIGN_SEED, PHASE5_REFERENCE_RUNS, PHASE5_ROUTINE_RUNS,
};
use ksa64_interface::crc32_ieee;

pub const PHASE5_MAX_DISTRIBUTIONS: usize = 24;
pub const KSC5_LENGTH: usize = 704;
pub const KSR5_LENGTH: usize = 160;
pub const KSC5_CONTRACT_ID: u32 = 0x050a_0000;
pub const KSR5_CONTRACT_ID: u32 = 0x050a_0001;
pub const KSR5_VERSION: u16 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5CampaignConfig {
    pub master_seed: u32,
    pub run_count: u32,
    pub distribution_count: u8,
    pub distributions: [DistributionSpec; PHASE5_MAX_DISTRIBUTIONS],
}
impl Phase5CampaignConfig {
    pub fn validate(&self) -> Result<(), CampaignError> {
        if self.master_seed == 0 || self.run_count == 0 {
            return Err(CampaignError::Empty);
        }
        if self.run_count > 65_535 {
            return Err(CampaignError::TooManyRuns);
        }
        if self.distribution_count as usize > PHASE5_MAX_DISTRIBUTIONS {
            return Err(CampaignError::TooManyDistributions);
        }
        let mut seen = 0u16;
        for spec in &self.distributions[..self.distribution_count as usize] {
            let bit = 1u16 << spec.parameter.index();
            if seen & bit != 0 {
                return Err(CampaignError::DuplicateParameter);
            }
            seen |= bit;
            validate_distribution(*spec)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5RunVariation {
    values: [i32; PARAMETER_COUNT],
    checksum: u32,
}
impl Phase5RunVariation {
    pub const fn value(self, parameter: ParameterId) -> i32 {
        self.values[parameter.index()]
    }
    pub const fn values(self) -> [i32; PARAMETER_COUNT] {
        self.values
    }
    pub const fn checksum(self) -> u32 {
        self.checksum
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5RunSpec {
    pub index: u32,
    pub sensor_seed: u32,
    pub variation: Phase5RunVariation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5CampaignError {
    Campaign(CampaignError),
    Parameters,
    Mission(crate::phase5_closed_loop::Phase5ClosedLoopError),
    Summary,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5RunSummary {
    pub campaign_seed: u32,
    pub run_index: u32,
    pub sensor_seed: u32,
    pub variation_checksum: u32,
    pub mission: Phase5MissionSummary,
}

pub fn reviewed_phase5_campaign_config(run_count: u32) -> Phase5CampaignConfig {
    let source = reviewed_campaign_config(run_count);
    let mut records = [DistributionSpec::EMPTY; PHASE5_MAX_DISTRIBUTIONS];
    records[..source.distribution_count as usize]
        .copy_from_slice(&source.distributions[..source.distribution_count as usize]);
    let gyro = &mut records[ParameterId::GyroBiasQ24.index()];
    gyro.minimum = -1_464;
    gyro.maximum = 1_464;
    Phase5CampaignConfig {
        master_seed: PHASE5_CAMPAIGN_SEED,
        run_count,
        distribution_count: source.distribution_count,
        distributions: records,
    }
}
pub fn routine_phase5_campaign_config() -> Phase5CampaignConfig {
    reviewed_phase5_campaign_config(PHASE5_ROUTINE_RUNS as u32)
}
pub fn reference_phase5_campaign_config() -> Phase5CampaignConfig {
    reviewed_phase5_campaign_config(PHASE5_REFERENCE_RUNS as u32)
}
fn mix32(mut v: u32) -> u32 {
    v ^= v >> 16;
    v = v.wrapping_mul(0x7feb_352d);
    v ^= v >> 15;
    v = v.wrapping_mul(0x846c_a68b);
    v ^ (v >> 16)
}
pub fn derive_phase5_run(
    c: &Phase5CampaignConfig,
    index: u32,
) -> Result<Phase5RunSpec, CampaignError> {
    c.validate()?;
    if index >= c.run_count {
        return Err(CampaignError::TooManyRuns);
    }
    let mut values = [0i32; PARAMETER_COUNT];
    for spec in &c.distributions[..c.distribution_count as usize] {
        values[spec.parameter.index()] =
            sample_distribution(*spec, c.master_seed, index)? - spec.baseline;
    }
    let sensor_seed = if index == 0 {
        0x5a00_0000
    } else {
        let v = mix32(c.master_seed ^ index.wrapping_mul(0xd1b5_4a35) ^ 0x5345_4544);
        if v == 0 {
            0x6d2b_79f5
        } else {
            v
        }
    };
    let mut bytes = [0u8; PARAMETER_COUNT * 4 + 8];
    bytes[..4].copy_from_slice(&index.to_le_bytes());
    bytes[4..8].copy_from_slice(&sensor_seed.to_le_bytes());
    for (n, v) in values.iter().enumerate() {
        bytes[8 + n * 4..12 + n * 4].copy_from_slice(&v.to_le_bytes())
    }
    Ok(Phase5RunSpec {
        index,
        sensor_seed,
        variation: Phase5RunVariation {
            values,
            checksum: crc32_ieee(&bytes),
        },
    })
}
pub fn phase5_mission_parameters(run: Phase5RunSpec) -> Option<Phase5MissionParameters> {
    let v = run.variation;
    let lag = 4 + v.value(ParameterId::ActuatorLagSteps);
    if !(1..=16).contains(&lag) {
        return None;
    }
    let downrange =
        ((v.value(ParameterId::GpsDownrangeQ32) as i64 * EARTH_RADIUS_Q12 as i64) >> 32) as i32;
    let accel = v.value(ParameterId::AccelerometerBiasQ28);
    let gyro = v.value(ParameterId::GyroBiasQ24);
    let gpsp = v.value(ParameterId::GpsRadialPositionQ12);
    let gpsv = v.value(ParameterId::GpsRadialVelocityQ24);
    let gpst = v.value(ParameterId::GpsTangentialVelocityQ24);
    let p = Phase5MissionParameters {
        sensor_seed: run.sensor_seed,
        sensors: Phase5SensorParameters {
            accelerometer_bias_q28: [accel, -accel / 2, accel / 3],
            gyro_bias_q24: [gyro, gyro / 2, -gyro / 2],
            barometer_bias_q12: v.value(ParameterId::AltimeterBiasQ12),
            gps_position_bias_q12: [gpsp, downrange, 0],
            gps_velocity_bias_q24: [gpsv, gpst, 0],
            noise_scale_ppm: v.value(ParameterId::SensorNoisePpm),
            ..Phase5SensorParameters::DEFAULT
        },
        vehicle: Phase5VehicleParameters {
            payload_mass_ppm: v.value(ParameterId::PayloadMassPpm),
            stage_thrust_ppm: [
                v.value(ParameterId::Stage1ThrustPpm),
                v.value(ParameterId::Stage2ThrustPpm),
            ],
            atmosphere_density_ppm: v.value(ParameterId::AtmosphereDensityPpm),
            aerodynamic_scale_ppm: v.value(ParameterId::DragPpm),
            gimbal_lag_steps: lag as u8,
            gimbal_slew_ppm: v.value(ParameterId::ActuatorSlewPpm),
        },
        guidance_pitch_percent: 100,
        guidance_downrange_percent: 100,
    };
    p.is_valid().then_some(p)
}
pub fn run_phase5_campaign_mission(
    c: &Phase5CampaignConfig,
    index: u32,
) -> Result<Phase5RunSummary, Phase5CampaignError> {
    let run = derive_phase5_run(c, index).map_err(Phase5CampaignError::Campaign)?;
    let p = phase5_mission_parameters(run).ok_or(Phase5CampaignError::Parameters)?;
    let mission = run_phase5_parameterized(p).map_err(Phase5CampaignError::Mission)?;
    Ok(Phase5RunSummary {
        campaign_seed: c.master_seed,
        run_index: index,
        sensor_seed: run.sensor_seed,
        variation_checksum: run.variation.checksum(),
        mission,
    })
}
pub fn write_ksc5(c: &Phase5CampaignConfig, out: &mut [u8]) -> Result<(), Phase5CampaignError> {
    c.validate().map_err(Phase5CampaignError::Campaign)?;
    if out.len() != KSC5_LENGTH {
        return Err(Phase5CampaignError::Summary);
    }
    out.fill(0);
    out[..4].copy_from_slice(b"KSC5");
    pu16(out, 4, 5);
    pu16(out, 6, KSC5_LENGTH as u16);
    pu32(out, 8, KSC5_CONTRACT_ID);
    pu32(
        out,
        12,
        ksa64_core::phase5_contract::PHASE5_NUMERIC_CONTRACT_ID,
    );
    pu32(out, 16, ksa64_core::phase5_contract::PHASE5_SCENARIO_ID);
    pu32(out, 20, c.master_seed);
    pu32(out, 24, c.run_count);
    out[28] = c.distribution_count;
    out[29] = PARAMETER_COUNT as u8;
    out[30] = PHASE5_MAX_DISTRIBUTIONS as u8;
    for n in 0..c.distribution_count as usize {
        let s = c.distributions[n];
        let p = 128 + n * 24;
        out[p] = s.parameter as u8;
        out[p + 1] = s.kind as u8;
        out[p + 2] = s.correlation_group;
        pi32(out, p + 4, s.minimum);
        pi32(out, p + 8, s.baseline);
        pi32(out, p + 12, s.maximum);
        pi32(out, p + 16, s.shape);
        pu32(out, p + 20, crc32_ieee(&out[p..p + 20]))
    }
    pu32(out, 120, crc32_ieee(&out[128..]));
    pu32(out, 124, crc32_ieee(&out[..124]));
    Ok(())
}
pub fn parse_ksc5(input: &[u8]) -> Result<Phase5CampaignConfig, Phase5CampaignError> {
    if input.len() != KSC5_LENGTH
        || &input[..4] != b"KSC5"
        || gu16(input, 4) != 5
        || gu16(input, 6) != KSC5_LENGTH as u16
        || gu32(input, 8) != KSC5_CONTRACT_ID
        || gu32(input, 12) != ksa64_core::phase5_contract::PHASE5_NUMERIC_CONTRACT_ID
        || gu32(input, 16) != ksa64_core::phase5_contract::PHASE5_SCENARIO_ID
    {
        return Err(Phase5CampaignError::Summary);
    }
    if input[29] != PARAMETER_COUNT as u8
        || input[30] != PHASE5_MAX_DISTRIBUTIONS as u8
        || input[31..120].iter().any(|&b| b != 0)
        || gu32(input, 120) != crc32_ieee(&input[128..])
        || gu32(input, 124) != crc32_ieee(&input[..124])
    {
        return Err(Phase5CampaignError::Summary);
    }
    let count = input[28] as usize;
    if count > PHASE5_MAX_DISTRIBUTIONS {
        return Err(Phase5CampaignError::Summary);
    }
    let mut records = [DistributionSpec::EMPTY; PHASE5_MAX_DISTRIBUTIONS];
    for (n, record) in records.iter_mut().enumerate() {
        let p = 128 + n * 24;
        if n >= count {
            if input[p..p + 24].iter().any(|&b| b != 0) {
                return Err(Phase5CampaignError::Summary);
            }
        } else {
            if input[p + 3] != 0 || gu32(input, p + 20) != crc32_ieee(&input[p..p + 20]) {
                return Err(Phase5CampaignError::Summary);
            }
            *record = DistributionSpec {
                parameter: ParameterId::from_byte(input[p]).ok_or(Phase5CampaignError::Summary)?,
                kind: DistributionKind::from_byte(input[p + 1])
                    .ok_or(Phase5CampaignError::Summary)?,
                correlation_group: input[p + 2],
                minimum: gi32(input, p + 4),
                baseline: gi32(input, p + 8),
                maximum: gi32(input, p + 12),
                shape: gi32(input, p + 16),
            }
        }
    }
    let c = Phase5CampaignConfig {
        master_seed: gu32(input, 20),
        run_count: gu32(input, 24),
        distribution_count: count as u8,
        distributions: records,
    };
    c.validate().map_err(Phase5CampaignError::Campaign)?;
    Ok(c)
}
pub fn write_ksr5(s: &Phase5RunSummary, out: &mut [u8]) -> Result<(), Phase5CampaignError> {
    if out.len() != KSR5_LENGTH {
        return Err(Phase5CampaignError::Summary);
    }
    out.fill(0);
    out[..4].copy_from_slice(b"KSR5");
    pu16(out, 4, KSR5_VERSION);
    pu16(out, 6, KSR5_LENGTH as u16);
    pu32(out, 8, KSR5_CONTRACT_ID);
    pu32(out, 12, s.campaign_seed);
    pu32(out, 16, s.run_index);
    pu32(out, 20, s.sensor_seed);
    pu32(out, 24, s.variation_checksum);
    out[28] = s.mission.outcome as u8;
    out[29] = s.mission.case as u8;
    pu32(out, 32, s.mission.steps);
    pia(out, 36, &s.mission.terminal_position_q12);
    pia(out, 48, &s.mission.terminal_velocity_q24);
    pi32(out, 60, s.mission.perigee_altitude_q12);
    pi32(out, 64, s.mission.apogee_altitude_q12);
    pu16(out, 68, s.mission.inclination_turn16);
    pu16(out, 70, s.mission.events);
    pi32(out, 72, s.mission.max_dynamic_pressure_q16);
    pi32(out, 76, s.mission.max_aoa_sine_q16);
    pi32(out, 80, s.mission.max_flexible_state_q24);
    pi32(out, 84, s.mission.max_nav_position_error_q12);
    pu32(out, 88, s.mission.sensor_checksum);
    pu32(out, 92, s.mission.navigation_checksum);
    pu32(out, 96, s.mission.flight_checksum);
    pu32(out, 100, s.mission.summary_checksum);
    pu32(out, 156, crc32_ieee(&out[..156]));
    Ok(())
}
pub fn parse_ksr5(input: &[u8]) -> Result<Phase5RunSummary, Phase5CampaignError> {
    if input.len() != 160
        || &input[..4] != b"KSR5"
        || gu16(input, 4) != 5
        || gu16(input, 6) != 160
        || gu32(input, 8) != KSR5_CONTRACT_ID
    {
        return Err(Phase5CampaignError::Summary);
    }
    if input[30..32]
        .iter()
        .chain(input[104..156].iter())
        .any(|&b| b != 0)
        || gu32(input, 156) != crc32_ieee(&input[..156])
    {
        return Err(Phase5CampaignError::Summary);
    }
    let outcome = match input[28] {
        0 => Phase5MissionOutcome::StableOrbit,
        1 => Phase5MissionOutcome::CompleteNotOrbit,
        2 => Phase5MissionOutcome::Aborted,
        3 => Phase5MissionOutcome::NumericFault,
        4 => Phase5MissionOutcome::StepLimit,
        _ => return Err(Phase5CampaignError::Summary),
    };
    let case = match input[29] {
        0 => crate::phase5_mission::Phase5MissionCase::Nominal,
        1 => crate::phase5_mission::Phase5MissionCase::GustAndSlosh,
        2 => crate::phase5_mission::Phase5MissionCase::StarOutageAndGyroBias,
        3 => crate::phase5_mission::Phase5MissionCase::GimbalJamAbort,
        4 => crate::phase5_mission::Phase5MissionCase::DampingLossAbort,
        5 => crate::phase5_mission::Phase5MissionCase::RcsLeakAndDepletion,
        _ => return Err(Phase5CampaignError::Summary),
    };
    Ok(Phase5RunSummary {
        campaign_seed: gu32(input, 12),
        run_index: gu32(input, 16),
        sensor_seed: gu32(input, 20),
        variation_checksum: gu32(input, 24),
        mission: Phase5MissionSummary {
            case,
            outcome,
            steps: gu32(input, 32),
            terminal_position_q12: gia(input, 36),
            terminal_velocity_q24: gia(input, 48),
            perigee_altitude_q12: gi32(input, 60),
            apogee_altitude_q12: gi32(input, 64),
            inclination_turn16: gu16(input, 68),
            events: gu16(input, 70),
            max_dynamic_pressure_q16: gi32(input, 72),
            max_aoa_sine_q16: gi32(input, 76),
            max_flexible_state_q24: gi32(input, 80),
            max_nav_position_error_q12: gi32(input, 84),
            sensor_checksum: gu32(input, 88),
            navigation_checksum: gu32(input, 92),
            flight_checksum: gu32(input, 96),
            summary_checksum: gu32(input, 100),
        },
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5CampaignAggregate {
    pub runs: u32,
    pub outcome_counts: [u32; 5],
    pub min_perigee_q12: i32,
    pub max_perigee_q12: i32,
    pub min_apogee_q12: i32,
    pub max_apogee_q12: i32,
    pub max_dynamic_pressure_q16: i32,
    pub max_nav_error_q12: i32,
    pub summary_chain: u32,
}
impl Phase5CampaignAggregate {
    pub const fn new() -> Self {
        Self {
            runs: 0,
            outcome_counts: [0; 5],
            min_perigee_q12: i32::MAX,
            max_perigee_q12: i32::MIN,
            min_apogee_q12: i32::MAX,
            max_apogee_q12: i32::MIN,
            max_dynamic_pressure_q16: 0,
            max_nav_error_q12: 0,
            summary_chain: 2_166_136_261,
        }
    }
    pub fn update(&mut self, s: &Phase5RunSummary) {
        self.runs += 1;
        self.outcome_counts[s.mission.outcome as usize] += 1;
        self.min_perigee_q12 = self.min_perigee_q12.min(s.mission.perigee_altitude_q12);
        self.max_perigee_q12 = self.max_perigee_q12.max(s.mission.perigee_altitude_q12);
        self.min_apogee_q12 = self.min_apogee_q12.min(s.mission.apogee_altitude_q12);
        self.max_apogee_q12 = self.max_apogee_q12.max(s.mission.apogee_altitude_q12);
        self.max_dynamic_pressure_q16 = self
            .max_dynamic_pressure_q16
            .max(s.mission.max_dynamic_pressure_q16);
        self.max_nav_error_q12 = self
            .max_nav_error_q12
            .max(s.mission.max_nav_position_error_q12);
        let mut b = [0u8; 160];
        if write_ksr5(s, &mut b).is_ok() {
            self.summary_chain = fnv(self.summary_chain, &b)
        }
    }
}
impl Default for Phase5CampaignAggregate {
    fn default() -> Self {
        Self::new()
    }
}
pub trait Phase5CampaignSink {
    type Error;
    fn observe(&mut self, s: &Phase5RunSummary) -> Result<(), Self::Error>;
}
pub fn run_phase5_campaign<S: Phase5CampaignSink>(
    c: &Phase5CampaignConfig,
    sink: &mut S,
) -> Result<Phase5CampaignAggregate, Phase5CampaignRunError<S::Error>> {
    c.validate().map_err(Phase5CampaignRunError::Campaign)?;
    let mut a = Phase5CampaignAggregate::new();
    for i in 0..c.run_count {
        let s = run_phase5_campaign_mission(c, i).map_err(Phase5CampaignRunError::Run)?;
        sink.observe(&s).map_err(Phase5CampaignRunError::Sink)?;
        a.update(&s)
    }
    Ok(a)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5CampaignRunError<E> {
    Campaign(CampaignError),
    Run(Phase5CampaignError),
    Sink(E),
}
fn pu16(o: &mut [u8], p: usize, v: u16) {
    o[p..p + 2].copy_from_slice(&v.to_le_bytes())
}
fn pu32(o: &mut [u8], p: usize, v: u32) {
    o[p..p + 4].copy_from_slice(&v.to_le_bytes())
}
fn pi32(o: &mut [u8], p: usize, v: i32) {
    pu32(o, p, v as u32)
}
fn gu16(i: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([i[p], i[p + 1]])
}
fn gu32(i: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([i[p], i[p + 1], i[p + 2], i[p + 3]])
}
fn gi32(i: &[u8], p: usize) -> i32 {
    gu32(i, p) as i32
}
fn pia(o: &mut [u8], p: usize, v: &[i32]) {
    for (n, x) in v.iter().enumerate() {
        pi32(o, p + 4 * n, *x)
    }
}
fn gia<const N: usize>(i: &[u8], p: usize) -> [i32; N] {
    let mut v = [0; N];
    let mut n = 0;
    while n < N {
        v[n] = gi32(i, p + 4 * n);
        n += 1
    }
    v
}
fn fnv(mut h: u32, b: &[u8]) -> u32 {
    for x in b {
        h ^= *x as u32;
        h = h.wrapping_mul(16_777_619)
    }
    h
}
