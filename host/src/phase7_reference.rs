//! Independent float64 analysis for Phase 7 vertical mission evidence.

use ksa64_core::phase7_numeric::*;
use ksa64_core::phase7_pack::{HobbyMissionPack, MotorPack, VerticalVehiclePack};

const EARTH_RADIUS_M: f64 = 6_371_000.0;
const G0: f64 = 9.80665;
const R_AIR: f64 = 287.05287;
const GAMMA: f64 = 1.4;
const LAYER_BASES: [f64; 8] = [
    0.0, 11_000.0, 20_000.0, 32_000.0, 47_000.0, 51_000.0, 71_000.0, 84_852.0,
];
const LAPSE: [f64; 8] = [-0.0065, 0.0, 0.001, 0.0028, 0.0, -0.0028, -0.002, 0.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HobbyReferenceResult {
    pub steps: u32,
    pub apogee_m: f64,
    pub max_speed_mps: f64,
    pub max_acceleration_mps2: f64,
    pub max_dynamic_pressure_pa: f64,
    pub max_mach: f64,
    pub impact_velocity_mps: f64,
    pub ground_time_s: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Rail,
    Powered,
    Coast,
    DrogueInflating,
    Drogue,
    MainInflating,
    Main,
}

fn raw(raw: i32, bits: u8) -> f64 {
    raw as f64 / (1u64 << bits) as f64
}

fn atmosphere(geometric_m: f64) -> (f64, Option<f64>) {
    if geometric_m > 86_000.0 {
        return (0.0, None);
    }
    let h = EARTH_RADIUS_M * geometric_m.max(0.0) / (EARTH_RADIUS_M + geometric_m.max(0.0));
    let mut temperatures = [0.0; 8];
    let mut pressures = [0.0; 8];
    temperatures[0] = 288.15;
    pressures[0] = 101_325.0;
    for index in 0..7 {
        let delta = LAYER_BASES[index + 1] - LAYER_BASES[index];
        temperatures[index + 1] = temperatures[index] + LAPSE[index] * delta;
        pressures[index + 1] = if LAPSE[index] == 0.0 {
            pressures[index] * (-G0 * delta / (R_AIR * temperatures[index])).exp()
        } else {
            pressures[index]
                * (temperatures[index] / temperatures[index + 1]).powf(G0 / (R_AIR * LAPSE[index]))
        };
    }
    let mut layer = 7;
    for index in 0..7 {
        if h < LAYER_BASES[index + 1] {
            layer = index;
            break;
        }
    }
    let delta = h - LAYER_BASES[layer];
    let temperature = temperatures[layer] + LAPSE[layer] * delta;
    let pressure = if LAPSE[layer] == 0.0 {
        pressures[layer] * (-G0 * delta / (R_AIR * temperatures[layer])).exp()
    } else {
        pressures[layer] * (temperatures[layer] / temperature).powf(G0 / (R_AIR * LAPSE[layer]))
    };
    (
        pressure / (R_AIR * temperature),
        Some((GAMMA * R_AIR * temperature).sqrt()),
    )
}

fn thrust(motor: &MotorPack, time: f64) -> f64 {
    let burn = raw(motor.burn_time.raw(), HOBBY_TIME_FRACTIONAL_BITS);
    if !(0.0..=burn).contains(&time) {
        return 0.0;
    }
    let mut index = 1usize;
    while index < motor.knot_count as usize
        && time > raw(motor.knots[index].time.raw(), HOBBY_TIME_FRACTIONAL_BITS)
    {
        index += 1;
    }
    if index >= motor.knot_count as usize {
        return 0.0;
    }
    let left = motor.knots[index - 1];
    let right = motor.knots[index];
    let left_time = raw(left.time.raw(), HOBBY_TIME_FRACTIONAL_BITS);
    let right_time = raw(right.time.raw(), HOBBY_TIME_FRACTIONAL_BITS);
    let fraction = (time - left_time) / (right_time - left_time);
    let left_thrust = raw(left.thrust_raw_q13, HOBBY_FORCE_FRACTIONAL_BITS);
    let right_thrust = raw(right.thrust_raw_q13, HOBBY_FORCE_FRACTIONAL_BITS);
    left_thrust + (right_thrust - left_thrust) * fraction
}

pub fn analyze_hobby_mission_f64(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
) -> HobbyReferenceResult {
    let dry = raw(vehicle.dry_mass.raw(), HOBBY_MASS_FRACTIONAL_BITS);
    let loaded = raw(motor.loaded_mass.raw(), HOBBY_MASS_FRACTIONAL_BITS);
    let propellant_initial = raw(motor.propellant_mass.raw(), HOBBY_MASS_FRACTIONAL_BITS);
    let impulse_total = raw(motor.total_impulse_raw_q16, 16);
    let body_cda = raw(vehicle.reference_area.raw(), HOBBY_AREA_FRACTIONAL_BITS)
        * raw(vehicle.body_cd_q16, 16);
    let drogue_cda = raw(vehicle.drogue_cda.raw(), HOBBY_RECOVERY_CDA_FRACTIONAL_BITS);
    let main_cda = raw(vehicle.main_cda.raw(), HOBBY_RECOVERY_CDA_FRACTIONAL_BITS);
    let launch = raw(
        mission.launch_altitude.raw(),
        HOBBY_ALTITUDE_FRACTIONAL_BITS,
    );
    let rail_top = launch + raw(mission.rail_length.raw(), HOBBY_ALTITUDE_FRACTIONAL_BITS);
    let main_threshold = launch
        + raw(
            mission.main_deployment_altitude.raw(),
            HOBBY_ALTITUDE_FRACTIONAL_BITS,
        );
    let drogue_inflation = raw(
        mission.drogue_inflation_time.raw(),
        HOBBY_TIME_FRACTIONAL_BITS,
    );
    let main_inflation = raw(
        mission.main_inflation_time.raw(),
        HOBBY_TIME_FRACTIONAL_BITS,
    );
    let burn = raw(motor.burn_time.raw(), HOBBY_TIME_FRACTIONAL_BITS);
    let mut phase = Phase::Rail;
    let mut phase_start = 0.0;
    let mut time = 0.0;
    let mut altitude = launch;
    let mut velocity: f64 = 0.0;
    let mut impulse = 0.0;
    let mut steps = 0u32;
    let mut apogee = altitude;
    let mut max_speed: f64 = 0.0;
    let mut max_acceleration: f64 = 0.0;
    let mut max_q: f64 = 0.0;
    let mut max_mach: f64 = 0.0;
    let mut impact = 0.0;
    while steps < 200_000 && time < 900.0 {
        let dt = match phase {
            Phase::Rail | Phase::Powered => 0.01,
            Phase::Coast => 0.02,
            _ => 0.05,
        };
        let thrust = if matches!(phase, Phase::Rail | Phase::Powered) {
            thrust(motor, time)
        } else {
            0.0
        };
        let propellant = (propellant_initial * (1.0 - impulse / impulse_total)).max(0.0);
        let mass = dry + loaded - propellant_initial + propellant;
        let (density, sound) = atmosphere(altitude);
        let q = 0.5 * density * velocity * velocity;
        let cda = match phase {
            Phase::Rail | Phase::Powered | Phase::Coast => body_cda,
            Phase::DrogueInflating => {
                body_cda
                    + (drogue_cda - body_cda)
                        * ((time - phase_start) / drogue_inflation).clamp(0.0, 1.0)
            }
            Phase::Drogue => drogue_cda,
            Phase::MainInflating => {
                drogue_cda
                    + (main_cda - drogue_cda)
                        * ((time - phase_start) / main_inflation).clamp(0.0, 1.0)
            }
            Phase::Main => main_cda,
        };
        let drag = q * cda * velocity.signum();
        let gravity = G0 * (EARTH_RADIUS_M / (EARTH_RADIUS_M + altitude.max(0.0))).powi(2);
        let acceleration = (thrust - drag) / mass - gravity;
        let mut successor_velocity = velocity + acceleration * dt;
        if phase == Phase::Rail && velocity == 0.0 && successor_velocity < 0.0 {
            successor_velocity = 0.0;
        }
        let successor_altitude = altitude + successor_velocity * dt;
        impulse = (impulse + thrust * dt).min(impulse_total);
        time += dt;
        steps += 1;
        if phase == Phase::Rail && successor_altitude >= rail_top {
            phase = Phase::Powered;
            phase_start = time;
        }
        if matches!(phase, Phase::Rail | Phase::Powered) && time >= burn {
            phase = Phase::Coast;
            phase_start = time;
        }
        if phase == Phase::Coast && velocity > 0.0 && successor_velocity <= 0.0 {
            phase = Phase::DrogueInflating;
            phase_start = time;
        }
        if phase == Phase::DrogueInflating && time - phase_start >= drogue_inflation {
            phase = Phase::Drogue;
            phase_start = time;
        }
        if matches!(phase, Phase::DrogueInflating | Phase::Drogue)
            && successor_velocity < 0.0
            && successor_altitude <= main_threshold
        {
            phase = Phase::MainInflating;
            phase_start = time;
        }
        if phase == Phase::MainInflating && time - phase_start >= main_inflation {
            phase = Phase::Main;
            phase_start = time;
        }
        apogee = apogee.max(successor_altitude);
        max_speed = max_speed.max(successor_velocity.abs());
        max_acceleration = max_acceleration.max(acceleration.abs());
        max_q = max_q.max(q);
        if let Some(sound) = sound {
            max_mach = max_mach.max(velocity.abs() / sound);
        }
        velocity = successor_velocity;
        altitude = successor_altitude;
        if velocity < 0.0 && altitude <= launch {
            impact = velocity;
            break;
        }
    }
    HobbyReferenceResult {
        steps,
        apogee_m: apogee,
        max_speed_mps: max_speed,
        max_acceleration_mps2: max_acceleration,
        max_dynamic_pressure_pa: max_q,
        max_mach,
        impact_velocity_mps: impact,
        ground_time_s: time,
    }
}
