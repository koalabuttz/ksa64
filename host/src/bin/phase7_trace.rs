use ksa64_core::phase7_format::{KMC7_LENGTH, KMP7_LENGTH, KVP7_LENGTH};
use ksa64_core::phase7_mission::{
    execute_hobby_mission_observed, HobbyMissionExecutionError, HobbyMissionObservation,
    HobbyMissionObserver,
};
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use serde_json::json;

const TRACE_COUNT: usize = 129;
const VEHICLE_BYTES: &[u8; KVP7_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/firestorm54.kvp7"
));
const MOTOR_BYTES: &[u8; KMP7_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/aerotech-i211w.kmp7"
));
const MISSION_BYTES: &[u8; KMC7_LENGTH] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../phase7/examples/firestorm-i211.kmc7"
));

struct TraceObserver {
    checksums: Vec<u32>,
    last: Option<HobbyMissionObservation>,
}

impl HobbyMissionObserver for TraceObserver {
    type Error = ();

    fn observe(&mut self, observation: HobbyMissionObservation) -> Result<(), Self::Error> {
        self.checksums.push(observation.checksum);
        self.last = Some(observation);
        if observation.state.step as usize + 1 == TRACE_COUNT {
            Err(())
        } else {
            Ok(())
        }
    }
}

fn main() {
    let vehicle = parse_vehicle_pack(VEHICLE_BYTES).expect("vehicle pack");
    let motor = parse_motor_pack(MOTOR_BYTES).expect("motor pack");
    let mission = parse_mission_pack(MISSION_BYTES).expect("mission pack");
    let mut trace = TraceObserver {
        checksums: Vec::with_capacity(TRACE_COUNT),
        last: None,
    };
    match execute_hobby_mission_observed(vehicle, &motor, mission, &mut trace) {
        Err(HobbyMissionExecutionError::Observer(())) if trace.checksums.len() == TRACE_COUNT => {}
        other => panic!("unexpected trace result: {other:?}"),
    }
    let observation = trace.last.expect("last observation");
    let output = json!({
        "schema": "ksa64.phase7.host-trace-v1",
        "checksums": trace.checksums,
        "last": {
            "step": observation.state.step,
            "time": observation.state.time.raw(),
            "altitude": observation.state.altitude.raw(),
            "velocity": observation.state.velocity.raw(),
            "acceleration": observation.state.acceleration.raw(),
            "mass": observation.state.mass.raw(),
            "propellant": observation.state.propellant.raw(),
            "impulse": observation.state.impulse_consumed_q16,
            "phase": observation.state.phase as u8,
            "thrust": observation.thrust_raw_q13,
            "dynamic_pressure": observation.dynamic_pressure.raw(),
            "mach": observation.mach.map_or(0, |value| value.raw()),
            "events": observation.events,
            "checksum": observation.checksum,
        }
    });
    println!("{}", serde_json::to_string_pretty(&output).expect("JSON"));
}
