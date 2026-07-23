use ksa64_flight::phase5_gnc::SpatialGuidanceTarget;
use ksa64_flight::phase5_navigation::INITIAL_ATTITUDE_Q30;
use ksa64_interface::EngineAction;
use ksa64_sim::phase5_closed_loop::Phase5ClosedLoop;
use ksa64_sim::phase5_sensors::Phase5SensorFaults;

#[test]
fn closed_loop_crosses_both_strict_transports_and_ignites() {
    let mut loopback = Phase5ClosedLoop::new(0x0507_0001, Phase5SensorFaults::default()).unwrap();
    let result = loopback
        .step(SpatialGuidanceTarget::hold(INITIAL_ATTITUDE_Q30))
        .unwrap();
    assert_eq!(result.sensor.sequence, 0);
    assert_eq!(result.flight.command.engine_action, EngineAction::Ignite);
    assert_eq!(result.vehicle.truth.step(), 1);
    assert!(result.vehicle.truth.phase() as u8 == 1);
}

#[test]
fn closed_loop_is_exactly_repeatable() {
    let mut a = Phase5ClosedLoop::new(0x0507_1234, Phase5SensorFaults::default()).unwrap();
    let mut b = Phase5ClosedLoop::new(0x0507_1234, Phase5SensorFaults::default()).unwrap();
    let target = SpatialGuidanceTarget::hold(INITIAL_ATTITUDE_Q30);
    for _ in 0..24 {
        assert_eq!(a.step(target), b.step(target));
    }
    assert_eq!(a.latest(), b.latest());
    assert_eq!(a.flight().navigation(), b.flight().navigation());
}
#[test]
fn frozen_avionics_vectors_match_independent_generator() {
    assert_eq!(ksa64_sim::phase5_avionics_signature(), 0xaa0a_0b0e);
    assert_eq!(ksa64_sim::run_phase5_avionics_self_tests(), 0);
}
