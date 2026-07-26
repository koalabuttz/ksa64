use crc32fast::Hasher;
use ksa64_flight::phase10::ksa_g10r_reference_flight_config;
use ksa64_flight::phase11::{
    ksa_g10r_reference_mission_plan, GlobalKlr10FlightPackage, KsaG10rReferenceOpsV1,
    KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
};
use ksa64_interface::phase10::{
    write_global_aid_frame, write_global_command, write_global_fast_sensor, write_global_status,
    GlobalAidFrameCell, GlobalFastSensorCell, GlobalFrameId, GLOBAL_AID_VALID_MASK,
    GLOBAL_COMMAND_LENGTH, GLOBAL_FAST_ATTITUDE, GLOBAL_FAST_DELTA_ANGLE, GLOBAL_FAST_DELTA_V,
    GLOBAL_STATUS_LENGTH,
};
use ksa64_interface::phase11::{
    write_kej11, write_kpd11, write_kua11, write_kul11, EventJournalRecord, FlightAbiId,
    UplinkCommandLoad, UplinkControlKind, UplinkControlRecord, UplinkLoadType, UplinkReasonCode,
    UplinkState, KEJ11_LENGTH, KPD11_LENGTH, KUA11_LENGTH, KUL11_LENGTH,
    PACKAGE_CAP_HIGH_LEVEL_MODE,
};
use serde_json::json;
use std::env;
use std::fs;

const RECORD_LENGTH: usize = 1_056;
const PAYLOAD_LENGTH: usize = 512;
const OP_RELEASE: u8 = 1;
const OP_STAGE: u8 = 2;
const OP_COMMIT: u8 = 3;
const OP_GROUND_COMMS: u8 = 5;
const OP_PREDICTION: u8 = 6;
const OP_JOURNAL: u8 = 7;

#[derive(Clone)]
struct Record {
    operation: u8,
    flags: u8,
    available: bool,
    aux: u32,
    input_length: u16,
    output_length: u16,
    epoch: u16,
    navigation_checksum: u32,
    flight_checksum: u32,
    command_checksum: u32,
    input: [u8; PAYLOAD_LENGTH],
    output: [u8; PAYLOAD_LENGTH],
}

fn main() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("usage: phase11_reference_ops_fixture TRANSCRIPT.bin EVIDENCE.json".into());
    }
    let mut package = KsaG10rReferenceOpsV1::new(ksa_g10r_reference_flight_config())
        .ok_or("reference package initialization failed")?;
    if !package.initialize_mission_plan(ksa_g10r_reference_mission_plan()) {
        return Err("reference mission-plan initialization failed".into());
    }
    let mut records = Vec::new();
    let first = package.process_release(Some(fast(0)), Some(aid(0)), None);
    let mut result = (
        first.command.source_epoch,
        first.navigation.checksum,
        first.flight_checksum,
        first.command.command_checksum,
    );
    records.push(release_record(0, Some(aid(0)), &first, result));

    let prediction = package
        .prediction_summary()
        .ok_or("initial prediction missing")?;
    let mut output = [0; PAYLOAD_LENGTH];
    write_kpd11(&prediction, &mut output[..KPD11_LENGTH]).map_err(debug)?;
    records.push(record(
        OP_PREDICTION,
        0,
        true,
        0,
        0,
        KPD11_LENGTH,
        result,
        [0; 512],
        output,
    ));

    let load = safe_load();
    let mut input = [0; PAYLOAD_LENGTH];
    write_kul11(&load, &mut input[..KUL11_LENGTH]).map_err(debug)?;
    let staged = package.stage_uplink(load, 0).ok_or("stage failed")?;
    let mut output = [0; PAYLOAD_LENGTH];
    write_kua11(&staged, &mut output[..KUA11_LENGTH]).map_err(debug)?;
    records.push(record(
        OP_STAGE,
        0,
        true,
        0,
        KUL11_LENGTH,
        KUA11_LENGTH,
        result,
        input,
        output,
    ));

    let commit = commit_request(load);
    let mut input = [0; PAYLOAD_LENGTH];
    write_kua11(&commit, &mut input[..KUA11_LENGTH]).map_err(debug)?;
    let committed = package.commit_uplink(&commit).ok_or("commit failed")?;
    let mut output = [0; PAYLOAD_LENGTH];
    write_kua11(&committed, &mut output[..KUA11_LENGTH]).map_err(debug)?;
    records.push(record(
        OP_COMMIT,
        0,
        true,
        0,
        KUA11_LENGTH,
        KUA11_LENGTH,
        result,
        input,
        output,
    ));

    package.record_ground_communications(false);
    records.push(record(
        OP_GROUND_COMMS,
        0,
        false,
        0,
        0,
        0,
        result,
        [0; 512],
        [0; 512],
    ));

    for epoch in 1..=4u16 {
        let aid_cell = (epoch == 4).then(|| aid(epoch));
        let evidence = package.process_release(Some(fast(epoch)), aid_cell, None);
        result = (
            evidence.command.source_epoch,
            evidence.navigation.checksum,
            evidence.flight_checksum,
            evidence.command.command_checksum,
        );
        records.push(release_record(epoch, aid_cell, &evidence, result));
    }

    package.record_ground_communications(true);
    records.push(record(
        OP_GROUND_COMMS,
        1,
        false,
        0,
        0,
        0,
        result,
        [0; 512],
        [0; 512],
    ));

    let mut journal = [EventJournalRecord::EMPTY; 1];
    if package.recover_journal_after(0, &mut journal) != 1 {
        return Err("journal recovery produced no first record".into());
    }
    let mut output = [0; PAYLOAD_LENGTH];
    write_kej11(&journal[0], &mut output[..KEJ11_LENGTH]).map_err(debug)?;
    records.push(record(
        OP_JOURNAL,
        0,
        true,
        0,
        0,
        KEJ11_LENGTH,
        result,
        [0; 512],
        output,
    ));
    let first_journal_sequence = journal[0].sequence;

    if package.recover_journal_after(first_journal_sequence, &mut journal) != 1 {
        return Err("journal recovery produced no successor record".into());
    }
    let mut output = [0; PAYLOAD_LENGTH];
    write_kej11(&journal[0], &mut output[..KEJ11_LENGTH]).map_err(debug)?;
    records.push(record(
        OP_JOURNAL,
        0,
        true,
        first_journal_sequence,
        0,
        KEJ11_LENGTH,
        result,
        [0; 512],
        output,
    ));

    let prediction = package
        .prediction_summary()
        .ok_or("final prediction missing")?;
    let mut output = [0; PAYLOAD_LENGTH];
    write_kpd11(&prediction, &mut output[..KPD11_LENGTH]).map_err(debug)?;
    records.push(record(
        OP_PREDICTION,
        0,
        true,
        0,
        0,
        KPD11_LENGTH,
        result,
        [0; 512],
        output,
    ));

    let transcript = encode(&records);
    fs::write(&arguments[0], &transcript).map_err(|error| error.to_string())?;
    let evidence = json!({
        "schema": "ksa64.phase11.reference-ops-endpoint-vectors.v1",
        "records": records.len(),
        "transcript_bytes": transcript.len(),
        "transcript_crc32": format!("{:08x}", crc32(&transcript)),
        "final_epoch": result.0,
        "navigation_checksum": format!("{:08x}", result.1),
        "flight_checksum": format!("{:08x}", result.2),
        "command_checksum": format!("{:08x}", result.3),
        "first_journal_sequence": first_journal_sequence,
        "coverage": [
            "ordinary release", "8 Hz aided release", "prediction", "stage", "commit",
            "ground blackout", "ground reacquisition", "journal recovery"
        ]
    });
    fs::write(
        &arguments[1],
        serde_json::to_vec_pretty(&evidence).map_err(debug)?,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).map_err(debug)?
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record(
    operation: u8,
    flags: u8,
    available: bool,
    aux: u32,
    input_length: usize,
    output_length: usize,
    result: (u16, u32, u32, u32),
    input: [u8; PAYLOAD_LENGTH],
    output: [u8; PAYLOAD_LENGTH],
) -> Record {
    Record {
        operation,
        flags,
        available,
        aux,
        input_length: input_length as u16,
        output_length: output_length as u16,
        epoch: result.0,
        navigation_checksum: result.1,
        flight_checksum: result.2,
        command_checksum: result.3,
        input,
        output,
    }
}

fn release_record(
    epoch: u16,
    aid_cell: Option<GlobalAidFrameCell>,
    evidence: &ksa64_flight::phase10::GlobalFlightEvidence,
    result: (u16, u32, u32, u32),
) -> Record {
    let mut input = [0; PAYLOAD_LENGTH];
    write_global_fast_sensor(&fast(epoch), &mut input[..64]).unwrap();
    let flags = if let Some(aid) = aid_cell {
        write_global_aid_frame(&aid, &mut input[64..160]).unwrap();
        1
    } else {
        0
    };
    let mut output = [0; PAYLOAD_LENGTH];
    write_global_command(&evidence.command, &mut output[..GLOBAL_COMMAND_LENGTH]).unwrap();
    let output_length = if let Some(status) = evidence.status {
        write_global_status(
            &status,
            &mut output[GLOBAL_COMMAND_LENGTH..GLOBAL_COMMAND_LENGTH + GLOBAL_STATUS_LENGTH],
        )
        .unwrap();
        GLOBAL_COMMAND_LENGTH + GLOBAL_STATUS_LENGTH
    } else {
        GLOBAL_COMMAND_LENGTH
    };
    record(
        OP_RELEASE,
        flags,
        evidence.status.is_some(),
        0,
        if aid_cell.is_some() { 160 } else { 64 },
        output_length,
        result,
        input,
        output,
    )
}

fn fast(epoch: u16) -> GlobalFastSensorCell {
    GlobalFastSensorCell {
        session: 0x10a0,
        measurement_epoch: epoch,
        production_epoch: epoch,
        frame: GlobalFrameId::LocalEnuV1,
        validity: GLOBAL_FAST_DELTA_V | GLOBAL_FAST_DELTA_ANGLE | GLOBAL_FAST_ATTITUDE,
        mission_time_q16: u32::from(epoch) * 2_048,
        delta_velocity_q24: [0, 0, 1],
        delta_angle_q24: [0; 3],
        attitude_vector_q15: [0; 3],
        angular_rate_q15: [0; 3],
        dynamic_pressure_q10: 0,
        mach_q12: 0,
        gimbal_applied_q15: [0; 2],
        rcs_propellant_q21: 5 << 21,
        actuator_feedback: 0,
        vehicle_status: 2,
        sensor_checksum: epoch,
    }
}

fn aid(epoch: u16) -> GlobalAidFrameCell {
    let config = ksa_g10r_reference_flight_config();
    GlobalAidFrameCell {
        session: config.session,
        measurement_epoch: epoch,
        production_epoch: epoch,
        frame: GlobalFrameId::LocalEnuV1,
        validity: GLOBAL_AID_VALID_MASK,
        mission_time_q16: u32::from(epoch) * 2_048,
        barometer_q12_km: 0,
        gnss_position_q12_km: [0; 3],
        gnss_velocity_q24_km_s: [0; 3],
        attitude_q30: config.initial_attitude_q30,
        frame_rotation_q30: [1 << 30, 0, 0, 0],
        frame_omega_q24: [0; 3],
        events: 0,
        continuity: 1,
        deployment_feedback: 0,
    }
}

fn safe_load() -> UplinkCommandLoad {
    let mut arguments = [0; 16];
    arguments[0] = 2;
    UplinkCommandLoad {
        load_identity: 0x11c0_1001,
        package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
        plan_identity: ksa_g10r_reference_mission_plan().plan_identity,
        abi: FlightAbiId::GlobalKlr10V1,
        source_estimator_identity: 0x11e0_1001,
        source_estimator_checksum: 0x1234_5678,
        stage_epoch: 0,
        not_before_epoch: 2,
        expires_epoch: 12,
        requested_effective_epoch: 4,
        required_capabilities: PACKAGE_CAP_HIGH_LEVEL_MODE,
        prerequisite_event_mask: 0,
        position_residual_limit_q12: 0,
        velocity_residual_limit_q24: 0,
        frame: GlobalFrameId::LocalEnuV1,
        load_type: UplinkLoadType::HighLevelMode,
        arguments,
    }
}

fn commit_request(load: UplinkCommandLoad) -> UplinkControlRecord {
    UplinkControlRecord {
        kind: UplinkControlKind::CommitRequest,
        control_identity: 0x11c1_1001,
        load_identity: load.load_identity,
        package_manifest_identity: load.package_manifest_identity,
        plan_identity: load.plan_identity,
        request_epoch: 0,
        effective_epoch: load.requested_effective_epoch,
        state: UplinkState::Staged,
        reason: UplinkReasonCode::Accepted,
        receipt_checksum: 0,
    }
}

fn encode(records: &[Record]) -> Vec<u8> {
    let mut bytes = vec![0; 16 + records.len() * RECORD_LENGTH];
    bytes[..4].copy_from_slice(b"KOT1");
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    bytes[6..8].copy_from_slice(&(RECORD_LENGTH as u16).to_le_bytes());
    bytes[8..10].copy_from_slice(&(records.len() as u16).to_le_bytes());
    bytes[10..12].copy_from_slice(&16u16.to_le_bytes());
    for (index, record) in records.iter().enumerate() {
        let start = 16 + index * RECORD_LENGTH;
        let target = &mut bytes[start..start + RECORD_LENGTH];
        target[0] = record.operation;
        target[1] = record.flags;
        target[2] = u8::from(record.available);
        target[4..8].copy_from_slice(&record.aux.to_le_bytes());
        target[8..10].copy_from_slice(&record.input_length.to_le_bytes());
        target[10..12].copy_from_slice(&record.output_length.to_le_bytes());
        target[12..14].copy_from_slice(&record.epoch.to_le_bytes());
        target[16..20].copy_from_slice(&record.navigation_checksum.to_le_bytes());
        target[20..24].copy_from_slice(&record.flight_checksum.to_le_bytes());
        target[24..28].copy_from_slice(&record.command_checksum.to_le_bytes());
        target[28..32]
            .copy_from_slice(&crc32(&record.output[..record.output_length as usize]).to_le_bytes());
        target[32..544].copy_from_slice(&record.input);
        target[544..1_056].copy_from_slice(&record.output);
    }
    let checksum = crc32(&bytes[16..]);
    bytes[12..16].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

fn debug(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
