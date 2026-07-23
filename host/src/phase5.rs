use ksa64_core::phase5_contract::{PHASE5_MISSION_STEPS, PHASE5_MISSION_STEP_Q16};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase5_mission::{Phase5MissionCase, Phase5MissionSummary};
use ksa64_sim::phase5_telemetry::{
    parse_phase5_telemetry_frame, parse_phase5_telemetry_header, run_phase5_mission_with_telemetry,
    Phase5TelemetryError, Phase5TelemetryFrame, Phase5TelemetryHeader, Phase5TelemetrySink,
    PHASE5_AVIONICS_SIGNATURE, PHASE5_FRAME_EVENT, PHASE5_TELEMETRY_FRAME_LENGTH,
    PHASE5_TELEMETRY_HEADER_LENGTH,
};

#[derive(Default)]
struct VecSink {
    bytes: Vec<u8>,
}
impl Phase5TelemetrySink for VecSink {
    type Error = core::convert::Infallible;
    fn write_header(
        &mut self,
        v: &[u8; PHASE5_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error> {
        self.bytes.extend_from_slice(v);
        Ok(())
    }
    fn write_frame(&mut self, v: &[u8; PHASE5_TELEMETRY_FRAME_LENGTH]) -> Result<(), Self::Error> {
        self.bytes.extend_from_slice(v);
        Ok(())
    }
}
pub fn capture_phase5_mission(case: Phase5MissionCase) -> (Phase5MissionSummary, Vec<u8>) {
    let mut sink = VecSink::default();
    let summary =
        run_phase5_mission_with_telemetry(case, &mut sink).expect("infallible host capture");
    (summary, sink.bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5StreamError {
    Framing,
    Empty,
    Header(Phase5TelemetryError),
    HeaderIdentity,
    Frame {
        index: usize,
        cause: Phase5TelemetryError,
    },
    Initial,
    Step {
        index: usize,
    },
    Time {
        index: usize,
    },
    Flags {
        index: usize,
    },
    Checksum {
        index: usize,
    },
    Terminal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5StreamInspection {
    pub header: Phase5TelemetryHeader,
    pub frame_count: usize,
    pub stream_bytes: usize,
    pub stream_crc32: u32,
    pub event_frames: u32,
    pub final_frame: Phase5TelemetryFrame,
}

pub fn inspect_phase5_stream(stream: &[u8]) -> Result<Phase5StreamInspection, Phase5StreamError> {
    if stream.len() < PHASE5_TELEMETRY_HEADER_LENGTH
        || (stream.len() - PHASE5_TELEMETRY_HEADER_LENGTH) % PHASE5_TELEMETRY_FRAME_LENGTH != 0
    {
        return Err(Phase5StreamError::Framing);
    }
    let count = (stream.len() - PHASE5_TELEMETRY_HEADER_LENGTH) / PHASE5_TELEMETRY_FRAME_LENGTH;
    if count == 0 {
        return Err(Phase5StreamError::Empty);
    }
    let header = parse_phase5_telemetry_header(&stream[..PHASE5_TELEMETRY_HEADER_LENGTH])
        .map_err(Phase5StreamError::Header)?;
    if header.seed != 0x5a00_0000 | header.case as u32
        || header.vehicle_signature != ksa64_sim::phase5_vehicle::PHASE5_VEHICLE_SIGNATURE
        || header.avionics_signature != PHASE5_AVIONICS_SIGNATURE
        || header.guidance_signature != ksa64_sim::flight::phase5_guidance::GUIDANCE_SIGNATURE
    {
        return Err(Phase5StreamError::HeaderIdentity);
    }
    let mut chain = 2_166_136_261u32;
    let mut events = 0;
    let mut final_frame = None;
    for index in 0..count {
        let start = PHASE5_TELEMETRY_HEADER_LENGTH + index * PHASE5_TELEMETRY_FRAME_LENGTH;
        let bytes = &stream[start..start + PHASE5_TELEMETRY_FRAME_LENGTH];
        let frame = parse_phase5_telemetry_frame(bytes)
            .map_err(|cause| Phase5StreamError::Frame { index, cause })?;
        if frame.step != index as u32 || frame.step > PHASE5_MISSION_STEPS {
            return Err(Phase5StreamError::Step { index });
        }
        if frame.mission_time_q16 != PHASE5_MISSION_STEP_Q16.saturating_mul(frame.step as i32) {
            return Err(Phase5StreamError::Time { index });
        }
        if index == 0
            && (frame.step != 0
                || frame.mission_time_q16 != 0
                || frame.terminal()
                || frame.events != 0)
        {
            return Err(Phase5StreamError::Initial);
        }
        let terminal = frame.terminal();
        if terminal != (index + 1 == count) {
            return Err(Phase5StreamError::Terminal);
        }
        if frame.event_record() != (frame.events != 0) {
            return Err(Phase5StreamError::Flags { index });
        }
        chain = hash_bytes(chain, &bytes[..412]);
        if frame.observation_checksum != chain {
            return Err(Phase5StreamError::Checksum { index });
        }
        events += u32::from(frame.frame_flags & PHASE5_FRAME_EVENT != 0);
        final_frame = Some(frame);
    }
    Ok(Phase5StreamInspection {
        header,
        frame_count: count,
        stream_bytes: stream.len(),
        stream_crc32: crc32_ieee(stream),
        event_frames: events,
        final_frame: final_frame.ok_or(Phase5StreamError::Empty)?,
    })
}
fn hash_bytes(mut h: u32, b: &[u8]) -> u32 {
    for x in b {
        h ^= *x as u32;
        h = h.wrapping_mul(16_777_619)
    }
    h
}
