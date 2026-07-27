//! Phase 10 nominal capture, strict artifacts, and passive host reports.

#[cfg(feature = "native")]
use crate::phase10::GlobalFixtureSet;
use ksa64_core::numeric::{magnitude3_floor, NumericStatus};
use ksa64_core::phase10_environment::ecef_to_geodetic;
use ksa64_core::phase10_telemetry::{
    global_evaluation_identity, GlobalEvaluationSummary, GlobalPlotHeader, GlobalPlotPoint,
    GlobalTelemetryFrame, GlobalTelemetryHeader, KPH10_HEADER_LENGTH, KPH10_POINT_LENGTH,
    KSR10_LENGTH, KTT10_FRAME_LENGTH, KTT10_HEADER_LENGTH,
};
use ksa64_sim::phase10::{FrameTransitionRecord, GlobalWorldError};
#[cfg(feature = "native")]
use ksa64_sim::phase10_avionics::{reference_global_flight_config, GlobalSensorFaults};
use ksa64_sim::phase10_avionics::{GlobalAvionicsMission, GlobalFlightReleaseProcessor};
#[cfg(feature = "native")]
use ksa64_sim::phase10_evaluation::{evaluate_global, GlobalEvaluationRequest};
use serde_json::json;
use std::fmt::Write as _;
#[cfg(feature = "native")]
use std::fs;
#[cfg(feature = "native")]
use std::path::Path;
#[cfg(feature = "native")]
use std::time::Instant;

pub const PHASE10_NOMINAL_CASE_SEED: u32 = 0x4b53_41a0;
pub const PHASE10_NOMINAL_SESSION: u16 = 0x10a0;
pub const PHASE10_TELEMETRY_IDENTITY: u32 = 0x10a1_0001;
pub const PHASE10_PLOT_IDENTITY: u32 = 0x10a1_0002;
pub const PHASE10_RECORD_STRIDE_RELEASES: u16 = 32;
pub const PHASE10_OBSERVER_STRIDE_RELEASES: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalMissionUpdate {
    pub frame: GlobalTelemetryFrame,
    pub plot: GlobalPlotPoint,
    pub release: u32,
}

#[derive(Clone, Debug)]
pub struct GlobalMissionCapture {
    pub telemetry_header: GlobalTelemetryHeader,
    pub plot_identity: u32,
    pub frames: Vec<GlobalTelemetryFrame>,
    pub plot_points: Vec<GlobalPlotPoint>,
    pub summary: GlobalEvaluationSummary,
    pub transition_records: [FrameTransitionRecord; 4],
    pub releases: u32,
    pub wall_seconds: f64,
}

#[cfg(feature = "native")]
pub fn capture_nominal_global_mission<F>(
    mut observer: F,
) -> Result<GlobalMissionCapture, GlobalWorldError>
where
    F: FnMut(&GlobalMissionUpdate),
{
    let fixtures = GlobalFixtureSet::embedded();
    let initial_world = ksa64_sim::phase10::GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )?;
    let flight_config = reference_global_flight_config(
        PHASE10_NOMINAL_SESSION,
        initial_world.active_state()?,
        fixtures.mission,
    )?;
    let mut runner = GlobalAvionicsMission::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
        flight_config,
        GlobalSensorFaults::NONE,
        PHASE10_NOMINAL_CASE_SEED,
    )?;
    let started = Instant::now();
    let mut frames = Vec::new();
    let mut plot_points = Vec::new();
    let mut releases = 0u32;
    let mut previous_transitions = 0u8;
    loop {
        let flight = runner.release()?;
        let snapshot = runner.world().snapshot()?;
        releases = releases.saturating_add(1);
        let update = mission_update(&runner, snapshot, flight, releases)?;
        let important = releases == 1
            || releases.is_multiple_of(u32::from(PHASE10_RECORD_STRIDE_RELEASES))
            || snapshot.events != 0
            || snapshot.transition_count != previous_transitions
            || runner.world().is_complete();
        if important {
            frames.push(update.frame);
            plot_points.push(update.plot);
        }
        if important || releases.is_multiple_of(PHASE10_OBSERVER_STRIDE_RELEASES) {
            observer(&update);
        }
        previous_transitions = snapshot.transition_count;
        if runner.world().is_complete() {
            break;
        }
        runner.advance_to_next_release()?;
        if releases > 460_800 {
            return Err(GlobalWorldError::Timeout);
        }
    }
    let transitions = *runner.world().transitions();

    let evaluation_world = ksa64_sim::phase10::GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )?;
    let evaluation_flight = reference_global_flight_config(
        PHASE10_NOMINAL_SESSION,
        evaluation_world.active_state()?,
        fixtures.mission,
    )?;
    let summary = evaluate_global(GlobalEvaluationRequest {
        earth: &fixtures.earth,
        transforms: &fixtures.transforms,
        atmosphere: &fixtures.atmosphere,
        vehicle: &fixtures.vehicle,
        mission: fixtures.mission,
        avionics: evaluation_flight,
        uncertainty: GlobalSensorFaults::NONE,
        case_seed: PHASE10_NOMINAL_CASE_SEED,
    })?;
    let telemetry_header = GlobalTelemetryHeader {
        identity: PHASE10_TELEMETRY_IDENTITY,
        earth_identity: fixtures.earth.identity,
        transform_identity: fixtures.transforms.identity,
        atmosphere_identity: fixtures.atmosphere.identity,
        vehicle_identity: fixtures.vehicle.identity,
        mission_identity: fixtures.mission.identity,
        avionics_identity: ksa64_flight::phase10::GLOBAL_FLIGHT_CONTRACT_ID,
        case_seed: PHASE10_NOMINAL_CASE_SEED,
        telemetry_period_q16: u32::from(PHASE10_RECORD_STRIDE_RELEASES) * 2_048,
        max_mission_time_q16: fixtures.mission.max_mission_time_q16_s,
    };
    Ok(GlobalMissionCapture {
        telemetry_header,
        plot_identity: PHASE10_PLOT_IDENTITY,
        frames,
        plot_points,
        summary,
        transition_records: transitions,
        releases,
        wall_seconds: started.elapsed().as_secs_f64(),
    })
}

#[cfg(feature = "native")]
pub(crate) fn mission_update<P: GlobalFlightReleaseProcessor>(
    runner: &GlobalAvionicsMission<'_, P>,
    snapshot: ksa64_sim::phase10::GlobalWorldSnapshot,
    flight: ksa64_flight::phase10::GlobalFlightEvidence,
    release: u32,
) -> Result<GlobalMissionUpdate, GlobalWorldError> {
    mission_update_with_case(runner, snapshot, flight, release, PHASE10_NOMINAL_CASE_SEED)
}

pub(crate) fn mission_update_with_case<P: GlobalFlightReleaseProcessor>(
    runner: &GlobalAvionicsMission<'_, P>,
    snapshot: ksa64_sim::phase10::GlobalWorldSnapshot,
    flight: ksa64_flight::phase10::GlobalFlightEvidence,
    release: u32,
    case_seed: u32,
) -> Result<GlobalMissionUpdate, GlobalWorldError> {
    let state = snapshot.state;
    let ecef = runner.world().ecef_state_public()?;
    let geodetic = ecef_to_geodetic(ecef.position)?;
    let offset = runner.world().ecef_to_launch_offset_public(ecef)?;
    let mut status = NumericStatus::CLEAR;
    let speed = magnitude3_floor(
        ecef.velocity.x(),
        ecef.velocity.y(),
        ecef.velocity.z(),
        &mut status,
    );
    if !status.is_clear() || speed > i32::MAX as u32 {
        return Err(GlobalWorldError::Numeric);
    }
    let truth_attitude = state.attitude;
    let frame = GlobalTelemetryFrame {
        step: release,
        mission_time_q16: state.time.raw(),
        frame: snapshot.frame,
        segment: snapshot.segment,
        flight_mode: flight.mode as u8,
        events: snapshot.events,
        truth_position_q12: vector(state.position),
        truth_velocity_q24: vector(state.velocity),
        truth_attitude_q30: [
            truth_attitude.w(),
            truth_attitude.x(),
            truth_attitude.y(),
            truth_attitude.z(),
        ],
        truth_angular_rate_q24: vector(state.angular_rate),
        ecef_position_q12: vector(ecef.position),
        ecef_velocity_q24: vector(ecef.velocity),
        navigation_position_q12: flight.navigation.position_q12,
        navigation_velocity_q24: flight.navigation.velocity_q24,
        navigation_attitude_q30: flight.navigation.attitude_q30,
        altitude_q12_km: snapshot.altitude_q12_km,
        mach_q24: snapshot.mach_q24,
        dynamic_pressure_q14_pa: snapshot.dynamic_pressure_q14_pa,
        total_mass_q21_kg: snapshot.total_mass_q21_kg,
        main_propellant_q21_kg: snapshot.main_propellant_q21_kg,
        rcs_propellant_q21_kg: snapshot.rcs_propellant_q21_kg,
        gimbal_q15: flight.command.gimbal_q15,
        rcs_pulses: flight.command.rcs_pulse_quanta,
        command_flags: flight.command.flags,
        command_discrete: flight.command.discrete,
        alarms: flight.alarms,
        transition_count: snapshot.transition_count,
        checksums: [
            snapshot.checksum,
            flight.navigation.checksum,
            flight.flight_checksum,
            flight.sensor_checksum,
            flight.command.command_checksum,
            flight.status.map_or(0, |status| status.flight_checksum),
            flight.deadline_misses.into(),
            case_seed,
        ],
    };
    let plot = GlobalPlotPoint {
        mission_time_q16: state.time.raw(),
        latitude_q28_rad: geodetic.latitude_q28_rad,
        longitude_q28_rad: geodetic.longitude_q28_rad,
        altitude_q12_km: geodetic.height_q12_km,
        downrange_q12_km: offset.x(),
        crossrange_q12_km: offset.y(),
        speed_q24_km_s: speed as i32,
        frame: snapshot.frame,
        segment: snapshot.segment,
        events: snapshot.events,
        truth_checksum: snapshot.checksum,
    };
    Ok(GlobalMissionUpdate {
        frame,
        plot,
        release,
    })
}

fn vector<const FRACTIONAL_BITS: u8>(
    value: ksa64_core::spatial_numeric::FixedVec3<FRACTIONAL_BITS>,
) -> [i32; 3] {
    [value.x(), value.y(), value.z()]
}

pub fn encode_ktt10(capture: &GlobalMissionCapture) -> Result<Vec<u8>, String> {
    let mut output = vec![0; KTT10_HEADER_LENGTH + capture.frames.len() * KTT10_FRAME_LENGTH];
    let mut header = [0; KTT10_HEADER_LENGTH];
    capture
        .telemetry_header
        .encode(&mut header)
        .map_err(|error| format!("{error:?}"))?;
    output[..KTT10_HEADER_LENGTH].copy_from_slice(&header);
    for (index, frame) in capture.frames.iter().enumerate() {
        let mut bytes = [0; KTT10_FRAME_LENGTH];
        frame
            .encode(&mut bytes)
            .map_err(|error| format!("{error:?}"))?;
        let offset = KTT10_HEADER_LENGTH + index * KTT10_FRAME_LENGTH;
        output[offset..offset + KTT10_FRAME_LENGTH].copy_from_slice(&bytes);
    }
    Ok(output)
}

pub fn encode_kph10(capture: &GlobalMissionCapture) -> Result<Vec<u8>, String> {
    let point_count: u16 = capture
        .plot_points
        .len()
        .try_into()
        .map_err(|_| "too many KPH10 points")?;
    let header = GlobalPlotHeader {
        identity: capture.plot_identity,
        evaluation_identity: global_evaluation_identity(&capture.summary),
        point_count,
        stride_releases: PHASE10_RECORD_STRIDE_RELEASES,
    };
    let mut output = vec![0; KPH10_HEADER_LENGTH + capture.plot_points.len() * KPH10_POINT_LENGTH];
    let mut header_bytes = [0; KPH10_HEADER_LENGTH];
    header
        .encode(&mut header_bytes)
        .map_err(|error| format!("{error:?}"))?;
    output[..KPH10_HEADER_LENGTH].copy_from_slice(&header_bytes);
    for (index, point) in capture.plot_points.iter().enumerate() {
        let mut bytes = [0; KPH10_POINT_LENGTH];
        point
            .encode(&mut bytes)
            .map_err(|error| format!("{error:?}"))?;
        let offset = KPH10_HEADER_LENGTH + index * KPH10_POINT_LENGTH;
        output[offset..offset + KPH10_POINT_LENGTH].copy_from_slice(&bytes);
    }
    Ok(output)
}

pub fn encode_ksr10(capture: &GlobalMissionCapture) -> Result<[u8; KSR10_LENGTH], String> {
    let mut output = [0; KSR10_LENGTH];
    capture
        .summary
        .encode(&mut output)
        .map_err(|error| format!("{error:?}"))?;
    Ok(output)
}

#[cfg(feature = "native")]
pub fn write_global_mission_artifacts(
    capture: &GlobalMissionCapture,
    output: &Path,
) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|error| error.to_string())?;
    fs::write(
        output.join("ksa-g10r-nominal.ktt10"),
        encode_ktt10(capture)?,
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        output.join("ksa-g10r-nominal.kph10"),
        encode_kph10(capture)?,
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        output.join("ksa-g10r-nominal.ksr10"),
        encode_ksr10(capture)?,
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        output.join("ksa-g10r-nominal.kmr10.json"),
        serde_json::to_vec_pretty(&mission_json(capture)).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::write(output.join("ksa-g10r-nominal.csv"), mission_csv(capture))
        .map_err(|error| error.to_string())?;
    fs::write(output.join("ksa-g10r-nominal.html"), mission_html(capture))
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn mission_json(capture: &GlobalMissionCapture) -> serde_json::Value {
    let points = capture
        .plot_points
        .iter()
        .map(|point| {
            json!({
                "time_s": q16(point.mission_time_q16),
                "latitude_deg": q28_radians_to_degrees(point.latitude_q28_rad),
                "longitude_deg": q28_radians_to_degrees(point.longitude_q28_rad),
                "altitude_km": q12(point.altitude_q12_km),
                "downrange_km": q12(point.downrange_q12_km),
                "crossrange_km": q12(point.crossrange_q12_km),
                "speed_km_s": q24(point.speed_q24_km_s),
                "frame": format!("{:?}", point.frame),
                "segment": format!("{:?}", point.segment),
                "events": format!("0x{:04x}", point.events),
                "truth_checksum": format!("0x{:08x}", point.truth_checksum),
            })
        })
        .collect::<Vec<_>>();
    let transitions = capture
        .transition_records
        .iter()
        .take(capture.summary.transition_count as usize)
        .map(|transition| {
            json!({
                "from": format!("{:?}", transition.from),
                "to": format!("{:?}", transition.to),
                "time_s": q16(transition.time.raw()),
                "position_delta_raw": transition.position_delta_raw,
                "velocity_delta_raw": transition.velocity_delta_raw,
                "attitude_delta_raw": transition.attitude_delta_raw,
                "angular_rate_delta_raw": transition.angular_rate_delta_raw,
                "checksum": format!("0x{:08x}", transition.checksum),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema": "ksa64.phase10.mission-recording-v1",
        "case_seed": format!("0x{PHASE10_NOMINAL_CASE_SEED:08x}"),
        "evaluation_identity": format!("0x{:08x}", global_evaluation_identity(&capture.summary)),
        "outcome": format!("{:?}", capture.summary.common.outcome),
        "releases": capture.releases,
        "retained_frames": capture.frames.len(),
        "wall_seconds": capture.wall_seconds,
        "apogee_km": q12(capture.summary.apogee_q12_km),
        "downrange_km": q12(capture.summary.downrange_q12_km),
        "crossrange_km": q12(capture.summary.crossrange_q12_km),
        "max_mach": q24(capture.summary.max_mach_q24),
        "max_dynamic_pressure_pa": q14(capture.summary.max_dynamic_pressure_q14_pa),
        "terminal_rcs_propellant_kg": q21(capture.summary.terminal_rcs_propellant_q21_kg),
        "transition_count": capture.summary.transition_count,
        "transitions": transitions,
        "points": points,
        "limitations": [
            "Fictional assumption-backed vehicle",
            "Compiled idealized atmosphere",
            "Engineering simulation only",
            "Not launch approval, certification, regulation, or safety authority"
        ]
    })
}

pub fn mission_csv(capture: &GlobalMissionCapture) -> String {
    let mut output = String::from(
        "time_s,latitude_deg,longitude_deg,altitude_km,downrange_km,crossrange_km,speed_km_s,frame,segment,events,truth_checksum\n",
    );
    for point in &capture.plot_points {
        let _ = writeln!(
            output,
            "{:.6},{:.9},{:.9},{:.6},{:.6},{:.6},{:.9},{:?},{:?},0x{:04x},0x{:08x}",
            q16(point.mission_time_q16),
            q28_radians_to_degrees(point.latitude_q28_rad),
            q28_radians_to_degrees(point.longitude_q28_rad),
            q12(point.altitude_q12_km),
            q12(point.downrange_q12_km),
            q12(point.crossrange_q12_km),
            q24(point.speed_q24_km_s),
            point.frame,
            point.segment,
            point.events,
            point.truth_checksum,
        );
    }
    output
}

pub fn mission_html(capture: &GlobalMissionCapture) -> String {
    let world_path = svg_world_path(&capture.plot_points);
    let altitude_path = svg_altitude_path(&capture.plot_points);
    let transitions = capture
        .transition_records
        .iter()
        .take(capture.summary.transition_count as usize)
        .map(|record| {
            format!(
                "<li>{:?} → {:?} at T+{:.3}s; Δr={} raw, Δv={} raw, Δq={} raw</li>",
                record.from,
                record.to,
                q16(record.time.raw()),
                record.position_delta_raw,
                record.velocity_delta_raw,
                record.attitude_delta_raw
            )
        })
        .collect::<String>();
    format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>KSA64 Phase 10 — KSA-G10R Mission Evidence</title>
<style>
:root{{--bg:#07111f;--panel:#0e2034;--line:#1e4568;--cyan:#57e4ff;--gold:#ffd166;--green:#63f5a4;--text:#e9f4ff;--muted:#91a9bd}}
*{{box-sizing:border-box}} body{{margin:0;background:radial-gradient(circle at top,#102b45,var(--bg) 45%);color:var(--text);font:15px/1.45 system-ui,sans-serif}}
main{{max-width:1400px;margin:auto;padding:28px}} h1{{margin:.1em 0;color:var(--cyan);letter-spacing:.05em}} h2{{color:var(--gold)}}
.badge{{display:inline-block;border:1px solid var(--green);color:var(--green);padding:5px 10px;border-radius:999px}}
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));gap:14px}} .card{{background:linear-gradient(145deg,#112943,#0b192a);border:1px solid var(--line);border-radius:12px;padding:16px;box-shadow:0 12px 35px #0006}}
.metric{{font-size:1.8rem;font-weight:700;color:var(--cyan)}} .muted{{color:var(--muted)}} svg{{width:100%;height:auto;background:#06101b;border:1px solid var(--line);border-radius:8px}}
.earth{{fill:#0d2940;stroke:#285f83}} .gridline{{stroke:#153b59;stroke-width:1}} .trajectory{{fill:none;stroke:var(--cyan);stroke-width:3}} .altitude{{fill:none;stroke:var(--gold);stroke-width:3}}
code{{color:var(--green)}} ul{{padding-left:1.25rem}}
</style></head><body><main>
<span class="badge">DETERMINISTIC ACCEPTED NOMINAL</span>
<h1>KSA64 // GLOBAL EARTH FLIGHT</h1>
<p class="muted">KSA-G10R • UTC epoch 2024-01-01 • ECEF atmosphere / GCRF coast • seed <code>0x4b5341a0</code></p>
<section class="grid">
<div class="card"><div class="muted">Outcome</div><div class="metric">{:?}</div></div>
<div class="card"><div class="muted">Apogee</div><div class="metric">{:.2} km</div></div>
<div class="card"><div class="muted">Downrange</div><div class="metric">{:.2} km</div></div>
<div class="card"><div class="muted">Peak Mach</div><div class="metric">{:.3}</div></div>
<div class="card"><div class="muted">Peak dynamic pressure</div><div class="metric">{:.0} Pa</div></div>
<div class="card"><div class="muted">Frame transitions</div><div class="metric">{}</div></div>
</section>
<h2>World ground track</h2>
<svg viewBox="0 0 1000 430" role="img" aria-label="World ground track">
<rect class="earth" x="1" y="1" width="998" height="398" rx="8"/>
<g class="gridline"><path d="M0 100H1000M0 200H1000M0 300H1000M250 0V400M500 0V400M750 0V400"/></g>
<path class="trajectory" d="{}"/><text x="20" y="420" fill="#91a9bd">longitude −180°…+180° • latitude +90°…−90°</text>
</svg>
<h2>Altitude profile</h2>
<svg viewBox="0 0 1000 330" role="img" aria-label="Altitude over mission time">
<rect class="earth" x="1" y="1" width="998" height="298" rx="8"/>
<g class="gridline"><path d="M0 75H1000M0 150H1000M0 225H1000M250 0V300M500 0V300M750 0V300"/></g>
<path class="altitude" d="{}"/><text x="20" y="320" fill="#91a9bd">mission time → • altitude 0…{:.1} km</text>
</svg>
<section class="grid">
<div class="card"><h2>Ownership transitions</h2><ul>{}</ul></div>
<div class="card"><h2>Evidence identities</h2><p>Evaluation <code>0x{:08x}</code><br>Earth <code>0x{:08x}</code><br>Transforms <code>0x{:08x}</code><br>Atmosphere <code>0x{:08x}</code></p></div>
<div class="card"><h2>Scope</h2><p>This is numerical engineering evidence for a fictional, assumption-backed vehicle. It is not launch approval, certification, regulatory acceptance, or safety authority.</p></div>
</section>
</main></body></html>"##,
        capture.summary.common.outcome,
        q12(capture.summary.apogee_q12_km),
        q12(capture.summary.downrange_q12_km),
        q24(capture.summary.max_mach_q24),
        q14(capture.summary.max_dynamic_pressure_q14_pa),
        capture.summary.transition_count,
        world_path,
        altitude_path,
        capture
            .plot_points
            .iter()
            .map(|point| q12(point.altitude_q12_km))
            .fold(0.0f64, f64::max),
        transitions,
        global_evaluation_identity(&capture.summary),
        capture.summary.earth_identity,
        capture.summary.transform_identity,
        capture.summary.atmosphere_identity,
    )
}

fn svg_world_path(points: &[GlobalPlotPoint]) -> String {
    points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let longitude = q28_radians_to_degrees(point.longitude_q28_rad);
            let latitude = q28_radians_to_degrees(point.latitude_q28_rad);
            let x = (longitude + 180.0) / 360.0 * 1_000.0;
            let y = (90.0 - latitude) / 180.0 * 400.0;
            format!("{} {:.2} {:.2}", if index == 0 { "M" } else { "L" }, x, y)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn svg_altitude_path(points: &[GlobalPlotPoint]) -> String {
    let duration = points
        .last()
        .map_or(1.0, |point| q16(point.mission_time_q16))
        .max(1.0);
    let maximum = points
        .iter()
        .map(|point| q12(point.altitude_q12_km))
        .fold(1.0f64, f64::max);
    points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let x = q16(point.mission_time_q16) / duration * 1_000.0;
            let y = 300.0 - q12(point.altitude_q12_km).max(0.0) / maximum * 300.0;
            format!("{} {:.2} {:.2}", if index == 0 { "M" } else { "L" }, x, y)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub const fn q16(raw: u32) -> f64 {
    raw as f64 / 65_536.0
}
pub const fn q12(raw: i32) -> f64 {
    raw as f64 / 4_096.0
}
pub const fn q14(raw: i32) -> f64 {
    raw as f64 / 16_384.0
}
pub const fn q21(raw: i32) -> f64 {
    raw as f64 / 2_097_152.0
}
pub const fn q24(raw: i32) -> f64 {
    raw as f64 / 16_777_216.0
}
pub fn q28_radians_to_degrees(raw: i32) -> f64 {
    raw as f64 / 268_435_456.0 * 180.0 / std::f64::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase10_telemetry::{
        GlobalPlotHeader, GlobalPlotPoint, GlobalTelemetryFrame, GlobalTelemetryHeader,
    };

    #[test]
    fn strict_artifacts_round_trip_and_reports_are_passive() {
        let capture = capture_nominal_global_mission(|_| {}).unwrap();
        let before = capture.summary;
        let telemetry = encode_ktt10(&capture).unwrap();
        GlobalTelemetryHeader::decode(&telemetry[..KTT10_HEADER_LENGTH]).unwrap();
        for frame in telemetry[KTT10_HEADER_LENGTH..].chunks_exact(KTT10_FRAME_LENGTH) {
            GlobalTelemetryFrame::decode(frame).unwrap();
        }
        let plot = encode_kph10(&capture).unwrap();
        GlobalPlotHeader::decode(&plot[..KPH10_HEADER_LENGTH]).unwrap();
        for point in plot[KPH10_HEADER_LENGTH..].chunks_exact(KPH10_POINT_LENGTH) {
            GlobalPlotPoint::decode(point).unwrap();
        }
        GlobalEvaluationSummary::decode(&encode_ksr10(&capture).unwrap()).unwrap();
        assert!(mission_html(&capture).contains("World ground track"));
        assert!(mission_csv(&capture).starts_with("time_s,"));
        assert_eq!(capture.summary, before);
    }
}
