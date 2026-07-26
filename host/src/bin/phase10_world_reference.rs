use ksa64_host::phase10::GlobalFixtureSet;
use ksa64_sim::phase10::{
    GlobalWorldMachine, EVENT_APOGEE, EVENT_BURNOUT, EVENT_DROGUE, EVENT_LANDING, EVENT_MAIN,
    EVENT_RAIL_CLEAR,
};
use serde_json::json;

fn main() -> Result<(), String> {
    let fixtures = GlobalFixtureSet::embedded();
    let mut world = GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut steps = 0u32;
    let mut maximum_mach = 0i32;
    let mut maximum_dynamic_pressure = 0i32;
    let mut transition_samples = Vec::new();
    let mut previous_transition_count = 0u8;
    let mut event_times = [None; 6];
    while !world.is_complete() {
        let snapshot = world.step().map_err(|error| format!("{error:?}"))?;
        steps = steps.saturating_add(1);
        maximum_mach = maximum_mach.max(snapshot.mach_q24);
        maximum_dynamic_pressure = maximum_dynamic_pressure.max(snapshot.dynamic_pressure_q14_pa);
        let time = snapshot.state.time.raw() as f64 / 65_536.0;
        for (index, event) in [
            EVENT_RAIL_CLEAR,
            EVENT_BURNOUT,
            EVENT_APOGEE,
            EVENT_DROGUE,
            EVENT_MAIN,
            EVENT_LANDING,
        ]
        .iter()
        .copied()
        .enumerate()
        {
            if snapshot.events & event != 0 && event_times[index].is_none() {
                event_times[index] = Some(time);
            }
        }
        if snapshot.transition_count != previous_transition_count {
            let attitude = snapshot.state.attitude;
            transition_samples.push(json!({
                "time_s": time,
                "attitude_q30": [
                    attitude.w(),
                    attitude.x(),
                    attitude.y(),
                    attitude.z(),
                ],
                "angular_rate_q24": [
                    snapshot.state.angular_rate.x(),
                    snapshot.state.angular_rate.y(),
                    snapshot.state.angular_rate.z(),
                ],
            }));
            previous_transition_count = snapshot.transition_count;
        }
        if steps > 150_000 {
            return Err("uninstrumented global world timed out".into());
        }
    }
    let terminal = world.snapshot().map_err(|error| format!("{error:?}"))?;
    let ecef = world
        .ecef_state_public()
        .map_err(|error| format!("{error:?}"))?;
    let local = world
        .ecef_to_launch_offset_public(ecef)
        .map_err(|error| format!("{error:?}"))?;
    let transitions = world
        .transitions()
        .iter()
        .take(terminal.transition_count as usize)
        .map(|record| {
            json!({
                "from": format!("{:?}", record.from),
                "to": format!("{:?}", record.to),
                "time_s": record.time.raw() as f64 / 65_536.0,
                "position_delta_raw": record.position_delta_raw,
                "velocity_delta_raw": record.velocity_delta_raw,
                "attitude_delta_raw": record.attitude_delta_raw,
                "angular_rate_delta_raw": record.angular_rate_delta_raw,
            })
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema": "ksa64.phase10.uninstrumented-exact-v1",
            "steps": steps,
            "terminal_time_s": terminal.state.time.raw() as f64 / 65_536.0,
            "apogee_km": terminal.apogee_q12_km as f64 / 4_096.0,
            "downrange_km": local.x() as f64 / 4_096.0,
            "crossrange_km": local.y() as f64 / 4_096.0,
            "maximum_mach": maximum_mach as f64 / 16_777_216.0,
            "maximum_dynamic_pressure_pa": maximum_dynamic_pressure as f64 / 16_384.0,
            "transition_count": terminal.transition_count,
            "transitions": transitions,
            "transition_samples": transition_samples,
            "event_times_s": {
                "rail_clear": event_times[0],
                "burnout": event_times[1],
                "apogee": event_times[2],
                "drogue": event_times[3],
                "main": event_times[4],
                "landing": event_times[5],
            },
            "terminal_ecef_position_km": [
                ecef.position.x() as f64 / 4_096.0,
                ecef.position.y() as f64 / 4_096.0,
                ecef.position.z() as f64 / 4_096.0,
            ],
            "terminal_ecef_velocity_km_s": [
                ecef.velocity.x() as f64 / 16_777_216.0,
                ecef.velocity.y() as f64 / 16_777_216.0,
                ecef.velocity.z() as f64 / 16_777_216.0,
            ],
            "checksum": format!("0x{:08x}", terminal.checksum),
        }))
        .map_err(|error| error.to_string())?
    );
    Ok(())
}
