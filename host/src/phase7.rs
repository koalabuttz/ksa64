//! Host capture and artifact generation for Phase 7 hobby missions.

use ksa64_core::evaluation::EvaluationSummary;
use ksa64_core::phase7_format::{KSR7_LENGTH, KST7_FRAME_LENGTH, KST7_HEADER_LENGTH};
use ksa64_core::phase7_mission::{
    execute_hobby_mission_observed, HobbyMissionExecutionError, HobbyMissionObservation,
    HobbyMissionObserver,
};
use ksa64_core::phase7_pack::{HobbyMissionPack, MotorPack, VerticalVehiclePack};
use ksa64_core::phase7_result::{encode_ksr7, Ksr7Error};
use ksa64_core::phase7_telemetry::{
    encode_kst7_frame, encode_kst7_header, HobbyTelemetryFrame, Kst7Error,
};
use ksa64_sim::evaluation::{evaluate, EvaluationError, EvaluationRequest};

#[derive(Debug)]
pub enum Phase7CaptureError {
    Evaluation(EvaluationError),
    Mission,
    Telemetry(Kst7Error),
    Summary(Ksr7Error),
}

struct CaptureObserver {
    bytes: Vec<u8>,
    next_time_raw: i32,
    period_raw: i32,
}

impl HobbyMissionObserver for CaptureObserver {
    type Error = Kst7Error;

    fn observe(&mut self, observation: HobbyMissionObservation) -> Result<(), Self::Error> {
        let must_write = observation.state.step == 0
            || observation.events != 0
            || observation.state.time.raw() >= self.next_time_raw;
        if must_write {
            let mut bytes = [0u8; KST7_FRAME_LENGTH];
            encode_kst7_frame(HobbyTelemetryFrame { observation }, &mut bytes)?;
            self.bytes.extend_from_slice(&bytes);
            while self.next_time_raw <= observation.state.time.raw() {
                self.next_time_raw += self.period_raw;
            }
        }
        Ok(())
    }
}

pub struct Phase7Capture {
    pub evaluation: EvaluationSummary,
    pub telemetry: Vec<u8>,
    pub summary_record: [u8; KSR7_LENGTH],
}

pub fn capture_hobby_mission(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
) -> Result<Phase7Capture, Phase7CaptureError> {
    let evaluation = evaluate(EvaluationRequest::HobbyVerticalV1 {
        vehicle,
        motor,
        mission,
    })
    .map_err(Phase7CaptureError::Evaluation)?;
    let mut header = [0u8; KST7_HEADER_LENGTH];
    encode_kst7_header(vehicle, motor, mission, &mut header)
        .map_err(Phase7CaptureError::Telemetry)?;
    let mut observer = CaptureObserver {
        bytes: header.to_vec(),
        next_time_raw: 0,
        period_raw: mission.telemetry_period.raw(),
    };
    execute_hobby_mission_observed(vehicle, motor, mission, &mut observer).map_err(|error| {
        match error {
            HobbyMissionExecutionError::Configuration => Phase7CaptureError::Mission,
            HobbyMissionExecutionError::Observer(error) => Phase7CaptureError::Telemetry(error),
        }
    })?;
    let mut summary_record = [0u8; KSR7_LENGTH];
    encode_ksr7(evaluation, &mut summary_record).map_err(Phase7CaptureError::Summary)?;
    Ok(Phase7Capture {
        evaluation,
        telemetry: observer.bytes,
        summary_record,
    })
}

pub fn telemetry_frame_count(capture: &Phase7Capture) -> usize {
    (capture.telemetry.len() - KST7_HEADER_LENGTH) / KST7_FRAME_LENGTH
}
