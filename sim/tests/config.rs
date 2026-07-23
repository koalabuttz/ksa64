use ksa64_core::phase2_scenario::PHASE2_SCENARIO_IMAGE_LENGTH;
use ksa64_sim::config::*;
use ksa64_sim::mission::MissionCase;
const BASE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
const OTHER: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-early-cutoff.ksc2");
const IMAGES: [(&[u8; PHASE3_CONFIG_LENGTH], MissionCase); 4] = [
    (
        include_bytes!("../../phase3/examples/ksa3-nominal.ksc3"),
        MissionCase::Nominal,
    ),
    (
        include_bytes!("../../phase3/examples/ksa3-altimeter-dropout.ksc3"),
        MissionCase::AltimeterDropout,
    ),
    (
        include_bytes!("../../phase3/examples/ksa3-gps-outage.ksc3"),
        MissionCase::GpsOutage,
    ),
    (
        include_bytes!("../../phase3/examples/ksa3-steering-stuck.ksc3"),
        MissionCase::SteeringStuck,
    ),
];
#[test]
fn frozen_configs_round_trip_and_bind_exact_base() {
    for (image, case) in IMAGES {
        let parsed = parse_phase3_config(image, BASE).unwrap();
        assert_eq!(parsed.case, case);
        let mut generated = [0u8; PHASE3_CONFIG_LENGTH];
        write_phase3_config(BASE, case, &mut generated).unwrap();
        assert_eq!(&generated, image);
        assert_eq!(
            parse_phase3_config(image, OTHER),
            Err(ConfigError::BaseScenario)
        );
    }
}
#[test]
fn corruption_and_reserved_fields_fail_closed() {
    let mut image = *IMAGES[0].0;
    image[20] ^= 1;
    assert_eq!(
        parse_phase3_config(&image, BASE),
        Err(ConfigError::Checksum)
    );
    image = *IMAGES[0].0;
    image[80] = 1;
    let crc = ksa64_interface::crc32_ieee(&image[..92]).to_le_bytes();
    image[92..96].copy_from_slice(&crc);
    assert_eq!(
        parse_phase3_config(&image, BASE),
        Err(ConfigError::Reserved)
    );
}
