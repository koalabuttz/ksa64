use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::{
    derive_run, distribution_fixture_config, sample_distribution, CampaignConfig, CampaignError,
    DistributionKind, DistributionSpec, ParameterId,
};
use ksa64_sim::phase4::generated_distribution_vectors::{CLT_CRC32, EXPECTED};

#[test]
fn independent_vectors_match_all_distribution_families() {
    let config = distribution_fixture_config();
    for &(run, seed, checksum, values) in EXPECTED {
        let actual = derive_run(&config, run).unwrap();
        assert_eq!(actual.sensor_seed, seed);
        assert_eq!(actual.variation.checksum(), checksum);
        assert_eq!(actual.variation.values(), values);
    }
}

#[test]
fn keyed_sampling_is_order_independent_and_groups_are_correlated() {
    let original = distribution_fixture_config();
    let mut reversed = CampaignConfig::empty(original.run_count);
    for index in (0..original.distribution_count as usize).rev() {
        reversed.push(original.distributions[index]).unwrap();
    }
    for run in [1, 2, 17, 63, 1023] {
        let a = derive_run(&original, run).unwrap();
        let b = derive_run(&reversed, run).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a.variation.value(ParameterId::AccelerometerBiasQ28),
            a.variation.value(ParameterId::GyroBiasQ24)
        );
    }
}

#[test]
fn invalid_ranges_duplicates_and_shapes_fail_closed() {
    let mut duplicate = CampaignConfig::empty(4);
    let fixed = DistributionSpec::fixed(ParameterId::PayloadMassPpm, 0);
    duplicate.push(fixed).unwrap();
    duplicate.push(fixed).unwrap();
    assert_eq!(duplicate.validate(), Err(CampaignError::DuplicateParameter));
    let invalid = DistributionSpec {
        parameter: ParameterId::ActuatorLagSteps,
        kind: DistributionKind::Triangular,
        correlation_group: 0,
        minimum: -2,
        baseline: 1,
        maximum: 2,
        shape: 0,
    };
    assert_eq!(
        sample_distribution(invalid, 1, 1),
        Err(CampaignError::Distribution)
    );
}

#[test]
fn clt_65536_sample_histogram_crc_matches_independent_generator() {
    let spec = DistributionSpec {
        parameter: ParameterId::DragPpm,
        kind: DistributionKind::CltNormal3Sigma,
        correlation_group: 0,
        minimum: -300_000,
        baseline: 0,
        maximum: 300_000,
        shape: 0,
    };
    let mut bytes = Vec::with_capacity(65_536 * 4);
    for run in 1..=65_536 {
        bytes.extend_from_slice(
            &sample_distribution(spec, 0x4b53_4134, run)
                .unwrap()
                .to_le_bytes(),
        );
    }
    assert_eq!(crc32_ieee(&bytes), CLT_CRC32);
}
