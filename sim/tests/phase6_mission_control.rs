use ksa64_sim::phase6_link::{run_exact_nominal, run_exact_nominal_observed};
use ksa64_sim::phase6_mission_control::{
    GroundEstimator, GroundTrackingNetwork, PassiveMissionControl, TrackingConfig,
};

#[test]
fn passive_mission_control_cannot_perturb_exact_flight() {
    let baseline = run_exact_nominal().unwrap();
    let mut mission_control = PassiveMissionControl::new();
    let observed = run_exact_nominal_observed(&mut mission_control).unwrap();
    assert_eq!(observed, baseline);
    assert_eq!(mission_control.frames(), baseline.steps * 2);
    assert_eq!(mission_control.world_frames(), baseline.steps);
    assert_eq!(mission_control.flight_frames(), baseline.steps);
    assert_eq!(mission_control.alarms(), 0);
    assert_ne!(mission_control.checksum(), 2_166_136_261);
}

#[test]
fn delayed_noisy_ground_tracking_is_deterministic_and_bounded() {
    let config = TrackingConfig {
        cadence_epochs: 8,
        delay_epochs: 3,
        network_id: 4,
    };
    let mut first = GroundTrackingNetwork::new(0x4752_4e44, config);
    let mut second = GroundTrackingNetwork::new(0x4752_4e44, config);
    let mut estimator = GroundEstimator::new();
    let initial = [1_000_000, -2_000_000, 3_000_000];
    let velocity = [16_384_000, -8_192_000, 4_096_000];
    let per_epoch = [velocity[0] >> 15, velocity[1] >> 15, velocity[2] >> 15];
    let mut accepted = 0;
    for epoch in 0..200u32 {
        let p = [
            initial[0] + per_epoch[0] * epoch as i32,
            initial[1] + per_epoch[1] * epoch as i32,
            initial[2] + per_epoch[2] * epoch as i32,
        ];
        first.observe(epoch, p, velocity);
        second.observe(epoch, p, velocity);
        let a = first.poll(epoch);
        let b = second.poll(epoch);
        assert_eq!(a, b);
        if let Some(fix) = a {
            let estimate = estimator.accept(epoch, fix).unwrap();
            accepted += 1;
            let truth = [
                initial[0] + per_epoch[0] * estimate.epoch as i32,
                initial[1] + per_epoch[1] * estimate.epoch as i32,
                initial[2] + per_epoch[2] * estimate.epoch as i32,
            ];
            for axis in 0..3 {
                assert!((estimate.position_q12[axis] - truth[axis]).abs() < 300_000);
                assert!((estimate.velocity_q24[axis] - velocity[axis]).abs() < 9_000_000);
            }
        }
    }
    assert_eq!(accepted, 25);
    assert_eq!(estimator.estimate().unwrap().fixes, 25);
}
