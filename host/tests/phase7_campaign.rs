use ksa64_core::evaluation::MetricSlot;
use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KSC7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use ksa64_host::phase7_campaign::{
    encode_candidate_list, encode_kra7, run_hobby_campaign, summary_for_record,
    validate_candidate_list, validate_kra7,
};
use ksa64_sim::phase7_campaign::{
    derive_hobby_uncertainty, encode_ksc7, parse_ksc7, HobbyCampaignConfig, HobbyDesignVector,
    HOBBY_REFERENCE_SEED,
};

const VEHICLE_BYTES: &[u8; KVP7_LENGTH] = include_bytes!("../../phase7/examples/firestorm54.kvp7");
const MOTOR_BYTES: &[u8; KMP7_LENGTH] = include_bytes!("../../phase7/examples/aerotech-i211w.kmp7");
const MISSION_BYTES: &[u8; KMC7_LENGTH] =
    include_bytes!("../../phase7/examples/firestorm-i211.kmc7");
const ROUTINE_CONFIG: &[u8] = include_bytes!("../../phase7/examples/campaign-64.ksc7");
const ROUTINE_ARCHIVE: &[u8] = include_bytes!("../../phase7/examples/campaign-64.kra7");

fn packs() -> (
    ksa64_core::phase7_pack::VerticalVehiclePack,
    ksa64_core::phase7_pack::MotorPack,
    ksa64_core::phase7_pack::HobbyMissionPack,
) {
    (
        parse_vehicle_pack(VEHICLE_BYTES).unwrap(),
        parse_motor_pack(MOTOR_BYTES).unwrap(),
        parse_mission_pack(MISSION_BYTES).unwrap(),
    )
}

#[test]
fn run_zero_is_nominal_and_keyed_draws_are_repeatable() {
    let config = HobbyCampaignConfig::ROUTINE;
    let zero = derive_hobby_uncertainty(config, 0);
    assert_eq!(zero.values, [0; 8]);
    assert_eq!(zero.checksum, 0);
    assert_eq!(
        derive_hobby_uncertainty(config, 37),
        derive_hobby_uncertainty(config, 37)
    );
    assert_ne!(
        derive_hobby_uncertainty(config, 36),
        derive_hobby_uncertainty(config, 37)
    );
}

#[test]
fn campaign_is_identical_across_worker_counts() {
    let (vehicle, motor, mission) = packs();
    let serial = run_hobby_campaign(
        vehicle,
        motor,
        mission,
        HobbyDesignVector::NOMINAL,
        HobbyCampaignConfig::ROUTINE,
        1,
    );
    let parallel = run_hobby_campaign(
        vehicle,
        motor,
        mission,
        HobbyDesignVector::NOMINAL,
        HobbyCampaignConfig::ROUTINE,
        4,
    );
    assert_eq!(serial.records, parallel.records);
    assert_eq!(serial.aggregate, parallel.aggregate);
    assert_eq!(serial.aggregate.runs, 64);
    assert_eq!(serial.aggregate.successful_recoveries, 64);
    let baseline = summary_for_record(&serial.records[0]);
    assert_eq!(baseline.source_checksums[0], 0xa61c_5720);
    assert_eq!(baseline.metric(MetricSlot::ApogeeAltitude), Some(8_012_317));
    let archive = encode_kra7(&serial);
    assert!(validate_kra7(&archive));
    assert_eq!(archive.as_slice(), ROUTINE_ARCHIVE);
    assert_eq!(
        parse_ksc7(ROUTINE_CONFIG).unwrap(),
        HobbyCampaignConfig::ROUTINE
    );
    let mut corrupt = archive;
    corrupt[100] ^= 1;
    assert!(!validate_kra7(&corrupt));
}

#[test]
fn campaign_and_candidate_formats_round_trip_strictly() {
    let mut campaign_bytes = [0u8; KSC7_LENGTH];
    encode_ksc7(HobbyCampaignConfig::ROUTINE, &mut campaign_bytes).unwrap();
    assert_eq!(
        parse_ksc7(&campaign_bytes).unwrap(),
        HobbyCampaignConfig::ROUTINE
    );
    campaign_bytes[200] = 1;
    ksa64_core::phase7_format::seal_phase7_record(&mut campaign_bytes).unwrap();
    assert!(parse_ksc7(&campaign_bytes).is_err());

    let (vehicle, _, mission) = packs();
    let candidates = [
        HobbyDesignVector::NOMINAL,
        HobbyDesignVector {
            dry_mass_scale_ppm: 950_000,
            body_drag_scale_ppm: 900_000,
            main_deployment_altitude_raw: 150 << 13,
            rail_length_raw: 3 << 13,
        },
    ];
    let bytes = encode_candidate_list(vehicle.identity, mission.identity, &candidates);
    assert!(validate_candidate_list(&bytes));
    let mut corrupt = bytes;
    corrupt[80] ^= 1;
    assert!(!validate_candidate_list(&corrupt));
    assert_eq!(HOBBY_REFERENCE_SEED, 0x4b53_4137);
}
