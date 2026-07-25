use ksa64_core::phase8_format::{KMC8_LENGTH, KMP8_LENGTH, KSC8_LENGTH, KVP8_LENGTH, KWP8_LENGTH};
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack,
};
use ksa64_host::phase8_campaign::{encode_kra8, run_spatial_campaign};
use ksa64_sim::phase8_campaign::{encode_ksc8, SpatialCampaignConfig, SPATIAL_REFERENCE_SEED};
use std::{env, fs, path::PathBuf, time::Instant};
fn fixed<const N: usize>(path: PathBuf) -> [u8; N] {
    fs::read(path)
        .expect("read pack")
        .try_into()
        .unwrap_or_else(|b: Vec<u8>| panic!("expected {N}, got {}", b.len()))
}
fn main() {
    let mut a = env::args_os().skip(1);
    let packs = PathBuf::from(a.next().unwrap_or_else(|| {
        eprintln!("usage: phase8_campaign PACK_DIR OUTPUT_DIR [RUNS] [WORKERS]");
        std::process::exit(2)
    }));
    let output = PathBuf::from(a.next().expect("output directory"));
    let run_count: u32 = a
        .next()
        .map(|v| v.to_string_lossy().parse().expect("runs"))
        .unwrap_or(64);
    let workers: usize = a
        .next()
        .map(|v| v.to_string_lossy().parse().expect("workers"))
        .unwrap_or(4);
    let vehicle =
        parse_spatial_vehicle_pack(&fixed::<KVP8_LENGTH>(packs.join("firestorm54.kvp8"))).unwrap();
    let motor =
        parse_spatial_motor_pack(&fixed::<KMP8_LENGTH>(packs.join("aerotech-i211w.kmp8"))).unwrap();
    let mission =
        parse_spatial_mission_pack(&fixed::<KMC8_LENGTH>(packs.join("firestorm-i211.kmc8")))
            .unwrap();
    let wind =
        parse_wind_profile_pack(&fixed::<KWP8_LENGTH>(packs.join("firestorm-calm.kwp8"))).unwrap();
    let config = SpatialCampaignConfig {
        master_seed: SPATIAL_REFERENCE_SEED,
        run_count,
    };
    let started = Instant::now();
    let campaign = run_spatial_campaign(vehicle, motor, mission, wind, config, workers);
    let mut ksc = [0u8; KSC8_LENGTH];
    encode_ksc8(config, &mut ksc).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join(format!("campaign-{run_count}.ksc8")), ksc).unwrap();
    fs::write(
        output.join(format!("campaign-{run_count}.kra8")),
        encode_kra8(&campaign),
    )
    .unwrap();
    println!(
        "runs={run_count} workers={workers} elapsed_ms={} aggregate={:?}",
        started.elapsed().as_millis(),
        campaign.aggregate
    )
}
