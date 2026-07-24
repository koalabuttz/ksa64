use std::env;
use std::fs;
use std::path::PathBuf;

use ksa64_core::evaluation::MetricSlot;
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use ksa64_host::phase7::{capture_hobby_mission, telemetry_frame_count};
use ksa64_host::phase7_plot::build_stock_kph7;

fn main() {
    let mut arguments = env::args_os().skip(1);
    let pack_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase7_run PACK_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    let output_directory = PathBuf::from(arguments.next().unwrap_or_else(|| {
        eprintln!("usage: phase7_run PACK_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(2)
    }));
    let vehicle = parse_vehicle_pack(
        &fs::read(pack_directory.join("firestorm54.kvp7")).expect("read vehicle pack"),
    )
    .expect("parse vehicle pack");
    let motor = parse_motor_pack(
        &fs::read(pack_directory.join("aerotech-i211w.kmp7")).expect("read motor pack"),
    )
    .expect("parse motor pack");
    let mission = parse_mission_pack(
        &fs::read(pack_directory.join("firestorm-i211.kmc7")).expect("read mission pack"),
    )
    .expect("parse mission pack");
    let capture = capture_hobby_mission(vehicle, &motor, mission).expect("capture hobby mission");
    fs::create_dir_all(&output_directory).expect("create output directory");
    fs::write(
        output_directory.join("firestorm-i211.kst7"),
        &capture.telemetry,
    )
    .expect("write telemetry");
    fs::write(
        output_directory.join("firestorm-i211.ksr7"),
        capture.summary_record,
    )
    .expect("write summary");
    fs::write(
        output_directory.join("firestorm-i211.kph7"),
        build_stock_kph7(&capture.telemetry).expect("build sparse plot"),
    )
    .expect("write sparse plot");
    println!(
        "outcome={:?} frames={} apogee_raw={} impact_velocity_raw={} checksum={:08x}",
        capture.evaluation.outcome,
        telemetry_frame_count(&capture),
        capture
            .evaluation
            .metric(MetricSlot::ApogeeAltitude)
            .unwrap_or_default(),
        capture
            .evaluation
            .metric(MetricSlot::ImpactVelocity)
            .unwrap_or_default(),
        capture.evaluation.source_checksums[0]
    );
}
