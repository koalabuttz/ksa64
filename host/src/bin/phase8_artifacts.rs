use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack,
};
use ksa64_host::phase8_capture::capture_spatial_mission;
use ksa64_host::phase8_plot::build_stock_kph8;
use std::{fs, path::PathBuf};
fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("phase8/examples"));
    let vehicle =
        parse_spatial_vehicle_pack(include_bytes!("../../../phase8/examples/firestorm54.kvp8"))
            .unwrap();
    let motor = parse_spatial_motor_pack(include_bytes!(
        "../../../phase8/examples/aerotech-i211w.kmp8"
    ))
    .unwrap();
    let mission = parse_spatial_mission_pack(include_bytes!(
        "../../../phase8/examples/firestorm-i211.kmc8"
    ))
    .unwrap();
    let wind = parse_wind_profile_pack(include_bytes!(
        "../../../phase8/examples/firestorm-calm.kwp8"
    ))
    .unwrap();
    let capture = capture_spatial_mission(
        &vehicle,
        &motor,
        mission,
        &wind,
        SpatialMissionVariation::NOMINAL,
        0,
    )
    .unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("firestorm-i211.kst8"), &capture.telemetry).unwrap();
    fs::write(output.join("firestorm-i211.ksr8"), capture.summary_record).unwrap();
    fs::write(
        output.join("firestorm-i211.kph8"),
        build_stock_kph8(&capture.telemetry).unwrap(),
    )
    .unwrap();
    println!(
        "frames={} telemetry_bytes={}",
        ksa64_host::phase8_capture::telemetry_frame_count(&capture),
        capture.telemetry.len()
    )
}
