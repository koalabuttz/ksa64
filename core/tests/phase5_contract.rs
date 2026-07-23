use ksa64_core::phase5_contract::*;

#[test]
fn frozen_phase5_contract_has_expected_identity_and_cadence() {
    assert_eq!(PHASE5_NUMERIC_CONTRACT, "ksa64.numeric.phase5-v1");
    assert_eq!(PHASE5_NUMERIC_CONTRACT_ID, 1_560_508_115);
    assert_eq!(PHASE5_ENVIRONMENT_ID, 2_082_184_392);
    assert_eq!(PHASE5_SCENARIO_ID, 4_178_197_782);
    assert_eq!(PHASE5_BASE_KSC2_CRC32, 0x5d36_2512);
    assert!(phase5_cadence_is_valid());
    assert_eq!(PHASE5_MISSION_STEP_Q16, 8_192);
    assert_eq!(PHASE5_FAST_STEP_Q16, 2_048);
    assert_eq!(PHASE5_ATTITUDE_SUBSTEPS, 4);
}

#[test]
fn frozen_vehicle_and_campaign_values_are_bounded() {
    assert_eq!(PHASE5_PAYLOAD_MASS_Q12, 12 * 4_096);
    assert_eq!(PHASE5_RCS_PROPELLANT_Q12, 410);
    assert_eq!(PHASE5_CAMPAIGN_SEED, 0x4b53_4135);
    assert_eq!(PHASE5_ROUTINE_RUNS, 32);
    assert_eq!(PHASE5_REFERENCE_RUNS, 256);
    assert_eq!(PHASE5_TARGET_RADIUS_Q12, 26_944_049);
    assert_eq!(PHASE5_MAX_AOA_TURN16, 2_731);
}

#[test]
fn compatibility_evidence_is_frozen_as_one_value() {
    assert_eq!(
        PHASE5_PLANAR_COMPATIBILITY,
        PlanarCompatibilityEvidence {
            phase2_state_checksum: 0xcc57_612b,
            phase3_truth_checksum: 0xc860_45a0,
            phase3_sensor_checksum: 0x47d1_1fb0,
            phase3_navigation_checksum: 0xc6f9_da7b,
            phase3_flight_checksum: 0x02ce_28ef,
            phase3_telemetry_crc32: 0xaf79_b36e,
        }
    );
}
