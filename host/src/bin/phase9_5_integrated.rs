use ksa64_core::phase8_5_contract::AvionicsProfilePack;
use ksa64_core::phase8_format::{KMC8_LENGTH, KMP8_LENGTH, KVP8_LENGTH, KWP8_LENGTH};
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_core::phase8_numeric::{SpatialPosition, SpatialTime, SpatialWind};
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack, SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack,
    WindProfilePack,
};
use ksa64_core::phase9_5_canard::CanardFaultMode;
use ksa64_core::phase9_5_contract::{
    parse_allocator_pack, parse_effector_pack, write_advanced_effector_summary,
    AdvancedEffectorPack, PriorityResidualAllocatorPack, KAS9_LENGTH, KAT9_FRAME_LENGTH,
    KAT9_HEADER_LENGTH, KPA9_LENGTH, KPE9_LENGTH,
};
use ksa64_core::phase9_5_rcs::RcsJetFault;
use ksa64_core::phase9_5_telemetry::{
    write_kat9_frame, write_kat9_header, AdvancedTelemetryHeader,
};
use ksa64_sim::phase8_5::reference_avionics_profile;
use ksa64_sim::phase9_5_mission::{
    evaluate_with_advanced_effectors, reference_capability, run_advanced_loopback_observed,
    AdvancedEffectorEvaluationRequest, AdvancedLoopbackRequest, AdvancedMissionFaults,
};
use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn fixed<const N: usize>(p: impl AsRef<Path>) -> [u8; N] {
    fs::read(p).unwrap().try_into().unwrap()
}
#[derive(Serialize)]
struct Row {
    name: String,
    outcome: String,
    releases: u32,
    steps: u32,
    events: u16,
    apogee_q13: i32,
    rail_exit_time_q18: i32,
    burnout_time_q18: i32,
    apogee_time_q18: i32,
    drogue_time_q18: i32,
    main_time_q18: i32,
    landing_time_q18: i32,
    max_speed_q19: i32,
    max_dynamic_pressure_q13: i32,
    max_attitude_turn16: i16,
    rail_settle_turn16: i16,
    disturbance_settle_turn16: i16,
    pulses: u32,
    saturation: u32,
    valve_edges: u32,
    handoffs: u16,
    fallback_epochs: u16,
    remaining_rcs_q21: i32,
    checksum: u32,
    cell_checksum: u32,
    kas9: String,
    kat9: String,
}
#[allow(clippy::too_many_arguments)]
fn run_case(
    name: &str,
    vehicle: SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mut mission: SpatialMissionPack,
    wind: &WindProfilePack,
    effectors: AdvancedEffectorPack,
    allocator: PriorityResidualAllocatorPack,
    faults: AdvancedMissionFaults,
    out: &Path,
    record: bool,
) -> Row {
    mission.identity ^= mission.vehicle_identity ^ vehicle.identity;
    mission.vehicle_identity = vehicle.identity;
    if mission.wind_identity != wind.identity {
        mission.identity ^= mission.wind_identity ^ wind.identity;
        mission.wind_identity = wind.identity;
    }
    let capability = reference_capability(vehicle.identity, &allocator);
    let avionics: AvionicsProfilePack = reference_avionics_profile(false);
    let req = AdvancedLoopbackRequest {
        vehicle: &vehicle,
        motor,
        mission,
        wind,
        variation: SpatialMissionVariation::NOMINAL,
        variation_checksum: 0,
        avionics,
        capability,
        effectors: &effectors,
        allocator: &allocator,
        faults,
    };
    let mut frames = Vec::new();
    let evidence = run_advanced_loopback_observed(req, |frame| {
        if record {
            let mut b = [0u8; KAT9_FRAME_LENGTH];
            write_kat9_frame(frame, &mut b).unwrap();
            frames.push(b);
        }
    })
    .unwrap();
    let summary = evaluate_with_advanced_effectors(AdvancedEffectorEvaluationRequest {
        vehicle: &vehicle,
        motor,
        mission,
        wind,
        variation: SpatialMissionVariation::NOMINAL,
        variation_checksum: 0,
        avionics,
        capability,
        effectors: &effectors,
        allocator: &allocator,
        uncertainty_identity: 0,
        evaluator_identity: 0x0958_0001,
        faults,
    })
    .unwrap();
    assert_eq!(summary.checksum_chains, evidence.checksum_chains);
    let mut kas = [0u8; KAS9_LENGTH];
    write_advanced_effector_summary(summary, &mut kas).unwrap();
    let kas_name = format!("{name}.kas9");
    fs::write(out.join(&kas_name), kas).unwrap();
    let kat_name = if record {
        let n = format!("{name}.kat9");
        let header = AdvancedTelemetryHeader {
            identity: summary.physical_summary_identity ^ summary.effector_identity,
            vehicle_identity: vehicle.identity,
            motor_identity: motor.identity,
            mission_identity: mission.identity,
            wind_identity: wind.identity,
            avionics_identity: avionics.identity,
            effector_identity: effectors.identity,
            allocator_identity: allocator.identity,
            uncertainty_identity: 0,
            frame_count: frames.len() as u32,
            period_q18: 8192,
            start_time_q18: 0,
            source_checksum: evidence.cell_checksum,
        };
        let mut hb = [0u8; KAT9_HEADER_LENGTH];
        write_kat9_header(header, &mut hb).unwrap();
        let mut bytes = Vec::with_capacity(hb.len() + frames.len() * KAT9_FRAME_LENGTH);
        bytes.extend_from_slice(&hb);
        for f in frames {
            bytes.extend_from_slice(&f)
        }
        fs::write(out.join(&n), bytes).unwrap();
        n
    } else {
        String::new()
    };
    Row {
        name: name.into(),
        outcome: format!("{:?}", evidence.result.outcome),
        releases: evidence.releases,
        steps: evidence.result.steps,
        events: evidence.result.event_history,
        apogee_q13: evidence.result.max_altitude_raw_q13,
        rail_exit_time_q18: evidence.result.rail_exit.time.raw(),
        burnout_time_q18: evidence.result.burnout.time.raw(),
        apogee_time_q18: evidence.result.apogee.time.raw(),
        drogue_time_q18: evidence.result.drogue.time.raw(),
        main_time_q18: evidence.result.main.time.raw(),
        landing_time_q18: evidence.result.landing.time.raw(),
        max_speed_q19: evidence.result.max_speed_raw_q19,
        max_dynamic_pressure_q13: evidence.result.max_dynamic_pressure_raw_q13,
        max_attitude_turn16: evidence.max_attitude_error_turn16,
        rail_settle_turn16: evidence.rail_settle_error_turn16,
        disturbance_settle_turn16: evidence.disturbance_settle_error_turn16,
        pulses: evidence.pulse_count,
        saturation: evidence.saturation_count,
        valve_edges: evidence.valve_edge_count,
        handoffs: evidence.authority_handoffs,
        fallback_epochs: evidence.air_fallback_epochs,
        remaining_rcs_q21: evidence.rcs_final_propellant_q21,
        checksum: evidence.result.checksum,
        cell_checksum: evidence.cell_checksum,
        kas9: kas_name,
        kat9: kat_name,
    }
}
fn steady_crosswind(mut base: WindProfilePack) -> WindProfilePack {
    base.identity ^= 0x5000_0005;
    base.gust_seed = 0;
    base.gust_cadence = SpatialTime::from_raw(1 << 18);
    base.gust_amplitude_east = SpatialWind::ZERO;
    base.gust_amplitude_north = SpatialWind::ZERO;
    base.max_gust = SpatialWind::ZERO;
    let top = base.knots[1];
    base.knot_count = 3;
    base.knots[0].east = SpatialWind::ZERO;
    base.knots[0].north = SpatialWind::ZERO;
    base.knots[1] = top;
    base.knots[1].altitude = SpatialPosition::from_raw(50 << 13);
    base.knots[1].east = SpatialWind::from_raw(5 << 22);
    base.knots[1].north = SpatialWind::ZERO;
    base.knots[2] = top;
    base.knots[2].east = SpatialWind::from_raw(5 << 22);
    base.knots[2].north = SpatialWind::ZERO;
    base
}

fn assert_acceptance(rows: &[Row]) {
    let find = |name: &str| rows.iter().find(|row| row.name == name).unwrap();
    for name in [
        "firestorm-c9-nominal",
        "firestorm-r9-nominal",
        "firestorm-m9-nominal",
    ] {
        assert_eq!(find(name).outcome, "GroundContact");
    }
    let crosswind = find("firestorm-c9-crosswind");
    assert_eq!(crosswind.outcome, "GroundContact");
    assert!(crosswind.rail_settle_turn16 <= 546); // three degrees in turn16
    let disturbance = find("firestorm-r9-disturbance");
    assert_eq!(disturbance.outcome, "GroundContact");
    assert!(disturbance.disturbance_settle_turn16 <= 364); // two degrees
    assert!(disturbance.remaining_rcs_q21 >= 41_943); // protected 20 percent reserve
    let airdata = find("firestorm-m9-airdata-loss");
    assert_eq!(airdata.outcome, "GroundContact");
    assert!(airdata.fallback_epochs >= 64);
    assert_ne!(
        find("firestorm-m9-canard-hardover").outcome,
        "GroundContact"
    );
    assert_ne!(find("firestorm-m9-stuck-open").outcome, "GroundContact");
}

fn main() {
    let root = PathBuf::from(env::args_os().nth(1).unwrap_or_else(|| ".".into()));
    let out = PathBuf::from(
        env::args_os()
            .nth(2)
            .unwrap_or_else(|| "phase9_5/evidence/integrated".into()),
    );
    fs::create_dir_all(&out).unwrap();
    let motor = parse_spatial_motor_pack(&fixed::<KMP8_LENGTH>(
        root.join("phase8/examples/aerotech-i211w.kmp8"),
    ))
    .unwrap();
    let base_mission = parse_spatial_mission_pack(&fixed::<KMC8_LENGTH>(
        root.join("phase8/examples/firestorm-i211.kmc8"),
    ))
    .unwrap();
    let wind = parse_wind_profile_pack(&fixed::<KWP8_LENGTH>(
        root.join("phase8/examples/firestorm-calm.kwp8"),
    ))
    .unwrap();
    let mut rows = Vec::new();
    for (name, stem) in [
        ("firestorm-c9-nominal", "firestorm-c9"),
        ("firestorm-r9-nominal", "firestorm-r9"),
        ("firestorm-m9-nominal", "firestorm-m9"),
    ] {
        let v = parse_spatial_vehicle_pack(&fixed::<KVP8_LENGTH>(
            root.join(format!("phase9_5/examples/{stem}.kvp8")),
        ))
        .unwrap();
        let e = parse_effector_pack(&fixed::<KPE9_LENGTH>(
            root.join(format!("phase9_5/examples/{stem}.kpe9")),
        ))
        .unwrap();
        let a = parse_allocator_pack(&fixed::<KPA9_LENGTH>(
            root.join(format!("phase9_5/examples/{stem}.kpa9")),
        ))
        .unwrap();
        rows.push(run_case(
            name,
            v,
            &motor,
            base_mission,
            &wind,
            e,
            a,
            AdvancedMissionFaults::NOMINAL,
            &out,
            true,
        ));
    }
    let crosswind = steady_crosswind(wind);
    // The canard derivative uses a 2.5 m rail in the accepted 5 m/s case so
    // local incidence remains inside the frozen 15-degree model envelope.
    let mut crosswind_mission = base_mission;
    crosswind_mission.identity ^= 0x0000_5000;
    crosswind_mission.rail_length = SpatialPosition::from_raw(20_480);
    let v = parse_spatial_vehicle_pack(&fixed::<KVP8_LENGTH>(
        root.join("phase9_5/examples/firestorm-c9.kvp8"),
    ))
    .unwrap();
    let e = parse_effector_pack(&fixed::<KPE9_LENGTH>(
        root.join("phase9_5/examples/firestorm-c9.kpe9"),
    ))
    .unwrap();
    let a = parse_allocator_pack(&fixed::<KPA9_LENGTH>(
        root.join("phase9_5/examples/firestorm-c9.kpa9"),
    ))
    .unwrap();
    rows.push(run_case(
        "firestorm-c9-crosswind",
        v,
        &motor,
        crosswind_mission,
        &crosswind,
        e,
        a,
        AdvancedMissionFaults::NOMINAL,
        &out,
        true,
    ));
    let v = parse_spatial_vehicle_pack(&fixed::<KVP8_LENGTH>(
        root.join("phase9_5/examples/firestorm-r9.kvp8"),
    ))
    .unwrap();
    let e = parse_effector_pack(&fixed::<KPE9_LENGTH>(
        root.join("phase9_5/examples/firestorm-r9.kpe9"),
    ))
    .unwrap();
    let a = parse_allocator_pack(&fixed::<KPA9_LENGTH>(
        root.join("phase9_5/examples/firestorm-r9.kpa9"),
    ))
    .unwrap();
    let mut f = AdvancedMissionFaults::NOMINAL;
    f.disturbance_epoch = 256;
    f.disturbance_angular_rate_q24 = [1 << 22, -(1 << 22), 1 << 22];
    rows.push(run_case(
        "firestorm-r9-disturbance",
        v,
        &motor,
        base_mission,
        &wind,
        e,
        a,
        f,
        &out,
        true,
    ));
    let v = parse_spatial_vehicle_pack(&fixed::<KVP8_LENGTH>(
        root.join("phase9_5/examples/firestorm-m9.kvp8"),
    ))
    .unwrap();
    let e = parse_effector_pack(&fixed::<KPE9_LENGTH>(
        root.join("phase9_5/examples/firestorm-m9.kpe9"),
    ))
    .unwrap();
    let a = parse_allocator_pack(&fixed::<KPA9_LENGTH>(
        root.join("phase9_5/examples/firestorm-m9.kpa9"),
    ))
    .unwrap();
    let mut f = AdvancedMissionFaults::NOMINAL;
    f.pitot_dropout_start = 32;
    f.pitot_dropout_epochs = 64;
    rows.push(run_case(
        "firestorm-m9-airdata-loss",
        v,
        &motor,
        base_mission,
        &wind,
        e,
        a,
        f,
        &out,
        false,
    ));
    let mut h = AdvancedMissionFaults::NOMINAL;
    h.canards[0] = CanardFaultMode::HardoverPositive;
    rows.push(run_case(
        "firestorm-m9-canard-hardover",
        v,
        &motor,
        base_mission,
        &wind,
        e,
        a,
        h,
        &out,
        false,
    ));
    let mut s = AdvancedMissionFaults::NOMINAL;
    s.rcs[0] = RcsJetFault::StuckOpen;
    rows.push(run_case(
        "firestorm-m9-stuck-open",
        v,
        &motor,
        base_mission,
        &wind,
        e,
        a,
        s,
        &out,
        false,
    ));
    assert_acceptance(&rows);
    fs::write(
        out.join("integrated-cases-v1.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
    println!("wrote {} integrated cases to {}", rows.len(), out.display());
}
