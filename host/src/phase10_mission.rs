//! Host-owned Phase 10 timing, persistence, and passive reports.

pub use ksa64_session::phase10_mission::*;

use ksa64_core::phase10_telemetry::{global_evaluation_identity, GlobalPlotPoint};
use ksa64_sim::phase10::GlobalWorldError;
use serde_json::json;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::Instant;

/// Historical host entrypoint. Wall-clock measurement remains noncanonical and
/// is injected only after the portable deterministic capture returns.
pub fn capture_nominal_global_mission<F>(
    observer: F,
) -> Result<GlobalMissionCapture, GlobalWorldError>
where
    F: FnMut(&GlobalMissionUpdate),
{
    let started = Instant::now();
    let mut capture = capture_nominal_global_mission_portable(observer)?;
    capture.wall_seconds = started.elapsed().as_secs_f64();
    Ok(capture)
}

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
