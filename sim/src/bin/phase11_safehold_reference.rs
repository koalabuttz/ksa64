fn main() {
    let value = ksa64_sim::run_safehold_probe();
    println!(
        "{{\"schema\":\"ksa64.phase11.safehold-probe-v1\",\"releases\":{},\"failures\":{},\"flight_checksum\":\"{:08x}\",\"navigation_checksum\":\"{:08x}\",\"command_checksum\":\"{:08x}\",\"journal_chain\":\"{:08x}\",\"drogue_epoch\":{},\"main_epoch\":{},\"transition_count\":{},\"final_frame\":{},\"safe\":{},\"signature\":\"{:08x}\"}}",
        value.releases,
        value.failures,
        value.flight_checksum,
        value.navigation_checksum,
        value.command_checksum,
        value.journal_chain,
        value.drogue_epoch,
        value.main_epoch,
        value.transition_count,
        value.final_frame as u8,
        value.safe,
        ksa64_sim::phase11_safehold_probe_signature(),
    );
}
