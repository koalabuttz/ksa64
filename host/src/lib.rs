pub mod phase2;
pub mod phase3;
pub mod phase4;
pub mod phase4_export;
pub mod phase4_storage;
pub mod phase5;
pub mod phase5_campaign;
pub mod phase6;
pub mod phase6_audio;
pub mod phase6_runner;
pub mod phase6_session;
pub mod phase6_trajectory;
pub mod phase6_tui;
pub mod phase7_compiler;

pub mod phase7;
pub mod phase8_campaign;
pub mod phase8_capture;
pub mod phase8_compiler;
pub mod phase8_plot;
use std::io::{self, Write};
pub mod phase7_campaign;
pub mod phase7_plot;
pub mod phase7_reference;

use ksa64_core::mission::VERTICAL_CHECKSUM_OFFSET;
use ksa64_core::scenario::{crc32_ieee, Scenario};
use ksa64_core::telemetry::{
    parse_telemetry_frame, parse_telemetry_header_for_scenario,
    run_vertical_mission_with_telemetry, TelemetryEvents, TelemetryFrame, TelemetryHeader,
    TelemetryMissionFailure, TelemetryMissionSummary, TelemetryReadError, TelemetrySink,
    TELEMETRY_FRAME_LENGTH, TELEMETRY_HEADER_LENGTH,
};

pub struct WriterTelemetrySink<'a, W: Write> {
    writer: &'a mut W,
    frames_written: u32,
    bytes_written: usize,
}

impl<'a, W: Write> WriterTelemetrySink<'a, W> {
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

impl<W: Write> TelemetrySink for WriterTelemetrySink<'_, W> {
    type Error = io::Error;

    fn write_header(&mut self, header: &[u8; TELEMETRY_HEADER_LENGTH]) -> Result<(), Self::Error> {
        self.writer.write_all(header)?;
        self.bytes_written += header.len();
        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8; TELEMETRY_FRAME_LENGTH]) -> Result<(), Self::Error> {
        self.writer.write_all(frame)?;
        self.frames_written += 1;
        self.bytes_written += frame.len();
        Ok(())
    }
}

pub fn capture_mission<W: Write>(
    scenario: &Scenario,
    writer: &mut W,
) -> Result<TelemetryMissionSummary, TelemetryMissionFailure<io::Error>> {
    let mut sink = WriterTelemetrySink::new(writer);
    run_vertical_mission_with_telemetry(scenario, &mut sink)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamInspectionError {
    Framing,
    Empty,
    Header(TelemetryReadError),
    Frame {
        index: usize,
        cause: TelemetryReadError,
    },
    InitialStep,
    InitialFrame,
    StepRange {
        index: usize,
    },
    StepOrder {
        index: usize,
    },
    Stride {
        index: usize,
    },
    MissionTime {
        index: usize,
    },
    NumericFaultWithoutEnd {
        index: usize,
    },
    TerminalBeforeEnd {
        index: usize,
    },
    MissingTerminal,
    SuccessfulEndStep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamInspection {
    header: TelemetryHeader,
    frame_count: usize,
    stream_bytes: usize,
    stream_crc32: u32,
    first_frame: TelemetryFrame,
    final_frame: TelemetryFrame,
    cutoff_event_frames: u32,
    depletion_event_frames: u32,
    numeric_fault_event_frames: u32,
}

impl StreamInspection {
    pub const fn header(self) -> TelemetryHeader {
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

    pub const fn first_frame(self) -> TelemetryFrame {
        self.first_frame
    }

    pub const fn final_frame(self) -> TelemetryFrame {
        self.final_frame
    }

    pub const fn cutoff_event_frames(self) -> u32 {
        self.cutoff_event_frames
    }

    pub const fn depletion_event_frames(self) -> u32 {
        self.depletion_event_frames
    }

    pub const fn numeric_fault_event_frames(self) -> u32 {
        self.numeric_fault_event_frames
    }
}

pub fn inspect_stream(
    stream: &[u8],
    scenario: &Scenario,
) -> Result<StreamInspection, StreamInspectionError> {
    let payload_length = stream.len().saturating_sub(TELEMETRY_HEADER_LENGTH);
    let complete_frames = (payload_length / TELEMETRY_FRAME_LENGTH) * TELEMETRY_FRAME_LENGTH;
    if stream.len() < TELEMETRY_HEADER_LENGTH || complete_frames != payload_length {
        return Err(StreamInspectionError::Framing);
    }
    let frame_count = (stream.len() - TELEMETRY_HEADER_LENGTH) / TELEMETRY_FRAME_LENGTH;
    if frame_count == 0 {
        return Err(StreamInspectionError::Empty);
    }
    let header = parse_telemetry_header_for_scenario(&stream[..TELEMETRY_HEADER_LENGTH], scenario)
        .map_err(StreamInspectionError::Header)?;

    let mut first_frame = None;
    let mut final_frame = None;
    let mut prior_step = 0u32;
    let mut cutoff_event_frames = 0u32;
    let mut depletion_event_frames = 0u32;
    let mut numeric_fault_event_frames = 0u32;

    for index in 0..frame_count {
        let start = TELEMETRY_HEADER_LENGTH + index * TELEMETRY_FRAME_LENGTH;
        let frame = parse_telemetry_frame(&stream[start..start + TELEMETRY_FRAME_LENGTH])
            .map_err(|cause| StreamInspectionError::Frame { index, cause })?;
        let events = frame.events().bits();
        let terminal = events & TelemetryEvents::END_OF_RUN != 0;
        let numeric_fault = events & TelemetryEvents::NUMERIC_FAULT != 0;

        if index == 0 {
            if frame.step() != 0 {
                return Err(StreamInspectionError::InitialStep);
            }
            let initial = scenario.initial();
            let expected_engine_active =
                initial.propellant().raw() > 0 && scenario.vehicle().burn_duration().raw() > 0;
            if frame.time().raw() != 0
                || frame.altitude() != initial.altitude()
                || frame.velocity() != initial.velocity()
                || frame.acceleration().raw() != 0
                || frame.total_mass() != initial.total_mass()
                || frame.propellant() != initial.propellant()
                || frame.status().bits() != u16::from(expected_engine_active)
                || events != 0
                || frame.state_checksum() != VERTICAL_CHECKSUM_OFFSET
            {
                return Err(StreamInspectionError::InitialFrame);
            }
            first_frame = Some(frame);
        } else {
            let repeated_fault_truth = terminal && numeric_fault && frame.step() == prior_step;
            if frame.step() <= prior_step && !repeated_fault_truth {
                return Err(StreamInspectionError::StepOrder { index });
            }
            let stride = header.telemetry_stride() as u32;
            let stride_aligned = (frame.step() / stride) * stride == frame.step();
            if !terminal && !stride_aligned {
                return Err(StreamInspectionError::Stride { index });
            }
        }

        if frame.step() > scenario.steps() {
            return Err(StreamInspectionError::StepRange { index });
        }
        let expected_time = i64::from(header.timestep().raw()) * i64::from(frame.step());
        if expected_time != i64::from(frame.time().raw()) {
            return Err(StreamInspectionError::MissionTime { index });
        }
        if numeric_fault && !terminal {
            return Err(StreamInspectionError::NumericFaultWithoutEnd { index });
        }
        if terminal && index + 1 != frame_count {
            return Err(StreamInspectionError::TerminalBeforeEnd { index });
        }

        cutoff_event_frames += u32::from(events & TelemetryEvents::ENGINE_CUTOFF != 0);
        depletion_event_frames += u32::from(events & TelemetryEvents::PROPELLANT_DEPLETED != 0);
        numeric_fault_event_frames += u32::from(numeric_fault);
        prior_step = frame.step();
        final_frame = Some(frame);
    }

    let first_frame = match first_frame {
        Some(frame) => frame,
        None => return Err(StreamInspectionError::Empty),
    };
    let final_frame = match final_frame {
        Some(frame) => frame,
        None => return Err(StreamInspectionError::Empty),
    };
    if final_frame.events().bits() & TelemetryEvents::END_OF_RUN == 0 {
        return Err(StreamInspectionError::MissingTerminal);
    }
    if final_frame.events().bits() & TelemetryEvents::NUMERIC_FAULT == 0
        && final_frame.step() != scenario.steps()
    {
        return Err(StreamInspectionError::SuccessfulEndStep);
    }

    Ok(StreamInspection {
        header,
        frame_count,
        stream_bytes: stream.len(),
        stream_crc32: crc32_ieee(stream),
        first_frame,
        final_frame,
        cutoff_event_frames,
        depletion_event_frames,
        numeric_fault_event_frames,
    })
}

pub fn format_inspection(inspection: StreamInspection) -> String {
    let header = inspection.header();
    let final_frame = inspection.final_frame();
    format!(
        concat!(
            "KSA64 TELEMETRY V1\n",
            "scenario       0x{scenario:08x}\n",
            "timestep       {timestep:.6} s\n",
            "stride         {stride} physics steps\n",
            "frames         {frames}\n",
            "stream         {bytes} bytes, CRC32 0x{stream_crc:08x}\n",
            "final step     {step}\n",
            "mission time   {time:.3} s\n",
            "altitude       {altitude:.6} km\n",
            "velocity       {velocity:.6} km/s\n",
            "acceleration   {acceleration:.9} km/s^2\n",
            "mass           {mass:.6} t\n",
            "propellant     {propellant:.6} t\n",
            "state checksum 0x{state_checksum:08x}\n",
            "event frames   cutoff={cutoff}, depletion={depletion}, numeric_fault={fault}\n"
        ),
        scenario = header.scenario_id(),
        timestep = f64::from(header.timestep().raw()) / 65_536.0,
        stride = header.telemetry_stride(),
        frames = inspection.frame_count(),
        bytes = inspection.stream_bytes(),
        stream_crc = inspection.stream_crc32(),
        step = final_frame.step(),
        time = f64::from(final_frame.time().raw()) / 65_536.0,
        altitude = f64::from(final_frame.altitude().raw()) / 4_096.0,
        velocity = f64::from(final_frame.velocity().raw()) / 16_777_216.0,
        acceleration = f64::from(final_frame.acceleration().raw()) / 268_435_456.0,
        mass = f64::from(final_frame.total_mass().raw()) / 4_096.0,
        propellant = f64::from(final_frame.propellant().raw()) / 4_096.0,
        state_checksum = final_frame.state_checksum(),
        cutoff = inspection.cutoff_event_frames(),
        depletion = inspection.depletion_event_frames(),
        fault = inspection.numeric_fault_event_frames(),
    )
}

pub mod phase5_history;

pub mod phase8;

pub mod phase8_5;
pub mod phase8_5_campaign;
pub mod phase8_5_link;
pub mod phase8_5_tui;

pub mod phase10;
pub mod phase10_link;
pub mod phase10_mission;
pub mod phase10_tui;
pub mod phase11_authoring;
pub mod phase11_debrief;
pub mod phase11_operations;
pub mod phase11_prediction;
pub mod phase11_scenarios;
pub mod phase11_session;
pub mod phase11_tui;
pub mod phase9;
pub mod phase9_5_archive;
pub mod phase9_5_compiler;
pub mod phase9_5_link;
pub mod phase9_5_tui;
pub mod phase9_5_workbench;
pub mod phase9_archive;
pub mod phase9_manifest;
pub mod phase9_protocol;
pub mod phase9_report;
pub mod phase9_search;
pub mod phase9_sensitivity;
pub mod phase9_tui;

pub mod application;
pub mod automation_app;
pub mod catalog_assets;
pub mod cli;
pub mod optimization_app;
pub mod product;
