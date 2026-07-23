//! Bounded 40x25 stock-C64 campaign presentation.

use super::contracts::STOCK_INTERESTING_SUMMARIES;
use super::plot::PlotArchive;
use super::stock::StockSnapshot;
use super::summary::RunOutcome;

pub const SCREEN_WIDTH: usize = 40;
pub const SCREEN_HEIGHT: usize = 25;
pub const SCREEN_BYTES: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StockPage {
    Campaign = 1,
    Histogram = 3,
    Trajectory = 5,
    Storage = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockUiData {
    pub campaign_crc32: u32,
    pub run_count: u32,
    pub outcomes: [u32; RunOutcome::COUNT],
    pub cutoff_altitude_km: [i32; 4],
    pub max_dynamic_pressure_kpa: [i32; 4],
    pub max_proper_acceleration_mps2: [i32; 4],
    pub navigation_position_error_m: [i32; 4],
    pub summary_chain: u32,
    pub retained_runs: [u32; STOCK_INTERESTING_SUMMARIES],
    pub plot_bytes: u16,
    pub plot_crc32: u32,
    pub archive_complete: bool,
}

impl StockUiData {
    pub fn from_snapshot(
        campaign_crc32: u32,
        snapshot: &StockSnapshot,
        plot_bytes: u16,
        plot_crc32: u32,
    ) -> Self {
        let metric = |value: super::aggregate::StreamingMetric| {
            [
                value.minimum,
                value.maximum,
                value.mean(),
                value
                    .sample_variance()
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            ]
        };
        Self {
            campaign_crc32,
            run_count: snapshot.aggregate.run_count,
            outcomes: snapshot.aggregate.outcome_counts,
            cutoff_altitude_km: metric(snapshot.aggregate.cutoff_altitude_km),
            max_dynamic_pressure_kpa: metric(snapshot.aggregate.max_dynamic_pressure_kpa),
            max_proper_acceleration_mps2: metric(snapshot.aggregate.max_proper_acceleration_mps2),
            navigation_position_error_m: metric(snapshot.aggregate.navigation_position_error_m),
            summary_chain: snapshot.aggregate.summary_chain,
            retained_runs: snapshot.retained.map(|summary| summary.run_index),
            plot_bytes,
            plot_crc32,
            archive_complete: true,
        }
    }
}

pub const REFERENCE_STOCK_UI: StockUiData = StockUiData {
    campaign_crc32: 0xa2e9_e9d5,
    run_count: 1_024,
    outcomes: [857, 166, 1, 0, 0, 0],
    cutoff_altitude_km: [180, 194, 188, 5],
    max_dynamic_pressure_kpa: [39, 43, 41, 0],
    max_proper_acceleration_mps2: [54, 55, 54, 0],
    navigation_position_error_m: [0, 62, 21, 127],
    summary_chain: 0x813c_e420,
    retained_runs: [0, 8, 96, 796, 1],
    plot_bytes: 1_872,
    plot_crc32: 0x7719_f7af,
    archive_complete: true,
};

struct Canvas<'a> {
    bytes: &'a mut [u8; SCREEN_BYTES],
}
impl Canvas<'_> {
    fn clear(&mut self) {
        self.bytes.fill(b' ');
    }
    fn text(&mut self, row: usize, column: usize, value: &[u8]) {
        if row >= SCREEN_HEIGHT || column >= SCREEN_WIDTH {
            return;
        }
        let length = value.len().min(SCREEN_WIDTH - column);
        self.bytes[row * SCREEN_WIDTH + column..row * SCREEN_WIDTH + column + length]
            .copy_from_slice(&value[..length]);
    }
    fn character(&mut self, row: usize, column: usize, value: u8) {
        if row < SCREEN_HEIGHT && column < SCREEN_WIDTH {
            self.bytes[row * SCREEN_WIDTH + column] = value;
        }
    }
    fn unsigned(&mut self, row: usize, column: usize, width: usize, mut value: u32) {
        let width = width.min(SCREEN_WIDTH.saturating_sub(column));
        for index in (0..width).rev() {
            self.character(row, column + index, b'0' + (value % 10) as u8);
            value /= 10;
        }
    }
    fn signed(&mut self, row: usize, column: usize, width: usize, value: i32) {
        if value < 0 {
            self.character(row, column, b'-');
            self.unsigned(
                row,
                column + 1,
                width.saturating_sub(1),
                value.unsigned_abs(),
            );
        } else {
            self.unsigned(row, column, width, value as u32);
        }
    }
    fn hex32(&mut self, row: usize, column: usize, value: u32) {
        for index in 0..8 {
            let nibble = ((value >> (28 - index * 4)) & 15) as u8;
            self.character(
                row,
                column + index,
                if nibble < 10 {
                    b'0' + nibble
                } else {
                    b'A' + nibble - 10
                },
            );
        }
    }
}

pub fn render_stock_page(
    page: StockPage,
    data: &StockUiData,
    plot: &PlotArchive<'_>,
    screen: &mut [u8; SCREEN_BYTES],
) {
    let mut canvas = Canvas { bytes: screen };
    canvas.clear();
    match page {
        StockPage::Campaign => render_campaign(&mut canvas, data),
        StockPage::Histogram => render_histogram(&mut canvas, data),
        StockPage::Trajectory => render_trajectory(&mut canvas, data, plot),
        StockPage::Storage => render_storage(&mut canvas, data),
    }
}

fn title(canvas: &mut Canvas<'_>, value: &[u8], key: u8) {
    canvas.text(0, 0, b"KSA64 PHASE 4 ");
    canvas.text(0, 14, value);
    canvas.text(0, 37, b"F");
    canvas.character(0, 38, key);
}

fn render_campaign(canvas: &mut Canvas<'_>, data: &StockUiData) {
    title(canvas, b"CAMPAIGN", b'1');
    canvas.text(2, 0, b"RUNS");
    canvas.unsigned(2, 6, 4, data.run_count);
    canvas.text(2, 13, b"SUCCESS");
    canvas.unsigned(2, 21, 4, data.outcomes[0]);
    canvas.text(3, 0, b"STABLE SUBORB IMPACT ESCAPE ABORT ERROR");
    for (index, count) in data.outcomes.iter().enumerate() {
        canvas.unsigned(4, index * 6, 4, *count);
    }
    metric_row(canvas, 7, b"CUTOFF ALT KM", data.cutoff_altitude_km);
    metric_row(canvas, 10, b"MAX Q KPA", data.max_dynamic_pressure_kpa);
    metric_row(
        canvas,
        13,
        b"PROPER M/S2",
        data.max_proper_acceleration_mps2,
    );
    metric_row(canvas, 16, b"NAV ERROR M", data.navigation_position_error_m);
    canvas.text(20, 0, b"SUMMARY CHAIN");
    canvas.hex32(20, 14, data.summary_chain);
    canvas.text(22, 0, b"STREAMING AGGREGATES - NO REU REQUIRED");
    canvas.text(24, 0, b"F1 CAMPAIGN F3 HIST F5 PLOT F7 STORAGE");
}

fn metric_row(canvas: &mut Canvas<'_>, row: usize, label: &[u8], values: [i32; 4]) {
    canvas.text(row, 0, label);
    canvas.text(row + 1, 0, b"MIN");
    canvas.signed(row + 1, 4, 5, values[0]);
    canvas.text(row + 1, 11, b"MAX");
    canvas.signed(row + 1, 15, 5, values[1]);
    canvas.text(row + 1, 22, b"MEAN");
    canvas.signed(row + 1, 27, 5, values[2]);
    canvas.text(row + 1, 34, b"VAR");
    canvas.signed(row + 1, 38, 2, values[3].min(99));
}

fn render_histogram(canvas: &mut Canvas<'_>, data: &StockUiData) {
    title(canvas, b"OUTCOME HISTOGRAM", b'3');
    let maximum = data.outcomes.iter().copied().max().unwrap_or(1).max(1);
    let labels: [&[u8]; 6] = [
        b"STABLE", b"SUBORB", b"IMPACT", b"ESCAPE", b"ABORT ", b"ERROR ",
    ];
    for index in 0..6 {
        let row = 3 + index * 3;
        canvas.text(row, 0, labels[index]);
        canvas.unsigned(row, 7, 4, data.outcomes[index]);
        let width = (data.outcomes[index] as u64 * 26 / maximum as u64) as usize;
        for column in 0..width {
            canvas.character(row + 1, 7 + column, b'#');
        }
    }
    canvas.text(22, 0, b"COMPACT CLASSIFIER - ANALYZER AUTHORITATIVE");
    canvas.text(24, 0, b"F1 CAMPAIGN F3 HIST F5 PLOT F7 STORAGE");
}

fn render_trajectory(canvas: &mut Canvas<'_>, data: &StockUiData, plot: &PlotArchive<'_>) {
    title(canvas, b"BASELINE TRAJECTORY", b'5');
    canvas.text(2, 0, b"ALTITUDE VS SAMPLE");
    let graph_top = 4usize;
    let graph_height = 14usize;
    for row in graph_top..graph_top + graph_height {
        canvas.character(row, 0, b'|');
    }
    for column in 0..SCREEN_WIDTH {
        canvas.character(graph_top + graph_height, column, b'-');
    }
    let count = plot.point_count.max(1);
    for index in 0..plot.point_count {
        if let Some(point) = plot.point(index) {
            let column = 1 + index * 38 / count;
            let altitude = point.altitude_quarter_km.max(0) as usize;
            let height = (altitude * (graph_height - 1) / 800).min(graph_height - 1);
            canvas.character(graph_top + graph_height - 1 - height, column, b'*');
        }
    }
    canvas.text(20, 0, b"RETAINED RUNS");
    for (index, run) in data.retained_runs.iter().enumerate() {
        canvas.unsigned(21, index * 8, 4, *run);
    }
    canvas.text(22, 0, b"BASE WORST-INS LOAD NAV FIRST-FAIL");
    canvas.text(24, 0, b"F1 CAMPAIGN F3 HIST F5 PLOT F7 STORAGE");
}

fn render_storage(canvas: &mut Canvas<'_>, data: &StockUiData) {
    title(canvas, b"STORAGE", b'7');
    canvas.text(3, 0, b"MODE             STOCK C64");
    canvas.text(5, 0, b"REU REQUIRED     NO");
    canvas.text(7, 0, b"SUMMARY SLOTS");
    canvas.unsigned(7, 18, 4, STOCK_INTERESTING_SUMMARIES as u32);
    canvas.text(9, 0, b"SPARSE PLOT BYTES");
    canvas.unsigned(9, 18, 4, data.plot_bytes as u32);
    canvas.text(11, 0, b"PLOT CRC");
    canvas.hex32(11, 18, data.plot_crc32);
    canvas.text(13, 0, b"CAMPAIGN ID");
    canvas.hex32(13, 18, data.campaign_crc32);
    canvas.text(15, 0, b"ARCHIVE");
    canvas.text(
        15,
        18,
        if data.archive_complete {
            b"COMPLETE"
        } else {
            b"INCOMPLETE"
        },
    );
    canvas.text(17, 0, b"REPORT EXPORT    READY");
    canvas.text(20, 0, b"REU WILL EXPAND RETENTION AUTOMATICALLY");
    canvas.text(22, 0, b"PHYSICS AND CHECKSUMS ARE STORAGE-NEUTRAL");
    canvas.text(24, 0, b"F1 CAMPAIGN F3 HIST F5 PLOT F7 STORAGE");
}
