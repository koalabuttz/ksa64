use ksa64_core::phase8_mission::Phase8MissionMachine;
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack,
};
use std::{fs, path::PathBuf};

const TRACE_COUNT: usize = 17;
fn fields(s: ksa64_core::phase8_mission::Phase8MissionSnapshot, checksum: u32) -> [i32; 20] {
    [
        s.state.time.raw(),
        s.state.position.x(),
        s.state.position.y(),
        s.state.position.z(),
        s.state.velocity.x(),
        s.state.velocity.y(),
        s.state.velocity.z(),
        s.state.attitude.w(),
        s.state.attitude.x(),
        s.state.attitude.y(),
        s.state.attitude.z(),
        s.state.angular_rate.x(),
        s.state.angular_rate.y(),
        s.state.angular_rate.z(),
        s.phase as i32,
        s.events as i32,
        s.mass.mass.raw(),
        s.thrust_q13,
        s.aero.dynamic_pressure_q13,
        checksum as i32,
    ]
}
fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("phase8/host-trace-v1.json"));
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
    let mut machine = Phase8MissionMachine::new(&vehicle, &motor, mission, &wind).unwrap();
    let mut checksums = [0u32; TRACE_COUNT];
    let mut checksum = machine.trace_checksum();
    checksums[0] = checksum;
    for slot in checksums.iter_mut().skip(1) {
        machine.step().unwrap();
        checksum = machine.trace_checksum();
        *slot = checksum;
    }
    let last = fields(machine.snapshot(), checksum);
    let checks = checksums
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let values = last
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let text=format!("{{\n  \"schema\": \"ksa64.phase8.exact-trace-v1\",\n  \"count\": {TRACE_COUNT},\n  \"checksums\": [{checks}],\n  \"last\": [{values}]\n}}\n");
    fs::write(output, text).unwrap();
}
