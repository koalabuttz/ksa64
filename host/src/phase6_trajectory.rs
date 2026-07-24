//! Host-only, noncanonical trajectory products for the Phase 6 presentation layer.
//!
//! Nothing in this module is visible to the world or flight endpoints. It deliberately
//! consumes only recorded/observed state for operational products; simulator truth is a
//! separate caller-selected input used by the SIM Director page.

use ksa64_core::numeric::NumericStatus;
use ksa64_core::phase2_numeric::{EARTH_MU_Q12, EARTH_RADIUS_Q12, EARTH_ROTATION_RAD_Q30};
use ksa64_core::spatial_numeric::{PositionVec, VelocityVec};
use ksa64_core::spatial_world::{evaluate_spatial_environment, SpatialState};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase5_history::{
    parse_kph5_point, validate_kph5, KPH5_HEADER_LENGTH, KPH5_POINT_LENGTH,
};
use serde_json::Value;
use std::f64::consts::{PI, TAU};

pub const EARTH_RADIUS_KM: f64 = EARTH_RADIUS_Q12 as f64 / 4096.0;
pub const EARTH_MU_KM3_S2: f64 = EARTH_MU_Q12 as f64 / 4096.0;
pub const EARTH_ROTATION_RAD_S: f64 = EARTH_ROTATION_RAD_Q30 as f64 / 1_073_741_824.0;
pub const LAUNCH_LATITUDE_DEG: f64 = 28.5;
pub const LAUNCH_LONGITUDE_DEG: f64 = 0.0;
pub const TARGET_ALTITUDE_KM: f64 = 200.0;
pub const TARGET_APSIS_MIN_KM: f64 = 180.0;
pub const TARGET_APSIS_MAX_KM: f64 = 220.0;
pub const TARGET_INCLINATION_DEG: f64 = 51.6;
pub const PLAN_STREAM_CRC32: u32 = 0xf2b3_b81f;
pub const PLAN_POINTS: usize = 99;
pub const PLAN_TERMINAL_STEP: u32 = 3133;
pub const FAST_EPOCH_HZ: f64 = 32.0;
pub const MISSION_STEP_SECONDS: f64 = 0.125;

const PLAN_BYTES: &[u8; 1664] = include_bytes!("../../phase5/examples/ksa5-baseline.kph5");
const MISSION_REFERENCE: &str = include_str!("../../phase5/mission-reference-v1.json");

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn from_q12(value: [i32; 3]) -> Self {
        Self::new(
            value[0] as f64 / 4096.0,
            value[1] as f64 / 4096.0,
            value[2] as f64 / 4096.0,
        )
    }
    pub fn from_q24(value: [i32; 3]) -> Self {
        Self::new(
            value[0] as f64 / 16_777_216.0,
            value[1] as f64 / 16_777_216.0,
            value[2] as f64 / 16_777_216.0,
        )
    }
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
    pub fn norm2(self) -> f64 {
        self.dot(self)
    }
    pub fn norm(self) -> f64 {
        self.norm2().sqrt()
    }
    pub fn normalized(self) -> Option<Self> {
        let n = self.norm();
        (n.is_finite() && n > 1e-12).then(|| self / n)
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}
impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}
impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}
impl std::ops::Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}
impl std::ops::Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbitKind {
    Elliptic,
    Impacting,
    Escape,
    Degenerate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitSolution {
    pub kind: OrbitKind,
    pub position: Vec3,
    pub velocity: Vec3,
    pub angular_momentum: Vec3,
    pub eccentricity_vector: Vec3,
    pub semi_major_km: f64,
    pub eccentricity: f64,
    pub perigee_altitude_km: f64,
    pub apogee_altitude_km: f64,
    pub inclination_deg: f64,
    pub raan_rad: f64,
    pub argument_of_periapsis_rad: f64,
    pub true_anomaly_rad: f64,
    pub period_seconds: f64,
    pub radial_velocity_km_s: f64,
    pub specific_energy: f64,
    pub plane_x: Vec3,
    pub plane_y: Vec3,
}

pub fn orbit_from_state(position: Vec3, velocity: Vec3) -> Option<OrbitSolution> {
    if !position.finite() || !velocity.finite() {
        return None;
    }
    let radius = position.norm();
    let speed2 = velocity.norm2();
    if radius <= 1.0 || !speed2.is_finite() {
        return None;
    }
    let h = position.cross(velocity);
    let hmag = h.norm();
    if hmag <= 1e-9 {
        return Some(OrbitSolution {
            kind: OrbitKind::Degenerate,
            position,
            velocity,
            angular_momentum: h,
            eccentricity_vector: Vec3::default(),
            semi_major_km: f64::NAN,
            eccentricity: f64::NAN,
            perigee_altitude_km: f64::NAN,
            apogee_altitude_km: f64::NAN,
            inclination_deg: f64::NAN,
            raan_rad: f64::NAN,
            argument_of_periapsis_rad: f64::NAN,
            true_anomaly_rad: f64::NAN,
            period_seconds: f64::NAN,
            radial_velocity_km_s: position.dot(velocity) / radius,
            specific_energy: speed2 * 0.5 - EARTH_MU_KM3_S2 / radius,
            plane_x: Vec3::new(1.0, 0.0, 0.0),
            plane_y: Vec3::new(0.0, 1.0, 0.0),
        });
    }
    let hhat = h / hmag;
    let evec = velocity.cross(h) / EARTH_MU_KM3_S2 - position / radius;
    let eccentricity = evec.norm();
    let energy = speed2 * 0.5 - EARTH_MU_KM3_S2 / radius;
    let semi_major = if energy.abs() > 1e-12 {
        -EARTH_MU_KM3_S2 / (2.0 * energy)
    } else {
        f64::INFINITY
    };
    let p = hmag * hmag / EARTH_MU_KM3_S2;
    let perigee_radius = p / (1.0 + eccentricity);
    let apogee_radius = if eccentricity < 1.0 {
        p / (1.0 - eccentricity)
    } else {
        f64::INFINITY
    };
    let kind = if energy >= 0.0 || eccentricity >= 1.0 {
        OrbitKind::Escape
    } else if perigee_radius < EARTH_RADIUS_KM {
        OrbitKind::Impacting
    } else {
        OrbitKind::Elliptic
    };
    let inclination = (h.z / hmag).clamp(-1.0, 1.0).acos();
    let node = Vec3::new(-h.y, h.x, 0.0);
    let node_mag = node.norm();
    let raan = if node_mag > 1e-12 {
        node.y.atan2(node.x).rem_euclid(TAU)
    } else {
        0.0
    };
    let plane_x = if eccentricity > 1e-10 {
        evec / eccentricity
    } else {
        position / radius
    };
    let plane_y = hhat.cross(plane_x).normalized()?;
    let argument_of_periapsis = if node_mag > 1e-12 && eccentricity > 1e-10 {
        let c = (node.dot(evec) / (node_mag * eccentricity)).clamp(-1.0, 1.0);
        let mut value = c.acos();
        if evec.z < 0.0 {
            value = TAU - value;
        }
        value
    } else {
        0.0
    };
    let true_anomaly = position
        .dot(plane_y)
        .atan2(position.dot(plane_x))
        .rem_euclid(TAU);
    let period = if semi_major.is_finite() && semi_major > 0.0 && eccentricity < 1.0 {
        TAU * (semi_major.powi(3) / EARTH_MU_KM3_S2).sqrt()
    } else {
        f64::INFINITY
    };
    Some(OrbitSolution {
        kind,
        position,
        velocity,
        angular_momentum: h,
        eccentricity_vector: evec,
        semi_major_km: semi_major,
        eccentricity,
        perigee_altitude_km: perigee_radius - EARTH_RADIUS_KM,
        apogee_altitude_km: apogee_radius - EARTH_RADIUS_KM,
        inclination_deg: inclination.to_degrees(),
        raan_rad: raan,
        argument_of_periapsis_rad: argument_of_periapsis,
        true_anomaly_rad: true_anomaly,
        period_seconds: period,
        radial_velocity_km_s: position.dot(velocity) / radius,
        specific_energy: energy,
        plane_x,
        plane_y,
    })
}

pub fn orbit_position_at_true_anomaly(orbit: OrbitSolution, anomaly: f64) -> Option<Vec3> {
    if matches!(orbit.kind, OrbitKind::Degenerate) {
        return None;
    }
    let p = orbit.angular_momentum.norm2() / EARTH_MU_KM3_S2;
    let denominator = 1.0 + orbit.eccentricity * anomaly.cos();
    if denominator <= 1e-9 {
        return None;
    }
    let radius = p / denominator;
    Some((orbit.plane_x * anomaly.cos() + orbit.plane_y * anomaly.sin()) * radius)
}

pub fn sample_orbit(orbit: OrbitSolution, count: usize) -> Vec<Vec3> {
    if count < 2 || matches!(orbit.kind, OrbitKind::Degenerate) {
        return Vec::new();
    }
    let full = orbit.eccentricity < 1.0;
    let start = if full {
        0.0
    } else {
        orbit.true_anomaly_rad - 1.25
    };
    let span = if full { TAU } else { 2.5 };
    (0..count)
        .filter_map(|index| {
            orbit_position_at_true_anomaly(orbit, start + span * index as f64 / (count - 1) as f64)
        })
        .collect()
}

pub fn propagate_elliptic(orbit: OrbitSolution, delta_seconds: f64) -> Option<Vec3> {
    if orbit.eccentricity >= 1.0 || !orbit.semi_major_km.is_finite() || orbit.semi_major_km <= 0.0 {
        return None;
    }
    let e = orbit.eccentricity;
    let nu = orbit.true_anomaly_rad;
    let eccentric_anomaly =
        2.0 * ((1.0 - e).sqrt() * (nu * 0.5).sin()).atan2((1.0 + e).sqrt() * (nu * 0.5).cos());
    let mean0 = eccentric_anomaly - e * eccentric_anomaly.sin();
    let mean_motion = (EARTH_MU_KM3_S2 / orbit.semi_major_km.powi(3)).sqrt();
    let target = (mean0 + mean_motion * delta_seconds).rem_euclid(TAU);
    let mut eccentric = target;
    for _ in 0..12 {
        let f = eccentric - e * eccentric.sin() - target;
        let derivative = 1.0 - e * eccentric.cos();
        if derivative.abs() < 1e-12 {
            return None;
        }
        eccentric -= f / derivative;
    }
    let x = orbit.semi_major_km * (eccentric.cos() - e);
    let y = orbit.semi_major_km * (1.0 - e * e).sqrt() * eccentric.sin();
    Some(orbit.plane_x * x + orbit.plane_y * y)
}

pub fn eci_to_ecef(position: Vec3, mission_seconds: f64) -> Vec3 {
    let angle = EARTH_ROTATION_RAD_S * mission_seconds;
    let (s, c) = angle.sin_cos();
    Vec3::new(
        c * position.x + s * position.y,
        -s * position.x + c * position.y,
        position.z,
    )
}

pub fn latitude_longitude(position_eci: Vec3, mission_seconds: f64) -> Option<(f64, f64)> {
    let p = eci_to_ecef(position_eci, mission_seconds);
    let radius = p.norm();
    if radius <= 1e-9 {
        return None;
    }
    Some((
        (p.z / radius).clamp(-1.0, 1.0).asin().to_degrees(),
        p.y.atan2(p.x).to_degrees(),
    ))
}

pub fn great_circle_downrange(position_eci: Vec3, mission_seconds: f64) -> f64 {
    let Some((latitude, longitude)) = latitude_longitude(position_eci, mission_seconds) else {
        return 0.0;
    };
    let lat1 = LAUNCH_LATITUDE_DEG.to_radians();
    let lon1 = LAUNCH_LONGITUDE_DEG.to_radians();
    let lat2 = latitude.to_radians();
    let lon2 = longitude.to_radians();
    let central = (lat1.sin() * lat2.sin() + lat1.cos() * lat2.cos() * (lon2 - lon1).cos())
        .clamp(-1.0, 1.0)
        .acos();
    EARTH_RADIUS_KM * central
}

pub fn flight_path_angle(position: Vec3, velocity: Vec3) -> f64 {
    let denominator = position.norm() * velocity.norm();
    if denominator <= 1e-12 {
        return 0.0;
    }
    (position.dot(velocity) / denominator)
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnvironmentEstimate {
    pub mach: f64,
    pub dynamic_pressure_kpa: f64,
    pub air_speed_km_s: f64,
}

pub fn environment_from_observed_raw(
    position_q12: [i32; 3],
    velocity_q24: [i32; 3],
) -> Option<EnvironmentEstimate> {
    let state = SpatialState::new(
        PositionVec::new(position_q12[0], position_q12[1], position_q12[2]),
        VelocityVec::new(velocity_q24[0], velocity_q24[1], velocity_q24[2]),
    );
    let mut status = NumericStatus::CLEAR;
    let sample = evaluate_spatial_environment(state, &mut status);
    if !status.is_clear() || sample.sound_speed_q24() <= 0 {
        return None;
    }
    let air_speed = sample.air_speed_q24() as f64 / 16_777_216.0;
    let sound = sample.sound_speed_q24() as f64 / 16_777_216.0;
    Some(EnvironmentEstimate {
        mach: air_speed / sound,
        dynamic_pressure_kpa: sample.dynamic_pressure().raw() as f64 / 65536.0,
        air_speed_km_s: air_speed,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannedPoint {
    pub step: u32,
    pub time_seconds: f64,
    pub position_eci: Vec3,
    pub dynamic_pressure_kpa: f64,
    pub events: u16,
}
#[derive(Clone, Debug)]
pub struct PlanReference {
    pub points: Vec<PlannedPoint>,
    pub terminal_position: Vec3,
    pub terminal_velocity: Vec3,
    pub orbit: OrbitSolution,
    pub stream_crc32: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    History,
    Identity,
    Json,
    Orbit,
}

impl PlanReference {
    pub fn load_embedded() -> Result<Self, PlanError> {
        let header = validate_kph5(PLAN_BYTES).map_err(|_| PlanError::History)?;
        if header.point_count as usize != PLAN_POINTS
            || header.run_index != 0
            || header.terminal_step != PLAN_TERMINAL_STEP
            || crc32_ieee(PLAN_BYTES) != PLAN_STREAM_CRC32
        {
            return Err(PlanError::Identity);
        }
        let mut points = Vec::with_capacity(PLAN_POINTS);
        for index in 0..PLAN_POINTS {
            let at = KPH5_HEADER_LENGTH + index * KPH5_POINT_LENGTH;
            let p = parse_kph5_point(&PLAN_BYTES[at..at + KPH5_POINT_LENGTH])
                .map_err(|_| PlanError::History)?;
            points.push(PlannedPoint {
                step: p.step as u32,
                time_seconds: p.step as f64 * MISSION_STEP_SECONDS,
                position_eci: Vec3::new(
                    p.position_quarter_km[0] as f64 * 0.25,
                    p.position_quarter_km[1] as f64 * 0.25,
                    p.position_quarter_km[2] as f64 * 0.25,
                ),
                dynamic_pressure_kpa: p.dynamic_pressure_sixteenth_kpa as f64 / 16.0,
                events: p.events,
            });
        }
        let json: Value = serde_json::from_str(MISSION_REFERENCE).map_err(|_| PlanError::Json)?;
        let raw = &json["cases"]["nominal"]["raw"];
        let position = json_vec3_q(raw, "position_q12", 4096.0)?;
        let velocity = json_vec3_q(raw, "velocity_q24", 16_777_216.0)?;
        let orbit = orbit_from_state(position, velocity).ok_or(PlanError::Orbit)?;
        Ok(Self {
            points,
            terminal_position: position,
            terminal_velocity: velocity,
            orbit,
            stream_crc32: PLAN_STREAM_CRC32,
        })
    }
    pub fn point_at_step(&self, step: u32) -> Option<PlannedPoint> {
        let index = self.points.partition_point(|point| point.step <= step);
        if index == 0 {
            self.points.first().copied()
        } else {
            self.points.get(index - 1).copied()
        }
    }
}

fn json_vec3_q(value: &Value, key: &str, scale: f64) -> Result<Vec3, PlanError> {
    let values = value[key].as_array().ok_or(PlanError::Json)?;
    if values.len() != 3 {
        return Err(PlanError::Json);
    }
    Ok(Vec3::new(
        values[0].as_i64().ok_or(PlanError::Json)? as f64 / scale,
        values[1].as_i64().ok_or(PlanError::Json)? as f64 / scale,
        values[2].as_i64().ok_or(PlanError::Json)? as f64 / scale,
    ))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Residual {
    pub radial_km: f64,
    pub along_track_km: f64,
    pub cross_track_km: f64,
}
pub fn residual_in_plan_frame(planned: Vec3, estimated: Vec3) -> Residual {
    let radial = planned.normalized().unwrap_or(Vec3::new(1.0, 0.0, 0.0));
    let along = Vec3::new(0.0, 0.0, 1.0)
        .cross(radial)
        .normalized()
        .unwrap_or(Vec3::new(0.0, 1.0, 0.0));
    let cross = radial
        .cross(along)
        .normalized()
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
    let delta = estimated - planned;
    Residual {
        radial_km: delta.dot(radial),
        along_track_km: delta.dot(along),
        cross_track_km: delta.dot(cross),
    }
}

pub fn wrap_longitude(value: f64) -> f64 {
    (value + 180.0).rem_euclid(360.0) - 180.0
}
pub fn split_antimeridian(points: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
    let mut segments = Vec::new();
    let mut current = Vec::new();
    for &point in points {
        if current
            .last()
            .is_some_and(|prior: &(f64, f64)| (prior.0 - point.0).abs() > 180.0)
        {
            if current.len() > 1 {
                segments.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
        current.push((wrap_longitude(point.0), point.1.clamp(-90.0, 90.0)));
    }
    if current.len() > 1 {
        segments.push(current);
    }
    segments
}
pub fn project_to_plane(point: Vec3, x: Vec3, y: Vec3) -> (f64, f64) {
    (point.dot(x), point.dot(y))
}
pub fn format_orbit_kind(kind: OrbitKind) -> &'static str {
    match kind {
        OrbitKind::Elliptic => "STABLE ELLIPSE",
        OrbitKind::Impacting => "EARTH-INTERSECTING",
        OrbitKind::Escape => "OPEN / ESCAPE",
        OrbitKind::Degenerate => "DEGENERATE",
    }
}
pub fn time_to_apsis(orbit: OrbitSolution, apogee: bool) -> Option<f64> {
    if orbit.eccentricity >= 1.0 || !orbit.period_seconds.is_finite() {
        return None;
    }
    let e = orbit.eccentricity;
    let nu = orbit.true_anomaly_rad;
    let eccentric =
        2.0 * ((1.0 - e).sqrt() * (nu * 0.5).sin()).atan2((1.0 + e).sqrt() * (nu * 0.5).cos());
    let current_mean = (eccentric - e * eccentric.sin()).rem_euclid(TAU);
    let target_mean = if apogee { PI } else { 0.0 };
    Some((target_mean - current_mean).rem_euclid(TAU) / TAU * orbit.period_seconds)
}
