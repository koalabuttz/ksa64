//! Allocation-free deterministic Phase 4 campaign sampling.

use ksa64_interface::crc32_ieee;

use super::contracts::{MAX_DISTRIBUTIONS, REFERENCE_MASTER_SEED};

pub const PARAMETER_COUNT: usize = 15;
pub const MAX_ABSOLUTE_SAMPLE: i32 = 16_777_216;
pub const MAX_SAMPLE_SPAN: i32 = 33_554_432;
pub const PROBABILITY_SCALE: i32 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ParameterId {
    PayloadMassPpm = 0,
    Stage1ThrustPpm = 1,
    Stage2ThrustPpm = 2,
    AtmosphereDensityPpm = 3,
    DragPpm = 4,
    AccelerometerBiasQ28 = 5,
    GyroBiasQ24 = 6,
    AltimeterBiasQ12 = 7,
    GpsRadialPositionQ12 = 8,
    GpsDownrangeQ32 = 9,
    GpsRadialVelocityQ24 = 10,
    GpsTangentialVelocityQ24 = 11,
    SensorNoisePpm = 12,
    ActuatorLagSteps = 13,
    ActuatorSlewPpm = 14,
}
impl ParameterId {
    pub const ALL: [Self; PARAMETER_COUNT] = [
        Self::PayloadMassPpm,
        Self::Stage1ThrustPpm,
        Self::Stage2ThrustPpm,
        Self::AtmosphereDensityPpm,
        Self::DragPpm,
        Self::AccelerometerBiasQ28,
        Self::GyroBiasQ24,
        Self::AltimeterBiasQ12,
        Self::GpsRadialPositionQ12,
        Self::GpsDownrangeQ32,
        Self::GpsRadialVelocityQ24,
        Self::GpsTangentialVelocityQ24,
        Self::SensorNoisePpm,
        Self::ActuatorLagSteps,
        Self::ActuatorSlewPpm,
    ];
    pub const fn index(self) -> usize {
        self as usize
    }
    pub const fn from_byte(value: u8) -> Option<Self> {
        if value < PARAMETER_COUNT as u8 {
            Some(Self::ALL[value as usize])
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DistributionKind {
    Fixed = 0,
    Uniform = 1,
    Triangular = 2,
    Bernoulli = 3,
    CltNormal3Sigma = 4,
}
impl DistributionKind {
    pub const fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Fixed),
            1 => Some(Self::Uniform),
            2 => Some(Self::Triangular),
            3 => Some(Self::Bernoulli),
            4 => Some(Self::CltNormal3Sigma),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DistributionSpec {
    pub parameter: ParameterId,
    pub kind: DistributionKind,
    pub correlation_group: u8,
    pub minimum: i32,
    pub baseline: i32,
    pub maximum: i32,
    pub shape: i32,
}
impl DistributionSpec {
    pub const EMPTY: Self = Self {
        parameter: ParameterId::PayloadMassPpm,
        kind: DistributionKind::Fixed,
        correlation_group: 0,
        minimum: 0,
        baseline: 0,
        maximum: 0,
        shape: 0,
    };
    pub const fn fixed(parameter: ParameterId, value: i32) -> Self {
        Self {
            parameter,
            kind: DistributionKind::Fixed,
            correlation_group: 0,
            minimum: value,
            baseline: value,
            maximum: value,
            shape: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignError {
    Empty,
    TooManyRuns,
    TooManyDistributions,
    DuplicateParameter,
    Range,
    Distribution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignConfig {
    pub master_seed: u32,
    pub run_count: u32,
    pub distribution_count: u8,
    pub distributions: [DistributionSpec; MAX_DISTRIBUTIONS],
}
impl CampaignConfig {
    pub const fn empty(run_count: u32) -> Self {
        Self {
            master_seed: REFERENCE_MASTER_SEED,
            run_count,
            distribution_count: 0,
            distributions: [DistributionSpec::EMPTY; MAX_DISTRIBUTIONS],
        }
    }
    pub fn push(&mut self, spec: DistributionSpec) -> Result<(), CampaignError> {
        if self.distribution_count as usize >= MAX_DISTRIBUTIONS {
            return Err(CampaignError::TooManyDistributions);
        }
        self.distributions[self.distribution_count as usize] = spec;
        self.distribution_count += 1;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), CampaignError> {
        if self.master_seed == 0 || self.run_count == 0 {
            return Err(CampaignError::Empty);
        }
        if self.run_count > 65_535 {
            return Err(CampaignError::TooManyRuns);
        }
        if self.distribution_count as usize > MAX_DISTRIBUTIONS {
            return Err(CampaignError::TooManyDistributions);
        }
        let mut seen = 0u16;
        let mut index = 0;
        while index < self.distribution_count as usize {
            let spec = self.distributions[index];
            let bit = 1u16 << spec.parameter.index();
            if seen & bit != 0 {
                return Err(CampaignError::DuplicateParameter);
            }
            seen |= bit;
            validate_distribution(spec)?;
            index += 1;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunVariation {
    values: [i32; PARAMETER_COUNT],
    checksum: u32,
}
impl RunVariation {
    pub const ZERO: Self = Self {
        values: [0; PARAMETER_COUNT],
        checksum: 0,
    };
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
pub struct RunSpec {
    pub index: u32,
    pub sensor_seed: u32,
    pub variation: RunVariation,
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

pub fn keyed_word(
    master_seed: u32,
    run_index: u32,
    parameter: ParameterId,
    correlation_group: u8,
    draw_index: u8,
) -> u32 {
    let source = if correlation_group == 0 {
        parameter as u32
    } else {
        0x100 + correlation_group as u32
    };
    mix32(
        master_seed
            ^ run_index.wrapping_mul(0x9e37_79b9)
            ^ source.wrapping_mul(0x85eb_ca6b)
            ^ (draw_index as u32).wrapping_mul(0xc2b2_ae35),
    )
}

/// Phase-neutral access to the frozen keyed draw used by later campaign
/// catalogs. Existing Phase 4 draws retain their original entry point.
pub fn keyed_word_raw(
    master_seed: u32,
    run_index: u32,
    parameter: u8,
    correlation_group: u8,
    draw_index: u8,
) -> u32 {
    let source = if correlation_group == 0 {
        parameter as u32
    } else {
        0x100 + correlation_group as u32
    };
    mix32(
        master_seed
            ^ run_index.wrapping_mul(0x9e37_79b9)
            ^ source.wrapping_mul(0x85eb_ca6b)
            ^ (draw_index as u32).wrapping_mul(0xc2b2_ae35),
    )
}

fn multiply_high_u32(a: u32, b: u32) -> u32 {
    let a0 = a & 0xffff;
    let a1 = a >> 16;
    let b0 = b & 0xffff;
    let b1 = b >> 16;
    let p0 = a0 * b0;
    let p1 = a0 * b1;
    let p2 = a1 * b0;
    let p3 = a1 * b1;
    let carry = (p0 >> 16)
        .wrapping_add(p1 & 0xffff)
        .wrapping_add(p2 & 0xffff);
    p3.wrapping_add(p1 >> 16)
        .wrapping_add(p2 >> 16)
        .wrapping_add(carry >> 16)
}

fn uniform_between(word: u32, minimum: i32, maximum: i32) -> i32 {
    if minimum == maximum {
        return minimum;
    }
    let span = (maximum - minimum) as u32 + 1;
    minimum + multiply_high_u32(word, span) as i32
}

pub fn validate_distribution(spec: DistributionSpec) -> Result<(), CampaignError> {
    if spec.minimum > spec.baseline
        || spec.baseline > spec.maximum
        || spec.minimum < -MAX_ABSOLUTE_SAMPLE
        || spec.maximum > MAX_ABSOLUTE_SAMPLE
        || spec.maximum - spec.minimum > MAX_SAMPLE_SPAN
    {
        return Err(CampaignError::Range);
    }
    match spec.kind {
        DistributionKind::Fixed => {
            if spec.minimum != spec.baseline || spec.baseline != spec.maximum || spec.shape != 0 {
                return Err(CampaignError::Distribution);
            }
        }
        DistributionKind::Uniform => {
            if spec.shape != 0 {
                return Err(CampaignError::Distribution);
            }
        }
        DistributionKind::Triangular => {
            if spec.shape != 0 || spec.baseline != spec.minimum + (spec.maximum - spec.minimum) / 2
            {
                return Err(CampaignError::Distribution);
            }
        }
        DistributionKind::Bernoulli => {
            if !(0..=PROBABILITY_SCALE).contains(&spec.shape) {
                return Err(CampaignError::Distribution);
            }
        }
        DistributionKind::CltNormal3Sigma => {
            if spec.shape != 0
                || spec.minimum == spec.maximum
                || spec.baseline == spec.minimum
                || spec.baseline == spec.maximum
            {
                return Err(CampaignError::Distribution);
            }
        }
    }
    Ok(())
}

pub fn sample_distribution(
    spec: DistributionSpec,
    master_seed: u32,
    run_index: u32,
) -> Result<i32, CampaignError> {
    validate_distribution(spec)?;
    if run_index == 0 || spec.kind == DistributionKind::Fixed {
        return Ok(spec.baseline);
    }
    let word = |draw| {
        keyed_word(
            master_seed,
            run_index,
            spec.parameter,
            spec.correlation_group,
            draw,
        )
    };
    let value = match spec.kind {
        DistributionKind::Fixed => spec.baseline,
        DistributionKind::Uniform => uniform_between(word(0), spec.minimum, spec.maximum),
        DistributionKind::Triangular => {
            let a = uniform_between(word(0), spec.minimum, spec.maximum);
            let b = uniform_between(word(1), spec.minimum, spec.maximum);
            (a + b) / 2
        }
        DistributionKind::Bernoulli => {
            if multiply_high_u32(word(0), PROBABILITY_SCALE as u32) < spec.shape as u32 {
                spec.maximum
            } else {
                spec.minimum
            }
        }
        DistributionKind::CltNormal3Sigma => {
            let mut total = 0i32;
            let mut draw = 0u8;
            while draw < 12 {
                total += (word(draw) & 0xff) as i32;
                draw += 1;
            }
            let centered = total - 1_530;
            let span = if centered >= 0 {
                spec.maximum - spec.baseline
            } else {
                spec.baseline - spec.minimum
            };
            let delta = ((centered as i64 * span as i64) / 768) as i32;
            (spec.baseline + delta).clamp(spec.minimum, spec.maximum)
        }
    };
    Ok(value)
}

pub fn derive_run(config: &CampaignConfig, index: u32) -> Result<RunSpec, CampaignError> {
    config.validate()?;
    if index >= config.run_count {
        return Err(CampaignError::TooManyRuns);
    }
    let mut values = [0i32; PARAMETER_COUNT];
    let mut spec_index = 0;
    while spec_index < config.distribution_count as usize {
        let spec = config.distributions[spec_index];
        values[spec.parameter.index()] =
            sample_distribution(spec, config.master_seed, index)? - spec.baseline;
        spec_index += 1;
    }
    let sensor_seed = if index == 0 {
        0x4b53_4133
    } else {
        let derived = mix32(config.master_seed ^ index.wrapping_mul(0xd1b5_4a35) ^ 0x5345_4544);
        if derived == 0 {
            0x6d2b_79f5
        } else {
            derived
        }
    };
    let mut bytes = [0u8; PARAMETER_COUNT * 4 + 8];
    bytes[..4].copy_from_slice(&index.to_le_bytes());
    bytes[4..8].copy_from_slice(&sensor_seed.to_le_bytes());
    let mut value_index = 0;
    while value_index < PARAMETER_COUNT {
        let at = 8 + value_index * 4;
        bytes[at..at + 4].copy_from_slice(&values[value_index].to_le_bytes());
        value_index += 1;
    }
    Ok(RunSpec {
        index,
        sensor_seed,
        variation: RunVariation {
            values,
            checksum: crc32_ieee(&bytes),
        },
    })
}

pub const fn reviewed_campaign_config(run_count: u32) -> CampaignConfig {
    let clt = DistributionKind::CltNormal3Sigma;
    let mut records = [DistributionSpec::EMPTY; MAX_DISTRIBUTIONS];
    records[0] = DistributionSpec {
        parameter: ParameterId::PayloadMassPpm,
        kind: clt,
        correlation_group: 0,
        minimum: -5_000,
        baseline: 0,
        maximum: 5_000,
        shape: 0,
    };
    records[1] = DistributionSpec {
        parameter: ParameterId::Stage1ThrustPpm,
        kind: clt,
        correlation_group: 0,
        minimum: -10_000,
        baseline: 0,
        maximum: 10_000,
        shape: 0,
    };
    records[2] = DistributionSpec {
        parameter: ParameterId::Stage2ThrustPpm,
        kind: clt,
        correlation_group: 0,
        minimum: -10_000,
        baseline: 0,
        maximum: 10_000,
        shape: 0,
    };
    records[3] = DistributionSpec {
        parameter: ParameterId::AtmosphereDensityPpm,
        kind: clt,
        correlation_group: 0,
        minimum: -50_000,
        baseline: 0,
        maximum: 50_000,
        shape: 0,
    };
    records[4] = DistributionSpec {
        parameter: ParameterId::DragPpm,
        kind: clt,
        correlation_group: 0,
        minimum: -50_000,
        baseline: 0,
        maximum: 50_000,
        shape: 0,
    };
    records[5] = DistributionSpec {
        parameter: ParameterId::AccelerometerBiasQ28,
        kind: clt,
        correlation_group: 0,
        minimum: -1_342,
        baseline: 0,
        maximum: 1_342,
        shape: 0,
    };
    records[6] = DistributionSpec {
        parameter: ParameterId::GyroBiasQ24,
        kind: clt,
        correlation_group: 0,
        minimum: -83_886,
        baseline: 0,
        maximum: 83_886,
        shape: 0,
    };
    records[7] = DistributionSpec {
        parameter: ParameterId::AltimeterBiasQ12,
        kind: clt,
        correlation_group: 0,
        minimum: -102,
        baseline: 0,
        maximum: 102,
        shape: 0,
    };
    records[8] = DistributionSpec {
        parameter: ParameterId::GpsRadialPositionQ12,
        kind: clt,
        correlation_group: 0,
        minimum: -205,
        baseline: 0,
        maximum: 205,
        shape: 0,
    };
    records[9] = DistributionSpec {
        parameter: ParameterId::GpsDownrangeQ32,
        kind: clt,
        correlation_group: 0,
        minimum: -5_362,
        baseline: 0,
        maximum: 5_362,
        shape: 0,
    };
    records[10] = DistributionSpec {
        parameter: ParameterId::GpsRadialVelocityQ24,
        kind: clt,
        correlation_group: 0,
        minimum: -8_389,
        baseline: 0,
        maximum: 8_389,
        shape: 0,
    };
    records[11] = DistributionSpec {
        parameter: ParameterId::GpsTangentialVelocityQ24,
        kind: clt,
        correlation_group: 0,
        minimum: -8_389,
        baseline: 0,
        maximum: 8_389,
        shape: 0,
    };
    records[12] = DistributionSpec {
        parameter: ParameterId::SensorNoisePpm,
        kind: clt,
        correlation_group: 0,
        minimum: -250_000,
        baseline: 0,
        maximum: 250_000,
        shape: 0,
    };
    records[13] = DistributionSpec {
        parameter: ParameterId::ActuatorLagSteps,
        kind: DistributionKind::Triangular,
        correlation_group: 0,
        minimum: 2,
        baseline: 4,
        maximum: 6,
        shape: 0,
    };
    records[14] = DistributionSpec {
        parameter: ParameterId::ActuatorSlewPpm,
        kind: clt,
        correlation_group: 0,
        minimum: -100_000,
        baseline: 0,
        maximum: 100_000,
        shape: 0,
    };
    CampaignConfig {
        master_seed: REFERENCE_MASTER_SEED,
        run_count,
        distribution_count: PARAMETER_COUNT as u8,
        distributions: records,
    }
}
#[cfg(feature = "fixtures")]
pub fn distribution_fixture_config() -> CampaignConfig {
    let mut config = CampaignConfig::empty(1_024);
    let specs = [
        DistributionSpec {
            parameter: ParameterId::PayloadMassPpm,
            kind: DistributionKind::Fixed,
            correlation_group: 0,
            minimum: 123,
            baseline: 123,
            maximum: 123,
            shape: 0,
        },
        DistributionSpec {
            parameter: ParameterId::Stage1ThrustPpm,
            kind: DistributionKind::Uniform,
            correlation_group: 0,
            minimum: -10_000,
            baseline: 0,
            maximum: 10_000,
            shape: 0,
        },
        DistributionSpec {
            parameter: ParameterId::Stage2ThrustPpm,
            kind: DistributionKind::Triangular,
            correlation_group: 0,
            minimum: -20_000,
            baseline: 0,
            maximum: 20_000,
            shape: 0,
        },
        DistributionSpec {
            parameter: ParameterId::AtmosphereDensityPpm,
            kind: DistributionKind::Bernoulli,
            correlation_group: 0,
            minimum: 0,
            baseline: 0,
            maximum: 1,
            shape: 333_333,
        },
        DistributionSpec {
            parameter: ParameterId::DragPpm,
            kind: DistributionKind::CltNormal3Sigma,
            correlation_group: 0,
            minimum: -300_000,
            baseline: 0,
            maximum: 300_000,
            shape: 0,
        },
        DistributionSpec {
            parameter: ParameterId::AccelerometerBiasQ28,
            kind: DistributionKind::CltNormal3Sigma,
            correlation_group: 7,
            minimum: -100_000,
            baseline: 0,
            maximum: 100_000,
            shape: 0,
        },
        DistributionSpec {
            parameter: ParameterId::GyroBiasQ24,
            kind: DistributionKind::CltNormal3Sigma,
            correlation_group: 7,
            minimum: -100_000,
            baseline: 0,
            maximum: 100_000,
            shape: 0,
        },
    ];
    let mut index = 0;
    while index < specs.len() {
        config.push(specs[index]).expect("fixture capacity");
        index += 1;
    }
    config
}

#[cfg(feature = "fixtures")]
pub fn run_distribution_self_tests() -> u16 {
    use super::generated_distribution_vectors::EXPECTED;
    let config = distribution_fixture_config();
    let mut failures = 0u16;
    let mut index = 0;
    while index < EXPECTED.len() {
        let (run, seed, checksum, values) = EXPECTED[index];
        match derive_run(&config, run) {
            Ok(actual) => {
                failures += u16::from(actual.sensor_seed != seed);
                failures += u16::from(actual.variation.checksum() != checksum);
                failures += u16::from(actual.variation.values() != values);
                failures += u16::from(
                    actual.variation.value(ParameterId::AccelerometerBiasQ28)
                        != actual.variation.value(ParameterId::GyroBiasQ24),
                );
            }
            Err(_) => failures += 1,
        }
        index += 1;
    }
    failures
}
