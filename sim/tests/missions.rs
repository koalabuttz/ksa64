use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_core::planar::{evaluate_vacuum, OrbitClass, PlanarWorld};
use ksa64_interface::{EngineAction, FlightMode, SENSOR_VALID_GPS};
use ksa64_sim::mission::*;

const IMAGE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
fn scenario() -> ksa64_core::phase2_scenario::Phase2Scenario {
    parse_phase2_scenario(IMAGE).unwrap()
}
fn altitude_km(raw: i32) -> f64 {
    (raw - EARTH_RADIUS_Q12) as f64 / 4096.0
}
fn velocity_ms(raw: i32) -> f64 {
    raw as f64 / (1u64 << 24) as f64 * 1000.0
}

#[test]
fn nominal_and_recoverable_cases_meet_orbit_load_and_navigation_limits() {
    let scenario = scenario();
    for case in [
        MissionCase::Nominal,
        MissionCase::AltimeterDropout,
        MissionCase::GpsOutage,
    ] {
        let result = run_phase3_mission(&scenario, case).unwrap();
        assert_eq!(result.outcome, MissionOutcome::DurationComplete);
        let orbit = result.orbit.unwrap();
        assert_eq!(orbit.class(), OrbitClass::StableOrbit);
        assert!((180.0..=220.0).contains(&altitude_km(orbit.perigee().raw())));
        assert!((180.0..=220.0).contains(&altitude_km(orbit.apogee().raw())));
        assert!(orbit.eccentricity().raw() <= 655);
        assert!(result.max_dynamic_pressure.raw() <= 60 * 65536);
        assert!(result.max_proper_acceleration.raw() <= ((0.060_f64 * (1u64 << 28) as f64) as i32));
        let nav = result.cutoff_navigation;
        let truth = result.cutoff_truth;
        let mut status = NumericStatus::CLEAR;
        let vt = evaluate_vacuum(
            PlanarWorld::simple_earth(scenario.timestep()),
            truth,
            &mut status,
        )
        .tangential_velocity()
        .raw();
        assert!((truth.radius().raw() - nav.radius_q12).abs() <= 4096);
        assert!(velocity_ms(truth.radial_velocity().raw() - nav.radial_velocity_q24).abs() <= 10.0);
        assert!(velocity_ms(vt - nav.tangential_velocity_q24).abs() <= 10.0);
    }
}

#[derive(Default)]
struct Evidence {
    tracking_sq: f64,
    tracking_count: u32,
    tracking_peak: u16,
    tracking_peak_step: u32,
    outage_position_km: f64,
    outage_velocity_ms: f64,
    abort_seen: bool,
    reignition_after_abort: bool,
}
impl MissionObserver for Evidence {
    type Error = ();
    fn observe(&mut self, r: MissionRecord) -> Result<(), Self::Error> {
        let step = r.world.truth.step();
        if (1268..=3171).contains(&step) {
            let error = r.steering.requested.abs_diff(r.steering.applied);
            if error > self.tracking_peak {
                self.tracking_peak = error;
                self.tracking_peak_step = step;
            }
            self.tracking_sq += (error as f64 * 360.0 / 65536.0).powi(2);
            self.tracking_count += 1
        }
        if (2080..2560).contains(&step) && r.sensors.validity & SENSOR_VALID_GPS == 0 {
            let radius =
                (r.world.truth.radius().raw() - r.flight.nav_radius_q12).abs() as f64 / 4096.0;
            let downrange_raw = r
                .world
                .truth
                .downrange()
                .raw()
                .wrapping_sub(r.flight.nav_downrange_q32)
                .abs() as f64;
            let downrange =
                downrange_raw / (u32::MAX as f64 + 1.0) * core::f64::consts::TAU * 6378.137;
            self.outage_position_km = self
                .outage_position_km
                .max((radius * radius + downrange * downrange).sqrt());
            let mut status = NumericStatus::CLEAR;
            let vt = evaluate_vacuum(
                PlanarWorld::simple_earth(ksa64_core::quantities::Time::from_raw(8192)),
                r.world.truth,
                &mut status,
            )
            .tangential_velocity()
            .raw();
            self.outage_velocity_ms = self
                .outage_velocity_ms
                .max(
                    velocity_ms(
                        r.world.truth.radial_velocity().raw() - r.flight.nav_radial_velocity_q24,
                    )
                    .abs(),
                )
                .max(velocity_ms(vt - r.flight.nav_tangential_velocity_q24).abs())
        }
        if r.flight.mode == FlightMode::Abort {
            self.abort_seen = true
        } else if self.abort_seen && r.flight.command.engine_action == EngineAction::Ignite {
            self.reignition_after_abort = true
        }
        Ok(())
    }
}

#[test]
fn nominal_tracking_and_gps_outage_bridge_are_bounded() {
    let scenario = scenario();
    let mut nominal = Evidence::default();
    run_phase3_mission_observed(&scenario, MissionCase::Nominal, &mut nominal).unwrap();
    let rms = (nominal.tracking_sq / nominal.tracking_count as f64).sqrt();
    assert!(rms <= 0.5, "rms={rms}");
    assert!(
        nominal.tracking_peak <= 364,
        "peak={} step={}",
        nominal.tracking_peak,
        nominal.tracking_peak_step
    );
    let mut outage = Evidence::default();
    run_phase3_mission_observed(&scenario, MissionCase::GpsOutage, &mut outage).unwrap();
    assert!(
        outage.outage_position_km <= 5.0,
        "position={}",
        outage.outage_position_km
    );
    assert!(
        outage.outage_velocity_ms <= 30.0,
        "velocity={}",
        outage.outage_velocity_ms
    );
}

#[test]
fn stuck_steering_aborts_and_safes_on_schedule() {
    let scenario = scenario();
    let mut evidence = Evidence::default();
    let result =
        run_phase3_mission_observed(&scenario, MissionCase::SteeringStuck, &mut evidence).unwrap();
    assert_eq!(result.outcome, MissionOutcome::Abort);
    assert!((2080..=2096).contains(&result.abort_step));
    assert!(result.cutoff_step <= result.abort_step + 4);
    assert!(result.recovery_requested);
    assert!(result.flight_status.abort_latched);
    assert!(!evidence.reignition_after_abort);
}

#[test]
fn each_case_is_checksum_deterministic() {
    let scenario = scenario();
    for case in [
        MissionCase::Nominal,
        MissionCase::AltimeterDropout,
        MissionCase::GpsOutage,
        MissionCase::SteeringStuck,
    ] {
        let a = run_phase3_mission(&scenario, case).unwrap();
        let b = run_phase3_mission(&scenario, case).unwrap();
        assert_eq!(
            (
                a.truth_checksum,
                a.sensor_checksum,
                a.nav_checksum,
                a.flight_checksum
            ),
            (
                b.truth_checksum,
                b.sensor_checksum,
                b.nav_checksum,
                b.flight_checksum
            )
        );
    }
}
