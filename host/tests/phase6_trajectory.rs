use ksa64_host::phase6_trajectory::{
    environment_from_observed_raw, great_circle_downrange, latitude_longitude, orbit_from_state,
    propagate_elliptic, residual_in_plan_frame, sample_orbit, split_antimeridian, PlanReference,
    Vec3, EARTH_MU_KM3_S2, EARTH_RADIUS_KM, PLAN_POINTS, PLAN_STREAM_CRC32,
};

#[test]
fn embedded_plan_and_nominal_orbit_match_frozen_evidence() {
    let plan = PlanReference::load_embedded().expect("validated KPH5 plan");
    assert_eq!(plan.points.len(), PLAN_POINTS);
    assert_eq!(plan.stream_crc32, PLAN_STREAM_CRC32);
    assert!((EARTH_RADIUS_KM - 6378.137).abs() < 0.001);
    assert!((plan.orbit.perigee_altitude_km - 181.44986979238456).abs() < 0.002);
    assert!((plan.orbit.apogee_altitude_km - 207.24562100498224).abs() < 0.002);
    assert!((plan.orbit.eccentricity - 0.0019624047990873415).abs() < 1e-9);
    assert!((plan.orbit.inclination_deg - 51.617502344320265).abs() < 1e-7);
}

#[test]
fn orbit_sampling_and_kepler_propagation_remain_on_the_conic() {
    let plan = PlanReference::load_embedded().unwrap();
    let points = sample_orbit(plan.orbit, 257);
    assert_eq!(points.len(), 257);
    let minimum = points
        .iter()
        .map(|p| p.norm())
        .fold(f64::INFINITY, f64::min);
    let maximum = points.iter().map(|p| p.norm()).fold(0.0, f64::max);
    assert!((minimum - (EARTH_RADIUS_KM + plan.orbit.perigee_altitude_km)).abs() < 0.01);
    assert!((maximum - (EARTH_RADIUS_KM + plan.orbit.apogee_altitude_km)).abs() < 0.01);
    let one_period = propagate_elliptic(plan.orbit, plan.orbit.period_seconds).unwrap();
    assert!((one_period - plan.terminal_position).norm() < 1e-5);
}

#[test]
fn circular_impact_escape_and_degenerate_states_are_classified() {
    let circular_radius = EARTH_RADIUS_KM + 200.0;
    let circular_speed = (EARTH_MU_KM3_S2 / circular_radius).sqrt();
    let circular = orbit_from_state(
        Vec3::new(circular_radius, 0.0, 0.0),
        Vec3::new(0.0, circular_speed, 0.0),
    )
    .unwrap();
    assert!(circular.eccentricity < 1e-12);
    assert!((circular.perigee_altitude_km - 200.0).abs() < 1e-8);
    let impacting = orbit_from_state(
        Vec3::new(EARTH_RADIUS_KM + 20.0, 0.0, 0.0),
        Vec3::new(0.0, 6.0, 0.0),
    )
    .unwrap();
    assert_eq!(format!("{:?}", impacting.kind), "Impacting");
    let escape = orbit_from_state(
        Vec3::new(EARTH_RADIUS_KM + 200.0, 0.0, 0.0),
        Vec3::new(0.0, 12.0, 0.0),
    )
    .unwrap();
    assert_eq!(format!("{:?}", escape.kind), "Escape");
    let radial = orbit_from_state(
        Vec3::new(EARTH_RADIUS_KM + 200.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
    )
    .unwrap();
    assert_eq!(format!("{:?}", radial.kind), "Degenerate");
}

#[test]
fn geography_environment_and_residual_helpers_are_bounded() {
    let plan = PlanReference::load_embedded().unwrap();
    let first = plan.points[0];
    let (latitude, longitude) = latitude_longitude(first.position_eci, 0.0).unwrap();
    assert!((latitude - 28.5).abs() < 0.01);
    assert!(longitude.abs() < 0.01);
    assert!(great_circle_downrange(first.position_eci, 0.0) < 1.0);
    let residual = residual_in_plan_frame(first.position_eci, first.position_eci);
    assert_eq!(residual, Default::default());
    let segments = split_antimeridian(&[(170.0, 1.0), (179.0, 2.0), (-179.0, 3.0), (-170.0, 4.0)]);
    assert_eq!(segments.len(), 2);
    let raw_position = [21468577, 3871182, 15698368];
    let raw_velocity = [-66327286, 89767125, 68337641];
    let environment = environment_from_observed_raw(raw_position, raw_velocity).unwrap();
    assert!(environment.mach > 10.0);
    assert!(environment.dynamic_pressure_kpa < 0.01);
}
