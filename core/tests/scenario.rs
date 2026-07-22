use ksa64_core::scenario::{
    crc32_ieee, parse_scenario_image, ScenarioError, ScenarioField, NUMERIC_CONTRACT_ID,
    SCENARIO_IMAGE_LENGTH, SIMPLE_EARTH_ENVIRONMENT_ID,
};

const GOLDEN: &[u8; SCENARIO_IMAGE_LENGTH] = include_bytes!("../../phase0/numeric/scenario-v1.bin");

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    write_u32(bytes, offset, value as u32);
}

fn repair_crc(bytes: &mut [u8; SCENARIO_IMAGE_LENGTH]) {
    let crc = crc32_ieee(&bytes[..72]);
    write_u32(bytes, 72, crc);
}

#[test]
fn standard_crc_vector_matches_ieee() {
    assert_eq!(crc32_ieee(b"123456789"), 0xcbf4_3926);
}

#[test]
fn golden_scenario_parses_to_strong_types() {
    let scenario = parse_scenario_image(GOLDEN).unwrap();
    assert_eq!(scenario.scenario_id(), 0xef03_0ab2);
    assert_eq!(scenario.timestep().raw(), 8_192);
    assert_eq!(scenario.steps(), 2_048);
    assert_eq!(scenario.telemetry_stride(), 8);
    assert_eq!(scenario.flags(), 0);
    assert_eq!(scenario.seed(), 0);
    assert_eq!(scenario.initial().altitude().raw(), 0);
    assert_eq!(scenario.initial().velocity().raw(), 0);
    assert_eq!(scenario.initial().total_mass().raw(), 2_048_000);
    assert_eq!(scenario.initial().propellant().raw(), 1_556_480);
    assert_eq!(scenario.vehicle().dry_mass().raw(), 491_520);
    assert_eq!(scenario.vehicle().thrust().raw(), 31_130);
    assert_eq!(scenario.vehicle().mass_flow().raw(), 163_840);
    assert_eq!(scenario.vehicle().burn_duration().raw(), 9_961_472);
    assert_eq!(scenario.vehicle().cda().raw(), 655_360);
    assert_eq!(scenario.environment_id(), SIMPLE_EARTH_ENVIRONMENT_ID);
    assert_eq!(crc32_ieee(&GOLDEN[..72]), 0xe86a_6f11);
}

#[test]
fn framing_identity_and_checksum_fail_closed() {
    assert_eq!(
        parse_scenario_image(&GOLDEN[..75]),
        Err(ScenarioError::Length)
    );

    let mut bytes = *GOLDEN;
    bytes[0] = b'X';
    assert_eq!(parse_scenario_image(&bytes), Err(ScenarioError::Magic));

    let mut bytes = *GOLDEN;
    write_u16(&mut bytes, 4, 2);
    assert_eq!(parse_scenario_image(&bytes), Err(ScenarioError::Version));

    let mut bytes = *GOLDEN;
    write_u16(&mut bytes, 6, 75);
    assert_eq!(
        parse_scenario_image(&bytes),
        Err(ScenarioError::RecordLength)
    );

    let mut bytes = *GOLDEN;
    bytes[32] ^= 1;
    assert_eq!(parse_scenario_image(&bytes), Err(ScenarioError::Checksum));

    let mut bytes = *GOLDEN;
    write_u32(&mut bytes, 8, NUMERIC_CONTRACT_ID ^ 1);
    repair_crc(&mut bytes);
    assert_eq!(
        parse_scenario_image(&bytes),
        Err(ScenarioError::NumericContract)
    );

    let mut bytes = *GOLDEN;
    write_u32(&mut bytes, 68, SIMPLE_EARTH_ENVIRONMENT_ID ^ 1);
    repair_crc(&mut bytes);
    assert_eq!(
        parse_scenario_image(&bytes),
        Err(ScenarioError::Environment)
    );
}

#[test]
fn field_ranges_and_cross_field_invariants_are_enforced() {
    let cases = [
        (
            16usize,
            0i32,
            ScenarioError::FieldRange(ScenarioField::Timestep),
        ),
        (20, 0, ScenarioError::FieldRange(ScenarioField::Steps)),
        (
            24,
            0,
            ScenarioError::FieldRange(ScenarioField::TelemetryStride),
        ),
        (
            32,
            8_192_001,
            ScenarioError::FieldRange(ScenarioField::Altitude),
        ),
        (
            36,
            134_217_729,
            ScenarioError::FieldRange(ScenarioField::Velocity),
        ),
        (40, 0, ScenarioError::FieldRange(ScenarioField::TotalMass)),
        (44, -1, ScenarioError::FieldRange(ScenarioField::Propellant)),
        (48, 0, ScenarioError::FieldRange(ScenarioField::DryMass)),
        (52, -1, ScenarioError::FieldRange(ScenarioField::Thrust)),
        (56, -1, ScenarioError::FieldRange(ScenarioField::MassFlow)),
        (
            60,
            -1,
            ScenarioError::FieldRange(ScenarioField::BurnDuration),
        ),
        (64, -1, ScenarioError::FieldRange(ScenarioField::Cda)),
    ];
    for (offset, value, expected) in cases {
        let mut bytes = *GOLDEN;
        write_i32(&mut bytes, offset, value);
        repair_crc(&mut bytes);
        assert_eq!(parse_scenario_image(&bytes), Err(expected));
    }

    let mut bytes = *GOLDEN;
    write_i32(&mut bytes, 40, 100);
    repair_crc(&mut bytes);
    assert_eq!(
        parse_scenario_image(&bytes),
        Err(ScenarioError::MassInvariant)
    );

    let mut bytes = *GOLDEN;
    write_i32(&mut bytes, 60, 20_000_000);
    repair_crc(&mut bytes);
    assert_eq!(parse_scenario_image(&bytes), Err(ScenarioError::Duration));

    let mut bytes = *GOLDEN;
    write_i32(&mut bytes, 48, 4_096);
    write_i32(&mut bytes, 52, 4_096);
    repair_crc(&mut bytes);
    assert_eq!(
        parse_scenario_image(&bytes),
        Err(ScenarioError::AccelerationEnvelope)
    );
}

#[test]
fn reserved_flags_are_preserved_for_forward_compatibility() {
    let mut bytes = *GOLDEN;
    write_u16(&mut bytes, 26, 0xa55a);
    repair_crc(&mut bytes);
    assert_eq!(parse_scenario_image(&bytes).unwrap().flags(), 0xa55a);
}
