use ksa64_host::phase8::run_checked_in_phase8;
use std::{env, fmt::Write as _, fs, path::PathBuf};
fn main() {
    let out = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("phase8"));
    let evidence = run_checked_in_phase8().expect("Phase 8 mission");
    let exports = out.join("exports");
    let plots = out.join("plots");
    fs::create_dir_all(&exports).unwrap();
    fs::create_dir_all(&plots).unwrap();
    fs::write(
        exports.join("firestorm-spatial-v1.json"),
        serde_json::to_string_pretty(&evidence).unwrap() + "\n",
    )
    .unwrap();
    let mut csv=String::from("time_s,phase,events,east_m,north_m,altitude_m,east_velocity_mps,north_velocity_mps,vertical_velocity_mps,mach,aoa_deg,dynamic_pressure_pa,static_margin_calibers,wind_east_mps,wind_north_mps\n");
    for p in &evidence.trace {
        writeln!(
            csv,
            "{:.6},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.8},{:.8},{:.6},{:.8},{:.6},{:.6}",
            p.time_s,
            p.phase,
            p.events,
            p.position_m[0],
            p.position_m[1],
            p.position_m[2],
            p.velocity_mps[0],
            p.velocity_mps[1],
            p.velocity_mps[2],
            p.mach,
            p.angle_of_attack_deg,
            p.dynamic_pressure_pa,
            p.static_margin_calibers,
            p.wind_mps[0],
            p.wind_mps[1]
        )
        .unwrap();
    }
    fs::write(exports.join("firestorm-spatial-v1.csv"), csv).unwrap();
    let max_alt = evidence
        .trace
        .iter()
        .map(|p| p.position_m[2])
        .fold(1.0, f64::max);
    let max_range = evidence
        .trace
        .iter()
        .map(|p| (p.position_m[0] * p.position_m[0] + p.position_m[1] * p.position_m[1]).sqrt())
        .fold(1.0, f64::max);
    let mut side=String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"700\" viewBox=\"0 0 1200 700\"><rect width=\"1200\" height=\"700\" fill=\"#07111f\"/><text x=\"48\" y=\"50\" fill=\"#d9f7ff\" font-family=\"monospace\" font-size=\"28\">KSA64 Firestorm altitude vs ground range</text><polyline fill=\"none\" stroke=\"#52f2c2\" stroke-width=\"4\" points=\"");
    for p in &evidence.trace {
        let range = (p.position_m[0] * p.position_m[0] + p.position_m[1] * p.position_m[1]).sqrt();
        write!(
            side,
            "{:.2},{:.2} ",
            60.0 + 1080.0 * range / max_range,
            640.0 - 540.0 * p.position_m[2] / max_alt
        )
        .unwrap();
    }
    side.push_str("\"/><line x1=\"60\" y1=\"640\" x2=\"1140\" y2=\"640\" stroke=\"#648097\"/><text x=\"60\" y=\"680\" fill=\"#9fb9ca\" font-family=\"monospace\">launch</text><text x=\"1040\" y=\"680\" fill=\"#9fb9ca\" font-family=\"monospace\">landing range</text></svg>\n");
    fs::write(plots.join("altitude-ground-range-v1.svg"), side).unwrap();
    let max_abs = evidence
        .trace
        .iter()
        .flat_map(|p| [p.position_m[0].abs(), p.position_m[1].abs()])
        .fold(1.0, f64::max);
    let mut top=String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"800\" viewBox=\"0 0 800 800\"><rect width=\"800\" height=\"800\" fill=\"#07111f\"/><text x=\"40\" y=\"48\" fill=\"#d9f7ff\" font-family=\"monospace\" font-size=\"25\">KSA64 Firestorm ground track (ENU)</text><line x1=\"400\" y1=\"80\" x2=\"400\" y2=\"760\" stroke=\"#31485b\"/><line x1=\"40\" y1=\"400\" x2=\"760\" y2=\"400\" stroke=\"#31485b\"/><polyline fill=\"none\" stroke=\"#ffcb6b\" stroke-width=\"4\" points=\"");
    for p in &evidence.trace {
        write!(
            top,
            "{:.2},{:.2} ",
            400.0 + 330.0 * p.position_m[0] / max_abs,
            400.0 - 330.0 * p.position_m[1] / max_abs
        )
        .unwrap();
    }
    top.push_str("\"/><circle cx=\"400\" cy=\"400\" r=\"7\" fill=\"#52f2c2\"/><text x=\"690\" y=\"390\" fill=\"#9fb9ca\" font-family=\"monospace\">E</text><text x=\"410\" y=\"100\" fill=\"#9fb9ca\" font-family=\"monospace\">N</text></svg>\n");
    fs::write(plots.join("ground-track-v1.svg"), top).unwrap();
    println!("exported {} trace points", evidence.trace.len())
}
