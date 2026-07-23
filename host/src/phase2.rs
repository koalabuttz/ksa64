use std::io::{self, Write};

use ksa64_core::phase2_mission::{
    EVENT_CUTOFF, EVENT_END, EVENT_IGNITION, EVENT_IMPACT, EVENT_SEPARATION, PLANAR_CHECKSUM_OFFSET,
};
use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_core::phase2_telemetry::{
    parse_phase2_telemetry_frame, parse_phase2_telemetry_header_for_scenario,
    run_phase2_mission_with_telemetry, validate_phase2_telemetry_frame_numeric,
    Phase2TelemetryFailure, Phase2TelemetryFrame, Phase2TelemetryHeader, Phase2TelemetryReadError,
    Phase2TelemetrySink, Phase2TelemetrySummary, PHASE2_TELEMETRY_FRAME_LENGTH,
    PHASE2_TELEMETRY_HEADER_LENGTH,
};
use ksa64_core::scenario::crc32_ieee;

pub struct WriterPhase2TelemetrySink<'a, W: Write> {
    writer: &'a mut W,
    frames_written: u32,
    bytes_written: usize,
}

impl<'a, W: Write> WriterPhase2TelemetrySink<'a, W> {
    pub fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            frames_written: 0,
            bytes_written: 0,
        }
    }
    pub const fn frames_written(&self) -> u32 {
        self.frames_written
    }
    pub const fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}

impl<W: Write> Phase2TelemetrySink for WriterPhase2TelemetrySink<'_, W> {
    type Error = io::Error;
    fn write_header(
        &mut self,
        header: &[u8; PHASE2_TELEMETRY_HEADER_LENGTH],
    ) -> Result<(), Self::Error> {
        self.writer.write_all(header)?;
        self.bytes_written += header.len();
        Ok(())
    }
    fn write_frame(
        &mut self,
        frame: &[u8; PHASE2_TELEMETRY_FRAME_LENGTH],
    ) -> Result<(), Self::Error> {
        self.writer.write_all(frame)?;
        self.frames_written += 1;
        self.bytes_written += frame.len();
        Ok(())
    }
}

pub fn capture_phase2_mission<W: Write>(
    scenario: &Phase2Scenario,
    writer: &mut W,
) -> Result<Phase2TelemetrySummary, Phase2TelemetryFailure<io::Error>> {
    let mut sink = WriterPhase2TelemetrySink::new(writer);
    run_phase2_mission_with_telemetry(scenario, &mut sink)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase2StreamInspectionError {
    Framing,
    Empty,
    Header(Phase2TelemetryReadError),
    Frame {
        index: usize,
        cause: Phase2TelemetryReadError,
    },
    InitialFrame,
    StepOrder {
        index: usize,
    },
    StepRange {
        index: usize,
    },
    Stride {
        index: usize,
    },
    MissionTime {
        index: usize,
    },
    NumericRange {
        index: usize,
    },
    TerminalBeforeEnd {
        index: usize,
    },
    MissingTerminal,
    SuccessfulEndStep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase2StreamInspection {
    header: Phase2TelemetryHeader,
    frame_count: usize,
    stream_bytes: usize,
    stream_crc32: u32,
    first_frame: Phase2TelemetryFrame,
    final_frame: Phase2TelemetryFrame,
    ignition_event_frames: u32,
    cutoff_event_frames: u32,
    separation_event_frames: u32,
    impact_event_frames: u32,
}

impl Phase2StreamInspection {
    pub const fn header(self) -> Phase2TelemetryHeader {
        self.header
    }
    pub const fn frame_count(self) -> usize {
        self.frame_count
    }
    pub const fn stream_bytes(self) -> usize {
        self.stream_bytes
    }
    pub const fn stream_crc32(self) -> u32 {
        self.stream_crc32
    }
    pub const fn first_frame(self) -> Phase2TelemetryFrame {
        self.first_frame
    }
    pub const fn final_frame(self) -> Phase2TelemetryFrame {
        self.final_frame
    }
    pub const fn ignition_event_frames(self) -> u32 {
        self.ignition_event_frames
    }
    pub const fn cutoff_event_frames(self) -> u32 {
        self.cutoff_event_frames
    }
    pub const fn separation_event_frames(self) -> u32 {
        self.separation_event_frames
    }
    pub const fn impact_event_frames(self) -> u32 {
        self.impact_event_frames
    }
}

pub fn inspect_phase2_stream(
    stream: &[u8],
    scenario: &Phase2Scenario,
) -> Result<Phase2StreamInspection, Phase2StreamInspectionError> {
    let payload = stream.len().saturating_sub(PHASE2_TELEMETRY_HEADER_LENGTH);
    let complete = (payload / PHASE2_TELEMETRY_FRAME_LENGTH) * PHASE2_TELEMETRY_FRAME_LENGTH;
    if stream.len() < PHASE2_TELEMETRY_HEADER_LENGTH || complete != payload {
        return Err(Phase2StreamInspectionError::Framing);
    }
    let frame_count =
        (stream.len() - PHASE2_TELEMETRY_HEADER_LENGTH) / PHASE2_TELEMETRY_FRAME_LENGTH;
    if frame_count == 0 {
        return Err(Phase2StreamInspectionError::Empty);
    }
    let header = parse_phase2_telemetry_header_for_scenario(
        &stream[..PHASE2_TELEMETRY_HEADER_LENGTH],
        scenario,
    )
    .map_err(Phase2StreamInspectionError::Header)?;
    let mut first = None;
    let mut final_frame = None;
    let mut prior_step = 0u32;
    let mut ignition = 0u32;
    let mut cutoff = 0u32;
    let mut separation = 0u32;
    let mut impact = 0u32;
    for index in 0..frame_count {
        let start = PHASE2_TELEMETRY_HEADER_LENGTH + index * PHASE2_TELEMETRY_FRAME_LENGTH;
        let frame =
            parse_phase2_telemetry_frame(&stream[start..start + PHASE2_TELEMETRY_FRAME_LENGTH])
                .map_err(|cause| Phase2StreamInspectionError::Frame { index, cause })?;
        if !validate_phase2_telemetry_frame_numeric(frame) {
            return Err(Phase2StreamInspectionError::NumericRange { index });
        }
        let terminal = frame.events() & EVENT_END != 0;
        if index == 0 {
            let mut status = ksa64_core::numeric::NumericStatus::CLEAR;
            let initial = scenario
                .initial_truth(&mut status)
                .ok_or(Phase2StreamInspectionError::InitialFrame)?;
            if !status.is_clear()
                || frame.step() != 0
                || frame.time().raw() != 0
                || frame.radius() != initial.radius()
                || frame.downrange() != initial.downrange()
                || frame.radial_velocity() != initial.radial_velocity()
                || frame.specific_angular_momentum() != initial.specific_angular_momentum()
                || frame.total_mass() != initial.total_mass()
                || frame.propellant() != initial.active_propellant()
                || frame.active_stage() != 0
                || frame.stage_phase() != initial.stage_phase()
                || frame.events() != 0
                || frame.state_checksum() != PLANAR_CHECKSUM_OFFSET
            {
                return Err(Phase2StreamInspectionError::InitialFrame);
            }
            first = Some(frame);
        } else {
            if frame.step() <= prior_step {
                return Err(Phase2StreamInspectionError::StepOrder { index });
            }
            let stride = header.telemetry_stride() as u32;
            if !terminal && (frame.step() / stride) * stride != frame.step() {
                return Err(Phase2StreamInspectionError::Stride { index });
            }
        }
        if frame.step() > scenario.steps() {
            return Err(Phase2StreamInspectionError::StepRange { index });
        }
        if i64::from(frame.time().raw())
            != i64::from(header.timestep().raw()) * i64::from(frame.step())
        {
            return Err(Phase2StreamInspectionError::MissionTime { index });
        }
        if terminal && index + 1 != frame_count {
            return Err(Phase2StreamInspectionError::TerminalBeforeEnd { index });
        }
        ignition += u32::from(frame.events() & EVENT_IGNITION != 0);
        cutoff += u32::from(frame.events() & EVENT_CUTOFF != 0);
        separation += u32::from(frame.events() & EVENT_SEPARATION != 0);
        impact += u32::from(frame.events() & EVENT_IMPACT != 0);
        prior_step = frame.step();
        final_frame = Some(frame);
    }
    let first_frame = first.ok_or(Phase2StreamInspectionError::Empty)?;
    let final_frame = final_frame.ok_or(Phase2StreamInspectionError::Empty)?;
    if final_frame.events() & EVENT_END == 0 {
        return Err(Phase2StreamInspectionError::MissingTerminal);
    }
    if final_frame.events() & EVENT_IMPACT == 0 && final_frame.step() != scenario.steps() {
        return Err(Phase2StreamInspectionError::SuccessfulEndStep);
    }
    Ok(Phase2StreamInspection {
        header,
        frame_count,
        stream_bytes: stream.len(),
        stream_crc32: crc32_ieee(stream),
        first_frame,
        final_frame,
        ignition_event_frames: ignition,
        cutoff_event_frames: cutoff,
        separation_event_frames: separation,
        impact_event_frames: impact,
    })
}

pub fn format_phase2_inspection(inspection: Phase2StreamInspection) -> String {
    let frame = inspection.final_frame();
    let altitude = f64::from(frame.radius().raw() - EARTH_RADIUS_Q12) / 4096.0;
    format!(
        concat!(
            "KSA64 TELEMETRY V2\n",
            "scenario       0x{scenario:08x}\n",
            "frames         {frames}\n",
            "stream         {bytes} bytes, CRC32 0x{stream_crc:08x}\n",
            "final step     {step}\n",
            "mission time   {time:.3} s\n",
            "altitude       {altitude:.6} km\n",
            "radial speed   {radial:.6} km/s\n",
            "stage          {stage} ({phase:?})\n",
            "mass           {mass:.6} t\n",
            "propellant     {propellant:.6} t\n",
            "state checksum 0x{checksum:08x}\n",
            "event frames   ignition={ignition}, cutoff={cutoff}, separation={separation}, impact={impact}\n"
        ),
        scenario = inspection.header().scenario_id(), frames = inspection.frame_count(),
        bytes = inspection.stream_bytes(), stream_crc = inspection.stream_crc32(), step = frame.step(),
        time = f64::from(frame.time().raw()) / 65_536.0, altitude = altitude,
        radial = f64::from(frame.radial_velocity().raw()) / 16_777_216.0,
        stage = frame.active_stage() + 1, phase = frame.stage_phase(),
        mass = f64::from(frame.total_mass().raw()) / 4096.0,
        propellant = f64::from(frame.propellant().raw()) / 4096.0,
        checksum = frame.state_checksum(), ignition = inspection.ignition_event_frames(),
        cutoff = inspection.cutoff_event_frames(), separation = inspection.separation_event_frames(),
        impact = inspection.impact_event_frames(),
    )
}
