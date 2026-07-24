use std::env;
use std::fs;
use std::path::PathBuf;

use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KSC7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use ksa64_host::phase7_campaign::{encode_kra7, run_hobby_campaign};
use ksa64_sim::phase7_campaign::{encode_ksc7, HobbyCampaignConfig, HobbyDesignVector};

fn fixed<const N: usize>(path: PathBuf) -> [u8; N] {
    fs::read(path)
        .expect("read pack")
        .try_into()
        .unwrap_or_else(|bytes: Vec<u8>| panic!("expected {N} bytes, got {}", bytes.len()))
}

fn main() {
    let mut arguments = env::args_os().skip(1);
    let pack_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase7_campaign PACK_DIRECTORY OUTPUT_DIRECTORY [RUNS] [WORKERS]");
        std::process::exit(2)
    }));
    let output_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase7_campaign PACK_DIRECTORY OUTPUT_DIRECTORY [RUNS] [WORKERS]");
        std::process::exit(2)
    }));
    let run_count: u32 = arguments
        .next()
        .map(|value| value.to_string_lossy().parse().expect("numeric run count"))
        .unwrap_or(64);
    let workers: usize = arguments
        .next()
        .map(|value| {
            value
                .to_string_lossy()
                .parse()
                .expect("numeric worker count")
        })
        .unwrap_or(4);
    let vehicle = parse_vehicle_pack(&fixed::<KVP7_LENGTH>(
        pack_directory.join("firestorm54.kvp7"),
    ))
    .expect("parse vehicle");
    let motor = parse_motor_pack(&fixed::<KMP7_LENGTH>(
        pack_directory.join("aerotech-i211w.kmp7"),
    ))
    .expect("parse motor");
    let mission = parse_mission_pack(&fixed::<KMC7_LENGTH>(
        pack_directory.join("firestorm-i211.kmc7"),
    ))
    .expect("parse mission");
    let config = HobbyCampaignConfig {
        master_seed: 0x4b53_4137,
        run_count,
    };
    let campaign = run_hobby_campaign(
        vehicle,
        motor,
        mission,
        HobbyDesignVector::NOMINAL,
        config,
        workers,
    );
    let mut config_bytes = [0u8; KSC7_LENGTH];
    encode_ksc7(config, &mut config_bytes).expect("encode KSC7");
    fs::create_dir_all(&output_directory).expect("create output");
    fs::write(
        output_directory.join(format!("campaign-{run_count}.ksc7")),
        config_bytes,
    )
    .expect("write KSC7");
    fs::write(
        output_directory.join(format!("campaign-{run_count}.kra7")),
        encode_kra7(&campaign),
    )
    .expect("write KRA7");
    println!("{:?}", campaign.aggregate);
}
