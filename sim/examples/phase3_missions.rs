use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_scenario::{parse_phase2_scenario, PHASE2_SCENARIO_IMAGE_LENGTH};
use ksa64_sim::mission::{run_phase3_mission, MissionCase};
const IMAGE: &[u8; PHASE2_SCENARIO_IMAGE_LENGTH] =
    include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");
fn main() {
    let scenario = parse_phase2_scenario(IMAGE).unwrap();
    for case in [
        MissionCase::Nominal,
        MissionCase::AltimeterDropout,
        MissionCase::GpsOutage,
        MissionCase::SteeringStuck,
    ] {
        let r = match run_phase3_mission(&scenario, case) {
            Ok(result) => result,
            Err(error) => {
                println!("{case:?}: ERROR {error:?}");
                continue;
            }
        };
        let orbit = r.orbit.unwrap();
        println!("{case:?}: outcome={:?} step={} alt={:.3}km perigee={:.3}km apogee={:.3}km ecc={:.6} maxq={:.3}kPa maxa={:.3}m/s2 cutoff={} abort={} truth={:08x} sensor={:08x} nav={:08x} flight={:08x}",r.outcome,r.truth.step(),(r.truth.radius().raw()-EARTH_RADIUS_Q12) as f64/4096.0,(orbit.perigee().raw()-EARTH_RADIUS_Q12) as f64/4096.0,(orbit.apogee().raw()-EARTH_RADIUS_Q12) as f64/4096.0,orbit.eccentricity().raw() as f64/65536.0,r.max_dynamic_pressure.raw() as f64/65536.0,r.max_proper_acceleration.raw() as f64/(1u64<<28) as f64*1000.0,r.cutoff_step,r.abort_step,r.truth_checksum,r.sensor_checksum,r.nav_checksum,r.flight_checksum);
    }
}
