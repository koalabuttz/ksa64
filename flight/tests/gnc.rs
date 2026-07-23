use ksa64_flight::gnc::*;
use ksa64_interface::*;

fn sensor(
    sequence: u32,
    stage: u8,
    phase: StagePhase,
    engine_on: bool,
    steering: u16,
) -> SensorFrame {
    SensorFrame {
        sequence,
        onboard_time_q16: (sequence as i32) * 8_192,
        accel_radial_q28: 2_628_000,
        accel_tangential_q28: 0,
        gyro_rate_q24: 0,
        steering_pitch: steering,
        validity: SENSOR_VALID_ACCEL
            | SENSOR_VALID_GYRO
            | SENSOR_VALID_STEERING
            | SENSOR_VALID_CLOCK,
        altitude_q12: 0,
        gps_radius_q12: 0,
        gps_downrange_q32: 0,
        gps_radial_velocity_q24: 0,
        gps_tangential_velocity_q24: 0,
        events: 0,
        active_stage: stage,
        stage_phase: phase,
        engine_on,
    }
}
fn phase_at(step: u32) -> (u8, StagePhase, bool) {
    match step {
        0 => (0, StagePhase::CoastBeforeIgnition, false),
        1..=1239 => (0, StagePhase::Burning, true),
        1240..=1247 => (0, StagePhase::CoastBeforeSeparation, false),
        1248..=1251 => (1, StagePhase::CoastBeforeIgnition, false),
        _ => (1, StagePhase::Burning, true),
    }
}

#[test]
fn flight_owns_ignition_cutoff_separation_and_handover() {
    let mut flight = FlightComputer::new();
    let mut steering = 0;
    let mut cutoff = false;
    let mut separation = false;
    let mut upper_ignition = false;
    let mut insertion = false;
    for step in 0..=1270 {
        let (stage, phase, engine) = phase_at(step);
        let output = flight.step(&sensor(step, stage, phase, engine, steering));
        steering = output.command.desired_pitch;
        assert_eq!(output.sequence, step);
        if step == 0 {
            assert_eq!(output.command.engine_action, EngineAction::Ignite)
        }
        if step == 1239 {
            cutoff = output.command.engine_action == EngineAction::Cutoff
        }
        if step == 1240 {
            separation = output.command.separate
        }
        if step == 1248 {
            upper_ignition = output.command.engine_action == EngineAction::Ignite
        }
        if step == 1268 {
            insertion = output.mode == FlightMode::Insertion
        }
    }
    assert!(cutoff && separation && upper_ignition && insertion);
    assert!(!flight.status().abort_latched)
}

#[test]
fn persistent_steering_error_latches_safe_abort_and_never_reignites() {
    let mut flight = FlightComputer::new();
    let mut steering = 0;
    let mut abort_step = None;
    for step in 0..=1300 {
        let (stage, phase, engine) = phase_at(step);
        if step >= 1270 {
            steering = 0
        }
        let output = flight.step(&sensor(step, stage, phase, engine, steering));
        if step < 1270 {
            steering = output.command.desired_pitch
        }
        if output.mode == FlightMode::Abort {
            abort_step.get_or_insert(step);
            assert_eq!(output.command.engine_action, EngineAction::Cutoff);
            assert!(output.command.abort_safeing);
            assert!(output.command.recovery_requested);
            assert!(!output.command.separate)
        }
    }
    assert!(abort_step.is_some());
    assert!(abort_step.unwrap() <= 1286);
    assert!(flight.status().abort_latched)
}

#[test]
fn corrupt_sensor_transport_fails_closed() {
    let frame = sensor(0, 0, StagePhase::CoastBeforeIgnition, false, 0);
    let mut bytes = [0u8; SENSOR_FRAME_LENGTH];
    write_sensor_frame(&frame, &mut bytes).unwrap();
    bytes[8] ^= 1;
    let mut flight = FlightComputer::new();
    let output = flight.step_serialized(&bytes);
    assert_eq!(output.mode, FlightMode::Abort);
    assert_eq!(output.command.engine_action, EngineAction::Cutoff);
    assert!(output.command.abort_safeing);
    assert!(output.alarms & ALARM_SENSOR_FRAME != 0)
}
