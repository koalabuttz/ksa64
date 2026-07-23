use ksa64_host::phase5_history::capture_phase5_stock_history;
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase5_history::validate_kph5;
use std::{env, fs};
fn main() {
    let output = env::args()
        .nth(1)
        .unwrap_or_else(|| "target/phase5-baseline.kph5".into());
    let (summary, bytes) = capture_phase5_stock_history();
    let header = validate_kph5(&bytes).expect("strict KPH5 validation");
    fs::write(&output, &bytes).expect("write KPH5");
    println!(
        "KPH5 points={} bytes={} crc32=0x{:08x} terminal={} summary=0x{:08x}",
        header.point_count,
        bytes.len(),
        crc32_ieee(&bytes),
        header.terminal_step,
        summary.summary_checksum
    );
}
