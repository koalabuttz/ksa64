use ksa64_interface::crc32_ieee;
use ksa64_sim::phase4::campaign::{derive_run, reviewed_campaign_config};
use ksa64_sim::phase4::contracts::{DETAIL_HEADER_LENGTH, REFERENCE_RUNS};
use ksa64_sim::phase4::detail::{parse_kst4, write_kst4, DetailError};
use ksa64_sim::telemetry::PHASE3_TELEMETRY_HEADER_LENGTH;

const NOMINAL_KST3: &[u8] = include_bytes!("../../phase3/examples/ksa3-nominal.kst3");

#[test]
fn kst4_binds_the_exact_phase3_frames_to_campaign_run_and_variation() {
    let run = derive_run(&reviewed_campaign_config(REFERENCE_RUNS), 0).unwrap();
    let frames = &NOMINAL_KST3[PHASE3_TELEMETRY_HEADER_LENGTH..];
    let mut bytes = vec![0u8; DETAIL_HEADER_LENGTH + frames.len()];
    let written = write_kst4(
        0xa2e9_e9d5,
        run.index,
        run.sensor_seed,
        run.variation.checksum(),
        frames,
        &mut bytes,
    )
    .unwrap();
    let parsed = parse_kst4(&bytes).unwrap();
    assert_eq!(parsed.header, written);
    assert_eq!(parsed.frames, frames);
    assert_eq!(parsed.header.frame_count, 906);
    assert_eq!(parsed.header.first_step, 0);
    assert_eq!(parsed.header.final_step, 7_200);
    assert_eq!(parsed.header.final_truth_checksum, 0xc860_45a0);
    assert_eq!(parsed.header.final_sensor_checksum, 0x47d1_1fb0);
    assert_eq!(parsed.header.final_navigation_checksum, 0xc6f9_da7b);
    assert_eq!(parsed.header.final_flight_checksum, 0x02ce_28ef);
    assert_eq!(parsed.frame(0).unwrap().step, 0);
    assert_eq!(parsed.frame(905).unwrap().step, 7_200);
}

#[test]
fn kst4_rejects_corrupt_frames_and_nonzero_reserved_header_bytes() {
    let run = derive_run(&reviewed_campaign_config(REFERENCE_RUNS), 0).unwrap();
    let frames = &NOMINAL_KST3[PHASE3_TELEMETRY_HEADER_LENGTH..];
    let mut bytes = vec![0u8; DETAIL_HEADER_LENGTH + frames.len()];
    write_kst4(
        0xa2e9_e9d5,
        run.index,
        run.sensor_seed,
        run.variation.checksum(),
        frames,
        &mut bytes,
    )
    .unwrap();

    let mut corrupt = bytes.clone();
    corrupt[DETAIL_HEADER_LENGTH + 7] ^= 0x40;
    assert!(matches!(parse_kst4(&corrupt), Err(DetailError::Checksum)));

    let mut reserved = bytes;
    reserved[72] = 1;
    let crc = crc32_ieee(&reserved[..92]);
    reserved[92..96].copy_from_slice(&crc.to_le_bytes());
    assert!(matches!(parse_kst4(&reserved), Err(DetailError::Reserved)));
}
