use ksa64_host::phase5::{capture_phase5_mission, inspect_phase5_stream};
use ksa64_sim::phase5_mission::Phase5MissionCase;
use std::{env, fs, path::PathBuf};
fn main() {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/phase5-nominal.kst5"));
    let (summary, bytes) = capture_phase5_mission(Phase5MissionCase::Nominal);
    let inspection = inspect_phase5_stream(&bytes).expect("strict KST5 inspection");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).expect("create output directory")
    }
    fs::write(&output, &bytes).expect("write KST5");
    println!(
        "KST5 frames={} bytes={} crc32=0x{:08x} observation=0x{:08x} summary=0x{:08x}",
        inspection.frame_count,
        inspection.stream_bytes,
        inspection.stream_crc32,
        inspection.final_frame.observation_checksum,
        summary.summary_checksum
    );
}
