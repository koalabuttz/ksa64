//! Post-run C64 telemetry status sink and direct 40x25 screen renderer.

use core::convert::Infallible;

use crate::numeric::{multiply_scaled, NumericStatus};
use crate::scenario::Scenario;
use crate::telemetry::{
    parse_telemetry_frame, parse_telemetry_header_for_scenario, TelemetryEvents, TelemetryFrame,
    TelemetryReadError, TelemetrySink, TELEMETRY_FRAME_LENGTH, TELEMETRY_HEADER_LENGTH,
};

mod error_data {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase1/generated/high_precision_v1.rs"
    ));
}

const SCREEN: *mut u8 = 0x0400 as *mut u8;
const COLOR_RAM: *mut u8 = 0xd800 as *mut u8;
const BORDER_COLOR: *mut u8 = 0xd020 as *mut u8;
const BACKGROUND_COLOR: *mut u8 = 0xd021 as *mut u8;
const SCREEN_COLUMNS: usize = 40;
const SCREEN_CELLS: usize = 1_000;
const SCREEN_SPACE: u8 = 32;
const TEXT_COLOR: u8 = 1;

pub struct C64StatusSink {
    header: [u8; TELEMETRY_HEADER_LENGTH],
    latest_frame: [u8; TELEMETRY_FRAME_LENGTH],
    has_header: bool,
    has_frame: bool,
    frames_written: u32,
    observed_events: u16,
}

impl C64StatusSink {
    pub const fn new() -> Self {
        Self {
            header: [0; TELEMETRY_HEADER_LENGTH],
            latest_frame: [0; TELEMETRY_FRAME_LENGTH],
            has_header: false,
            has_frame: false,
            frames_written: 0,
            observed_events: 0,
        }
    }

    pub const fn frames_written(&self) -> u32 {
        self.frames_written
    }

    pub fn latest_frame(&self) -> Result<TelemetryFrame, TelemetryReadError> {
        if !self.has_frame {
            return Err(TelemetryReadError::Length);
        }
        parse_telemetry_frame(&self.latest_frame)
    }
}

impl Default for C64StatusSink {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySink for C64StatusSink {
    type Error = Infallible;

    fn write_header(&mut self, header: &[u8; TELEMETRY_HEADER_LENGTH]) -> Result<(), Self::Error> {
        self.header.copy_from_slice(header);
        self.has_header = true;
        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8; TELEMETRY_FRAME_LENGTH]) -> Result<(), Self::Error> {
        self.latest_frame.copy_from_slice(frame);
        self.has_frame = true;
        self.frames_written += 1;
        self.observed_events |= u16::from_le_bytes([frame[30], frame[31]]);
        Ok(())
    }
}

#[inline]
fn ascii_to_screen(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte - b'A' + 1
    } else {
        byte
    }
}

#[inline]
unsafe fn put(row: usize, column: usize, byte: u8) {
    if row < 25 && column < SCREEN_COLUMNS {
        let offset = row * SCREEN_COLUMNS + column;
        core::ptr::write_volatile(SCREEN.add(offset), ascii_to_screen(byte));
        core::ptr::write_volatile(COLOR_RAM.add(offset), TEXT_COLOR);
    }
}

unsafe fn clear_screen() {
    let mut offset = 0usize;
    while offset < SCREEN_CELLS {
        core::ptr::write_volatile(SCREEN.add(offset), SCREEN_SPACE);
        core::ptr::write_volatile(COLOR_RAM.add(offset), TEXT_COLOR);
        offset += 1;
    }
    core::ptr::write_volatile(BORDER_COLOR, 6);
    core::ptr::write_volatile(BACKGROUND_COLOR, 0);
}

unsafe fn write_text(row: usize, column: usize, text: &str) {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && column + index < SCREEN_COLUMNS {
        put(row, column + index, bytes[index]);
        index += 1;
    }
}

unsafe fn clear_field(row: usize, column: usize, width: usize) {
    let mut index = 0usize;
    while index < width {
        put(row, column + index, b' ');
        index += 1;
    }
}

unsafe fn write_u32_right(row: usize, column: usize, width: usize, mut value: u32) {
    clear_field(row, column, width);
    let mut position = column + width;
    loop {
        if position == column {
            return;
        }
        position -= 1;
        put(row, position, b'0' + (value % 10) as u8);
        value /= 10;
        if value == 0 {
            return;
        }
    }
}

unsafe fn write_fixed_3(row: usize, column: usize, width: usize, raw: i32, fractional_bits: u8) {
    clear_field(row, column, width);
    let negative = raw < 0;
    let magnitude = raw.saturating_abs();
    let scale = 1i32 << fractional_bits;
    let mut integer = (magnitude >> fractional_bits) as u32;
    let fraction = magnitude & (scale - 1);
    let mut status = NumericStatus::CLEAR;
    let mut thousandths = multiply_scaled(fraction, 1_000, fractional_bits, &mut status) as u32;
    if thousandths >= 1_000 {
        integer += 1;
        thousandths -= 1_000;
    }

    let fraction_start = column + width - 3;
    put(row, fraction_start - 1, b'.');
    put(row, fraction_start, b'0' + (thousandths / 100) as u8);
    put(
        row,
        fraction_start + 1,
        b'0' + ((thousandths / 10) % 10) as u8,
    );
    put(row, fraction_start + 2, b'0' + (thousandths % 10) as u8);

    let mut position = fraction_start - 1;
    loop {
        if position == column {
            return;
        }
        position -= 1;
        put(row, position, b'0' + (integer % 10) as u8);
        integer /= 10;
        if integer == 0 {
            break;
        }
    }
    if negative && position > column {
        put(row, position - 1, b'-');
    }
}

unsafe fn write_hex_u32(row: usize, column: usize, value: u32) {
    let mut index = 0usize;
    while index < 8 {
        let shift = 28 - index * 4;
        let digit = ((value >> shift) & 0x0f) as u8;
        put(
            row,
            column + index,
            if digit < 10 {
                b'0' + digit
            } else {
                b'A' + digit - 10
            },
        );
        index += 1;
    }
}

pub fn render_status(scenario: &Scenario, sink: &C64StatusSink) -> Result<(), TelemetryReadError> {
    if !sink.has_header {
        return Err(TelemetryReadError::Length);
    }
    let header = parse_telemetry_header_for_scenario(&sink.header, scenario)?;
    let frame = sink.latest_frame()?;
    let events = sink.observed_events;
    let fault = events & TelemetryEvents::NUMERIC_FAULT != 0;

    unsafe {
        clear_screen();
        write_text(0, 0, "KSA64 VERTICAL FLIGHT");
        write_text(0, 31, if fault { "FAULT" } else { "COMPLETE" });
        write_text(1, 0, "----------------------------------------");

        write_text(3, 0, "MISSION TIME");
        write_fixed_3(3, 16, 12, frame.time().raw(), 16);
        write_text(3, 29, "S");
        write_text(4, 0, "ALTITUDE");
        write_fixed_3(4, 16, 12, frame.altitude().raw(), 12);
        write_text(4, 29, "KM");
        write_text(5, 0, "VELOCITY");
        write_fixed_3(5, 16, 12, frame.velocity().raw(), 24);
        write_text(5, 29, "KM/S");
        write_text(6, 0, "ACCELERATION");
        write_fixed_3(6, 16, 12, frame.acceleration().raw(), 28);
        write_text(6, 29, "KM/S2");
        write_text(7, 0, "MASS");
        write_fixed_3(7, 16, 12, frame.total_mass().raw(), 12);
        write_text(7, 29, "T");
        write_text(8, 0, "PROPELLANT");
        write_fixed_3(8, 16, 12, frame.propellant().raw(), 12);
        write_text(8, 29, "T");

        write_text(10, 0, "STEP");
        write_u32_right(10, 16, 12, frame.step());
        write_text(11, 0, "FRAMES");
        write_u32_right(11, 16, 12, sink.frames_written());
        write_text(12, 0, "STRIDE");
        write_u32_right(12, 16, 12, header.telemetry_stride() as u32);

        write_text(14, 0, "STATE CHECKSUM");
        write_hex_u32(14, 20, frame.state_checksum());
        write_text(16, 0, "EVENTS");
        if events & TelemetryEvents::ENGINE_CUTOFF != 0 {
            write_text(17, 0, "CUTOFF");
        }
        if events & TelemetryEvents::PROPELLANT_DEPLETED != 0 {
            write_text(17, 8, "DEPLETED");
        }
        if events & TelemetryEvents::END_OF_RUN != 0 {
            write_text(17, 18, "END");
        }
        if fault {
            write_text(17, 23, "NUMERIC FAULT");
        }
        write_text(18, 0, "HP ALT DELTA");
        write_fixed_3(18, 16, 12, error_data::FINAL_ALTITUDE_ERROR_M_Q16, 16);
        write_text(18, 29, "M");
        write_text(19, 0, "HP VEL DELTA");
        write_fixed_3(19, 16, 12, error_data::FINAL_VELOCITY_ERROR_M_S_Q16, 16);
        write_text(19, 29, "M/S");

        write_text(20, 0, "RAW PHYSICS       8.34 HZ");
        write_text(21, 0, "RECORDED MODE     5.62 HZ");
        write_text(22, 0, "STATUS SINK");
        write_u32_right(22, 16, 12, core::mem::size_of::<C64StatusSink>() as u32);
        write_text(22, 29, "B");
        write_text(23, 0, "POST-RUN DISPLAY - TIMING EXCLUDED");
    }
    Ok(())
}
