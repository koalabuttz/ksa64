//! Compact KPH4 sparse trajectory histories.

use ksa64_core::phase2_numeric::EARTH_RADIUS_Q12;
use ksa64_interface::crc32_ieee;

use crate::mission::{MissionObserver, MissionRecord};

use super::contracts::{KPH4_MAGIC, PLOT_HEADER_LENGTH, PLOT_POINT_LENGTH, STOCK_PLOT_STRIDE};
use super::PHASE4_CONTRACT_ID;

pub const KPH4_VERSION: u16 = 4;
pub const STOCK_PLOT_MAX_POINTS: usize = 256;
const HEADER_CRC_OFFSET: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PlotPoint {
    pub step: u16,
    pub altitude_quarter_km: i16,
    pub downrange_q16: u16,
    pub flags: u16,
}
impl PlotPoint {
    pub const ZERO: Self = Self {
        step: 0,
        altitude_quarter_km: 0,
        downrange_q16: 0,
        flags: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlotIdentity {
    pub campaign_crc32: u32,
    pub run_index: u32,
    pub sensor_seed: u32,
    pub variation_checksum: u32,
    pub source_summary_crc32: u32,
    pub stride: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotError {
    Capacity,
    Length,
    Magic,
    Version,
    Contract,
    Reserved,
    Checksum,
}

pub struct PlotRecorder<const N: usize> {
    stride: u32,
    points: [PlotPoint; N],
    count: usize,
}

impl<const N: usize> PlotRecorder<N> {
    pub const fn new(stride: u32) -> Self {
        Self {
            stride,
            points: [PlotPoint::ZERO; N],
            count: 0,
        }
    }

    pub const fn stock() -> Self {
        Self::new(STOCK_PLOT_STRIDE)
    }

    pub fn points(&self) -> &[PlotPoint] {
        &self.points[..self.count]
    }
}

impl<const N: usize> MissionObserver for PlotRecorder<N> {
    type Error = PlotError;

    fn observe(&mut self, record: MissionRecord) -> Result<(), Self::Error> {
        let step = record.world.truth.step();
        if self.stride == 0 || step % self.stride != 0 {
            return Ok(());
        }
        if self.count == N || step > u16::MAX as u32 {
            return Err(PlotError::Capacity);
        }
        let altitude_q12 = record.world.truth.radius().raw() - EARTH_RADIUS_Q12;
        let altitude_quarter_km = if altitude_q12 >= 0 {
            (altitude_q12 + 512) / 1_024
        } else {
            (altitude_q12 - 512) / 1_024
        };
        self.points[self.count] = PlotPoint {
            step: step as u16,
            altitude_quarter_km: altitude_quarter_km.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            downrange_q16: (record.world.truth.downrange().raw() as u32 >> 16) as u16,
            flags: (record.sensors.events & 0x00ff) | ((record.flight.alarms & 0x00ff) << 8),
        };
        self.count += 1;
        Ok(())
    }
}

pub struct PlotArchive<'a> {
    pub identity: PlotIdentity,
    pub points: &'a [u8],
    pub point_count: usize,
}

impl PlotArchive<'_> {
    pub fn point(&self, index: usize) -> Option<PlotPoint> {
        if index >= self.point_count {
            return None;
        }
        let offset = index * PLOT_POINT_LENGTH;
        Some(read_point(&self.points[offset..offset + PLOT_POINT_LENGTH]))
    }
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn write_point(point: PlotPoint, out: &mut [u8]) {
    put_u16(out, 0, point.step);
    put_u16(out, 2, point.altitude_quarter_km as u16);
    put_u16(out, 4, point.downrange_q16);
    put_u16(out, 6, point.flags);
}
fn read_point(bytes: &[u8]) -> PlotPoint {
    PlotPoint {
        step: get_u16(bytes, 0),
        altitude_quarter_km: get_u16(bytes, 2) as i16,
        downrange_q16: get_u16(bytes, 4),
        flags: get_u16(bytes, 6),
    }
}

pub fn encoded_kph4_length(point_count: usize) -> Option<usize> {
    point_count
        .checked_mul(PLOT_POINT_LENGTH)
        .and_then(|payload| PLOT_HEADER_LENGTH.checked_add(payload))
}

pub fn write_kph4(
    identity: PlotIdentity,
    points: &[PlotPoint],
    out: &mut [u8],
) -> Result<(), PlotError> {
    let expected = encoded_kph4_length(points.len()).ok_or(PlotError::Length)?;
    if out.len() != expected || points.len() > u16::MAX as usize || identity.stride == 0 {
        return Err(PlotError::Length);
    }
    out.fill(0);
    out[0..4].copy_from_slice(&KPH4_MAGIC);
    put_u16(out, 4, KPH4_VERSION);
    put_u16(out, 6, PLOT_HEADER_LENGTH as u16);
    put_u16(out, 8, PLOT_POINT_LENGTH as u16);
    put_u32(out, 12, PHASE4_CONTRACT_ID);
    put_u32(out, 16, identity.campaign_crc32);
    put_u32(out, 20, identity.run_index);
    put_u32(out, 24, identity.sensor_seed);
    put_u32(out, 28, identity.variation_checksum);
    put_u16(out, 32, identity.stride);
    put_u16(out, 34, points.len() as u16);
    put_u32(out, 36, identity.source_summary_crc32);
    let mut offset = PLOT_HEADER_LENGTH;
    for point in points {
        write_point(*point, &mut out[offset..offset + PLOT_POINT_LENGTH]);
        offset += PLOT_POINT_LENGTH;
    }
    put_u32(out, 40, crc32_ieee(&out[PLOT_HEADER_LENGTH..]));
    put_u32(
        out,
        HEADER_CRC_OFFSET,
        crc32_ieee(&out[..HEADER_CRC_OFFSET]),
    );
    Ok(())
}

pub fn parse_kph4(bytes: &[u8]) -> Result<PlotArchive<'_>, PlotError> {
    if bytes.len() < PLOT_HEADER_LENGTH {
        return Err(PlotError::Length);
    }
    if bytes[0..4] != KPH4_MAGIC {
        return Err(PlotError::Magic);
    }
    if get_u16(bytes, 4) != KPH4_VERSION
        || get_u16(bytes, 6) != PLOT_HEADER_LENGTH as u16
        || get_u16(bytes, 8) != PLOT_POINT_LENGTH as u16
    {
        return Err(PlotError::Version);
    }
    if get_u32(bytes, 12) != PHASE4_CONTRACT_ID {
        return Err(PlotError::Contract);
    }
    if bytes[10..12].iter().any(|&value| value != 0)
        || bytes[44..HEADER_CRC_OFFSET].iter().any(|&value| value != 0)
    {
        return Err(PlotError::Reserved);
    }
    if crc32_ieee(&bytes[..HEADER_CRC_OFFSET]) != get_u32(bytes, HEADER_CRC_OFFSET) {
        return Err(PlotError::Checksum);
    }
    let point_count = get_u16(bytes, 34) as usize;
    let expected = encoded_kph4_length(point_count).ok_or(PlotError::Length)?;
    if bytes.len() != expected || get_u16(bytes, 32) == 0 {
        return Err(PlotError::Length);
    }
    let points = &bytes[PLOT_HEADER_LENGTH..];
    if crc32_ieee(points) != get_u32(bytes, 40) {
        return Err(PlotError::Checksum);
    }
    Ok(PlotArchive {
        identity: PlotIdentity {
            campaign_crc32: get_u32(bytes, 16),
            run_index: get_u32(bytes, 20),
            sensor_seed: get_u32(bytes, 24),
            variation_checksum: get_u32(bytes, 28),
            source_summary_crc32: get_u32(bytes, 36),
            stride: get_u16(bytes, 32),
        },
        points,
        point_count,
    })
}
