//! Phase 8 KST8 capture and KSR8 generation over the portable mission machine.
use ksa64_core::evaluation::EvaluationSummary;
use ksa64_core::phase8_format::{KSR8_LENGTH, KST8_FRAME_LENGTH, KST8_HEADER_LENGTH};
use ksa64_core::phase8_mission::{
    Phase8MissionError, Phase8MissionMachine, Phase8MissionSnapshot, SpatialMissionVariation,
};
use ksa64_core::phase8_pack::{
    SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack, WindProfilePack,
};
use ksa64_core::phase8_result::{encode_ksr8, Ksr8Error};
use ksa64_core::phase8_telemetry::{encode_kst8_frame, encode_kst8_header, Kst8Error};
use ksa64_sim::evaluation::{evaluate, EvaluationError, EvaluationRequest};
#[derive(Debug)]
pub enum Phase8CaptureError {
    Mission(Phase8MissionError),
    Telemetry(Kst8Error),
    Evaluation(EvaluationError),
    Summary(Ksr8Error),
}
pub struct Phase8Capture {
    pub evaluation: EvaluationSummary,
    pub telemetry: Vec<u8>,
    pub summary_record: [u8; KSR8_LENGTH],
}
fn mix(mut h: u32, s: Phase8MissionSnapshot) -> u32 {
    for v in [
        s.state.time.raw(),
        s.state.position.x(),
        s.state.position.y(),
        s.state.position.z(),
        s.state.velocity.x(),
        s.state.velocity.y(),
        s.state.velocity.z(),
        s.phase as i32,
        s.events as i32,
    ] {
        h = (h ^ v as u32).wrapping_mul(16_777_619)
    }
    h
}
pub fn capture_spatial_mission(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    mission: SpatialMissionPack,
    wind: &WindProfilePack,
    variation: SpatialMissionVariation,
    variation_checksum: u32,
) -> Result<Phase8Capture, Phase8CaptureError> {
    let mut header = [0u8; KST8_HEADER_LENGTH];
    encode_kst8_header(
        vehicle,
        motor,
        mission,
        wind,
        variation_checksum,
        &mut header,
    )
    .map_err(Phase8CaptureError::Telemetry)?;
    let mut telemetry = header.to_vec();
    let mut machine =
        Phase8MissionMachine::new_with_variation(vehicle, motor, mission, wind, variation)
            .map_err(Phase8CaptureError::Mission)?;
    let mut step = 0u32;
    let mut next_time = 0i32;
    let mut checksum = 0x811c_9dc5u32;
    let initial = machine.snapshot();
    let mut frame = [0u8; KST8_FRAME_LENGTH];
    encode_kst8_frame(initial, step, checksum, &mut frame)
        .map_err(Phase8CaptureError::Telemetry)?;
    telemetry.extend_from_slice(&frame);
    next_time += mission.telemetry_period.raw();
    while !machine.is_complete() {
        match machine.step() {
            Ok(snapshot) => {
                step = step.saturating_add(1);
                checksum = mix(checksum, snapshot);
                if snapshot.events != 0 || snapshot.state.time.raw() >= next_time {
                    encode_kst8_frame(snapshot, step, checksum, &mut frame)
                        .map_err(Phase8CaptureError::Telemetry)?;
                    telemetry.extend_from_slice(&frame);
                    while next_time <= snapshot.state.time.raw() {
                        next_time += mission.telemetry_period.raw();
                    }
                }
            }
            Err(Phase8MissionError::Complete | Phase8MissionError::ModelEnvelopeExceeded) => {}
            Err(error) => return Err(Phase8CaptureError::Mission(error)),
        }
    }
    let evaluation = evaluate(EvaluationRequest::HobbySpatialV1 {
        vehicle,
        motor,
        mission,
        wind,
        variation,
        variation_checksum,
    })
    .map_err(Phase8CaptureError::Evaluation)?;
    let mut summary_record = [0u8; KSR8_LENGTH];
    encode_ksr8(evaluation, &mut summary_record).map_err(Phase8CaptureError::Summary)?;
    Ok(Phase8Capture {
        evaluation,
        telemetry,
        summary_record,
    })
}
pub fn telemetry_frame_count(capture: &Phase8Capture) -> usize {
    (capture.telemetry.len() - KST8_HEADER_LENGTH) / KST8_FRAME_LENGTH
}
