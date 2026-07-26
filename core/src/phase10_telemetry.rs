//! Strict Phase 10 telemetry, evaluation, plot, and campaign records.
//!
//! These contracts carry only fixed-width raw values. Interpretation is bound
//! to `GlobalEcef6DofV1` and the identities embedded in each record.

use crate::evaluation::{EvaluationOutcome, EvaluationSummary, MetricValidity, ModelProfileId};
use crate::phase10_contract::{GlobalSegment, ReferenceFrameId, PHASE10_CONTRACT_ID};
use crate::scenario::crc32_ieee;

pub const KTT10_HEADER_LENGTH: usize = 128;
pub const KTT10_FRAME_LENGTH: usize = 256;
pub const KSR10_LENGTH: usize = 512;
pub const KPH10_HEADER_LENGTH: usize = 64;
pub const KPH10_POINT_LENGTH: usize = 48;
pub const KSC10_LENGTH: usize = 512;
pub const GLOBAL_CHECKSUM_COUNT: usize = 8;
pub const GLOBAL_TRANSITION_COUNT: usize = 4;
const VERSION: u16 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalRecordError {
    Length,
    Magic,
    Version,
    Kind,
    Contract,
    Identity,
    Profile,
    Frame,
    Segment,
    Outcome,
    Range,
    Reserved,
    Checksum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
enum RecordKind {
    TelemetryHeader = 1,
    EvaluationSummary = 2,
    PlotHeader = 3,
    Campaign = 4,
}

impl RecordKind {
    const fn magic(self) -> [u8; 5] {
        match self {
            Self::TelemetryHeader => *b"KTT10",
            Self::EvaluationSummary => *b"KSR10",
            Self::PlotHeader => *b"KPH10",
            Self::Campaign => *b"KSC10",
        }
    }

    const fn length(self) -> usize {
        match self {
            Self::TelemetryHeader => KTT10_HEADER_LENGTH,
            Self::EvaluationSummary => KSR10_LENGTH,
            Self::PlotHeader => KPH10_HEADER_LENGTH,
            Self::Campaign => KSC10_LENGTH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalTelemetryHeader {
    pub identity: u32,
    pub earth_identity: u32,
    pub transform_identity: u32,
    pub atmosphere_identity: u32,
    pub vehicle_identity: u32,
    pub mission_identity: u32,
    pub avionics_identity: u32,
    pub case_seed: u32,
    pub telemetry_period_q16: u32,
    pub max_mission_time_q16: u32,
}

impl GlobalTelemetryHeader {
    pub fn encode(self, output: &mut [u8; KTT10_HEADER_LENGTH]) -> Result<(), GlobalRecordError> {
        if self.identity == 0
            || self.earth_identity == 0
            || self.transform_identity == 0
            || self.atmosphere_identity == 0
            || self.vehicle_identity == 0
            || self.mission_identity == 0
            || self.avionics_identity == 0
            || self.telemetry_period_q16 == 0
            || self.max_mission_time_q16 == 0
        {
            return Err(GlobalRecordError::Identity);
        }
        write_header(output, RecordKind::TelemetryHeader, self.identity)?;
        output[32] = ModelProfileId::GlobalEcef6DofV1 as u8;
        output[33] = ReferenceFrameId::LocalEnuV1 as u8;
        output[34] = ReferenceFrameId::EarthFixedEcefV1 as u8;
        output[35] = ReferenceFrameId::EarthInertialEciV1 as u8;
        p16(output, 36, KTT10_FRAME_LENGTH as u16);
        for (at, value) in [
            (40, self.earth_identity),
            (44, self.transform_identity),
            (48, self.atmosphere_identity),
            (52, self.vehicle_identity),
            (56, self.mission_identity),
            (60, self.avionics_identity),
            (64, self.case_seed),
            (68, self.telemetry_period_q16),
            (72, self.max_mission_time_q16),
        ] {
            p32(output, at, value);
        }
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalRecordError> {
        let identity = validate_record(bytes, RecordKind::TelemetryHeader)?;
        if bytes[32] != ModelProfileId::GlobalEcef6DofV1 as u8
            || bytes[33] != ReferenceFrameId::LocalEnuV1 as u8
            || bytes[34] != ReferenceFrameId::EarthFixedEcefV1 as u8
            || bytes[35] != ReferenceFrameId::EarthInertialEciV1 as u8
            || g16(bytes, 36) as usize != KTT10_FRAME_LENGTH
            || bytes[38..40].iter().any(|value| *value != 0)
            || bytes[76..KTT10_HEADER_LENGTH - 4]
                .iter()
                .any(|value| *value != 0)
        {
            return Err(GlobalRecordError::Reserved);
        }
        let header = Self {
            identity,
            earth_identity: g32(bytes, 40),
            transform_identity: g32(bytes, 44),
            atmosphere_identity: g32(bytes, 48),
            vehicle_identity: g32(bytes, 52),
            mission_identity: g32(bytes, 56),
            avionics_identity: g32(bytes, 60),
            case_seed: g32(bytes, 64),
            telemetry_period_q16: g32(bytes, 68),
            max_mission_time_q16: g32(bytes, 72),
        };
        if header.earth_identity == 0
            || header.transform_identity == 0
            || header.atmosphere_identity == 0
            || header.vehicle_identity == 0
            || header.mission_identity == 0
            || header.avionics_identity == 0
            || header.telemetry_period_q16 == 0
            || header.max_mission_time_q16 == 0
        {
            return Err(GlobalRecordError::Identity);
        }
        Ok(header)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalTelemetryFrame {
    pub step: u32,
    pub mission_time_q16: u32,
    pub frame: ReferenceFrameId,
    pub segment: GlobalSegment,
    pub flight_mode: u8,
    pub events: u16,
    pub truth_position_q12: [i32; 3],
    pub truth_velocity_q24: [i32; 3],
    pub truth_attitude_q30: [i32; 4],
    pub truth_angular_rate_q24: [i32; 3],
    pub ecef_position_q12: [i32; 3],
    pub ecef_velocity_q24: [i32; 3],
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub navigation_attitude_q30: [i32; 4],
    pub altitude_q12_km: i32,
    pub mach_q24: i32,
    pub dynamic_pressure_q14_pa: i32,
    pub total_mass_q21_kg: i32,
    pub main_propellant_q21_kg: i32,
    pub rcs_propellant_q21_kg: i32,
    pub gimbal_q15: [i16; 2],
    pub rcs_pulses: [u8; 12],
    pub command_flags: u8,
    pub command_discrete: u8,
    pub alarms: u16,
    pub transition_count: u8,
    pub checksums: [u32; GLOBAL_CHECKSUM_COUNT],
}

impl GlobalTelemetryFrame {
    pub fn encode(self, output: &mut [u8; KTT10_FRAME_LENGTH]) -> Result<(), GlobalRecordError> {
        if self.flight_mode > 6 || self.transition_count as usize > GLOBAL_TRANSITION_COUNT {
            return Err(GlobalRecordError::Range);
        }
        output.fill(0);
        p32(output, 0, self.step);
        p32(output, 4, self.mission_time_q16);
        output[8] = self.frame as u8;
        output[9] = self.segment as u8;
        output[10] = self.flight_mode;
        output[11] = self.transition_count;
        p16(output, 12, self.events);
        write_i32_array(output, 16, &self.truth_position_q12);
        write_i32_array(output, 28, &self.truth_velocity_q24);
        write_i32_array(output, 40, &self.truth_attitude_q30);
        write_i32_array(output, 56, &self.truth_angular_rate_q24);
        write_i32_array(output, 68, &self.ecef_position_q12);
        write_i32_array(output, 80, &self.ecef_velocity_q24);
        write_i32_array(output, 92, &self.navigation_position_q12);
        write_i32_array(output, 104, &self.navigation_velocity_q24);
        write_i32_array(output, 116, &self.navigation_attitude_q30);
        for (at, value) in [
            (132, self.altitude_q12_km),
            (136, self.mach_q24),
            (140, self.dynamic_pressure_q14_pa),
            (144, self.total_mass_q21_kg),
            (148, self.main_propellant_q21_kg),
            (152, self.rcs_propellant_q21_kg),
        ] {
            pi32(output, at, value);
        }
        pi16(output, 156, self.gimbal_q15[0]);
        pi16(output, 158, self.gimbal_q15[1]);
        output[160..172].copy_from_slice(&self.rcs_pulses);
        output[172] = self.command_flags;
        output[173] = self.command_discrete;
        p16(output, 174, self.alarms);
        for (index, value) in self.checksums.iter().enumerate() {
            p32(output, 176 + index * 4, *value);
        }
        let frame_crc = crc32_ieee(&output[..KTT10_FRAME_LENGTH - 4]);
        p32(output, KTT10_FRAME_LENGTH - 4, frame_crc);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalRecordError> {
        if bytes.len() != KTT10_FRAME_LENGTH {
            return Err(GlobalRecordError::Length);
        }
        if g32(bytes, KTT10_FRAME_LENGTH - 4) != crc32_ieee(&bytes[..KTT10_FRAME_LENGTH - 4]) {
            return Err(GlobalRecordError::Checksum);
        }
        if bytes[208..KTT10_FRAME_LENGTH - 4]
            .iter()
            .any(|value| *value != 0)
        {
            return Err(GlobalRecordError::Reserved);
        }
        let frame = parse_frame(bytes[8])?;
        let segment = parse_segment(bytes[9])?;
        if bytes[10] > 6 || bytes[11] as usize > GLOBAL_TRANSITION_COUNT {
            return Err(GlobalRecordError::Range);
        }
        let mut pulses = [0; 12];
        pulses.copy_from_slice(&bytes[160..172]);
        let mut checksums = [0; GLOBAL_CHECKSUM_COUNT];
        for (index, value) in checksums.iter_mut().enumerate() {
            *value = g32(bytes, 176 + index * 4);
        }
        Ok(Self {
            step: g32(bytes, 0),
            mission_time_q16: g32(bytes, 4),
            frame,
            segment,
            flight_mode: bytes[10],
            transition_count: bytes[11],
            events: g16(bytes, 12),
            truth_position_q12: read_i32_array(bytes, 16),
            truth_velocity_q24: read_i32_array(bytes, 28),
            truth_attitude_q30: read_i32_array(bytes, 40),
            truth_angular_rate_q24: read_i32_array(bytes, 56),
            ecef_position_q12: read_i32_array(bytes, 68),
            ecef_velocity_q24: read_i32_array(bytes, 80),
            navigation_position_q12: read_i32_array(bytes, 92),
            navigation_velocity_q24: read_i32_array(bytes, 104),
            navigation_attitude_q30: read_i32_array(bytes, 116),
            altitude_q12_km: gi32(bytes, 132),
            mach_q24: gi32(bytes, 136),
            dynamic_pressure_q14_pa: gi32(bytes, 140),
            total_mass_q21_kg: gi32(bytes, 144),
            main_propellant_q21_kg: gi32(bytes, 148),
            rcs_propellant_q21_kg: gi32(bytes, 152),
            gimbal_q15: [gi16(bytes, 156), gi16(bytes, 158)],
            rcs_pulses: pulses,
            command_flags: bytes[172],
            command_discrete: bytes[173],
            alarms: g16(bytes, 174),
            checksums,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalEvaluationSummary {
    pub common: EvaluationSummary,
    pub terminal_frame: ReferenceFrameId,
    pub terminal_segment: GlobalSegment,
    pub transition_count: u8,
    pub earth_identity: u32,
    pub transform_identity: u32,
    pub atmosphere_identity: u32,
    pub terminal_ecef_position_q12: [i32; 3],
    pub terminal_ecef_velocity_q24: [i32; 3],
    pub terminal_gcrf_position_q12: [i32; 3],
    pub terminal_gcrf_velocity_q24: [i32; 3],
    pub landing_geodetic_q28_q12: [i32; 3],
    pub apogee_q12_km: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
    pub max_navigation_position_error_q12_km: i32,
    pub max_navigation_velocity_error_q24_km_s: i32,
    pub max_dynamic_pressure_q14_pa: i32,
    pub max_acceleration_q28_km_s2: i32,
    pub max_mach_q24: i32,
    pub terminal_rcs_propellant_q21_kg: i32,
    pub time_identity: u32,
    pub transition_position_error_q12_km: i32,
    pub transition_velocity_error_q24_km_s: i32,
    pub transition_attitude_error_q30: i32,
    pub transition_angular_rate_error_q24: i32,
    pub global_checksums: [u32; GLOBAL_CHECKSUM_COUNT],
    pub transition_checksums: [u32; GLOBAL_TRANSITION_COUNT],
}

impl GlobalEvaluationSummary {
    pub fn encode(self, output: &mut [u8; KSR10_LENGTH]) -> Result<(), GlobalRecordError> {
        if self.common.profile != ModelProfileId::GlobalEcef6DofV1
            || self.transition_count as usize > GLOBAL_TRANSITION_COUNT
        {
            return Err(GlobalRecordError::Profile);
        }
        let identity = global_evaluation_identity(&self);
        write_header(output, RecordKind::EvaluationSummary, identity)?;
        output[32] = self.common.profile as u8;
        output[33] = self.common.outcome as u8;
        output[34] = self.common.numeric_faults;
        output[35] = self.terminal_frame as u8;
        output[36] = self.terminal_segment as u8;
        output[37] = self.transition_count;
        p32(output, 40, self.common.steps);
        p32(output, 44, self.common.metric_validity.bits());
        write_i32_array(output, 48, &self.common.terminal_state_a);
        write_i32_array(output, 60, &self.common.terminal_state_b);
        write_i32_array(output, 72, &self.common.metrics);
        p32(output, 200, self.common.events);
        write_u32_array(output, 204, &self.common.identities);
        write_u32_array(output, 228, &self.common.source_checksums);
        for (at, value) in [
            (248, self.earth_identity),
            (252, self.transform_identity),
            (256, self.atmosphere_identity),
        ] {
            p32(output, at, value);
        }
        write_i32_array(output, 260, &self.terminal_ecef_position_q12);
        write_i32_array(output, 272, &self.terminal_ecef_velocity_q24);
        write_i32_array(output, 284, &self.terminal_gcrf_position_q12);
        write_i32_array(output, 296, &self.terminal_gcrf_velocity_q24);
        write_i32_array(output, 308, &self.landing_geodetic_q28_q12);
        for (index, value) in [
            self.apogee_q12_km,
            self.downrange_q12_km,
            self.crossrange_q12_km,
            self.max_navigation_position_error_q12_km,
            self.max_navigation_velocity_error_q24_km_s,
            self.max_dynamic_pressure_q14_pa,
            self.max_acceleration_q28_km_s2,
            self.max_mach_q24,
            self.terminal_rcs_propellant_q21_kg,
        ]
        .iter()
        .enumerate()
        {
            pi32(output, 320 + index * 4, *value);
        }
        p32(output, 356, self.time_identity);
        for (index, value) in [
            self.transition_position_error_q12_km,
            self.transition_velocity_error_q24_km_s,
            self.transition_attitude_error_q30,
            self.transition_angular_rate_error_q24,
        ]
        .iter()
        .enumerate()
        {
            pi32(output, 360 + index * 4, *value);
        }
        write_u32_array(output, 376, &self.global_checksums);
        write_u32_array(output, 408, &self.transition_checksums);
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalRecordError> {
        validate_record(bytes, RecordKind::EvaluationSummary)?;
        if bytes[32] != ModelProfileId::GlobalEcef6DofV1 as u8
            || bytes[38..40].iter().any(|value| *value != 0)
            || bytes[424..KSR10_LENGTH - 4].iter().any(|value| *value != 0)
        {
            return Err(GlobalRecordError::Reserved);
        }
        let mut common = EvaluationSummary::empty(ModelProfileId::GlobalEcef6DofV1);
        common.outcome = parse_outcome(bytes[33])?;
        common.numeric_faults = bytes[34];
        common.steps = g32(bytes, 40);
        common.metric_validity = MetricValidity::from_bits(g32(bytes, 44));
        common.terminal_state_a = read_i32_array(bytes, 48);
        common.terminal_state_b = read_i32_array(bytes, 60);
        common.metrics = read_i32_array(bytes, 72);
        common.events = g32(bytes, 200);
        common.identities = read_u32_array(bytes, 204);
        common.source_checksums = read_u32_array(bytes, 228);
        let mut global_checksums = [0; GLOBAL_CHECKSUM_COUNT];
        for (index, value) in global_checksums.iter_mut().enumerate() {
            *value = g32(bytes, 376 + index * 4);
        }
        let mut transition_checksums = [0; GLOBAL_TRANSITION_COUNT];
        for (index, value) in transition_checksums.iter_mut().enumerate() {
            *value = g32(bytes, 408 + index * 4);
        }
        let result = Self {
            common,
            terminal_frame: parse_frame(bytes[35])?,
            terminal_segment: parse_segment(bytes[36])?,
            transition_count: bytes[37],
            earth_identity: g32(bytes, 248),
            transform_identity: g32(bytes, 252),
            atmosphere_identity: g32(bytes, 256),
            terminal_ecef_position_q12: read_i32_array(bytes, 260),
            terminal_ecef_velocity_q24: read_i32_array(bytes, 272),
            terminal_gcrf_position_q12: read_i32_array(bytes, 284),
            terminal_gcrf_velocity_q24: read_i32_array(bytes, 296),
            landing_geodetic_q28_q12: read_i32_array(bytes, 308),
            apogee_q12_km: gi32(bytes, 320),
            downrange_q12_km: gi32(bytes, 324),
            crossrange_q12_km: gi32(bytes, 328),
            max_navigation_position_error_q12_km: gi32(bytes, 332),
            max_navigation_velocity_error_q24_km_s: gi32(bytes, 336),
            max_dynamic_pressure_q14_pa: gi32(bytes, 340),
            max_acceleration_q28_km_s2: gi32(bytes, 344),
            max_mach_q24: gi32(bytes, 348),
            terminal_rcs_propellant_q21_kg: gi32(bytes, 352),
            time_identity: g32(bytes, 356),
            transition_position_error_q12_km: gi32(bytes, 360),
            transition_velocity_error_q24_km_s: gi32(bytes, 364),
            transition_attitude_error_q30: gi32(bytes, 368),
            transition_angular_rate_error_q24: gi32(bytes, 372),
            global_checksums,
            transition_checksums,
        };
        if result.transition_count as usize > GLOBAL_TRANSITION_COUNT
            || result.earth_identity == 0
            || result.transform_identity == 0
            || result.atmosphere_identity == 0
            || result.time_identity == 0
        {
            return Err(GlobalRecordError::Identity);
        }
        Ok(result)
    }
}

pub fn global_evaluation_identity(summary: &GlobalEvaluationSummary) -> u32 {
    let mut hash = 2_166_136_261u32;
    for word in summary
        .common
        .identities
        .into_iter()
        .chain(summary.common.source_checksums)
        .chain([
            summary.earth_identity,
            summary.transform_identity,
            summary.atmosphere_identity,
            summary.time_identity,
        ])
    {
        hash = fnv_word(hash, word);
    }
    hash
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalPlotHeader {
    pub identity: u32,
    pub evaluation_identity: u32,
    pub point_count: u16,
    pub stride_releases: u16,
}

impl GlobalPlotHeader {
    pub fn encode(self, output: &mut [u8; KPH10_HEADER_LENGTH]) -> Result<(), GlobalRecordError> {
        if self.identity == 0
            || self.evaluation_identity == 0
            || self.point_count == 0
            || self.stride_releases == 0
        {
            return Err(GlobalRecordError::Range);
        }
        write_header(output, RecordKind::PlotHeader, self.identity)?;
        output[32] = ModelProfileId::GlobalEcef6DofV1 as u8;
        p16(output, 34, KPH10_POINT_LENGTH as u16);
        p32(output, 36, self.evaluation_identity);
        p16(output, 40, self.point_count);
        p16(output, 42, self.stride_releases);
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalRecordError> {
        let identity = validate_record(bytes, RecordKind::PlotHeader)?;
        if bytes[32] != ModelProfileId::GlobalEcef6DofV1 as u8
            || bytes[33] != 0
            || g16(bytes, 34) as usize != KPH10_POINT_LENGTH
            || bytes[44..KPH10_HEADER_LENGTH - 4]
                .iter()
                .any(|value| *value != 0)
        {
            return Err(GlobalRecordError::Reserved);
        }
        let header = Self {
            identity,
            evaluation_identity: g32(bytes, 36),
            point_count: g16(bytes, 40),
            stride_releases: g16(bytes, 42),
        };
        if header.evaluation_identity == 0 || header.point_count == 0 || header.stride_releases == 0
        {
            return Err(GlobalRecordError::Range);
        }
        Ok(header)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalPlotPoint {
    pub mission_time_q16: u32,
    pub latitude_q28_rad: i32,
    pub longitude_q28_rad: i32,
    pub altitude_q12_km: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
    pub speed_q24_km_s: i32,
    pub frame: ReferenceFrameId,
    pub segment: GlobalSegment,
    pub events: u16,
    pub truth_checksum: u32,
}

impl GlobalPlotPoint {
    pub fn encode(self, output: &mut [u8; KPH10_POINT_LENGTH]) -> Result<(), GlobalRecordError> {
        output.fill(0);
        p32(output, 0, self.mission_time_q16);
        for (index, value) in [
            self.latitude_q28_rad,
            self.longitude_q28_rad,
            self.altitude_q12_km,
            self.downrange_q12_km,
            self.crossrange_q12_km,
            self.speed_q24_km_s,
        ]
        .iter()
        .enumerate()
        {
            pi32(output, 4 + index * 4, *value);
        }
        output[28] = self.frame as u8;
        output[29] = self.segment as u8;
        p16(output, 30, self.events);
        p32(output, 32, self.truth_checksum);
        let point_crc = crc32_ieee(&output[..KPH10_POINT_LENGTH - 4]);
        p32(output, KPH10_POINT_LENGTH - 4, point_crc);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalRecordError> {
        if bytes.len() != KPH10_POINT_LENGTH {
            return Err(GlobalRecordError::Length);
        }
        if g32(bytes, KPH10_POINT_LENGTH - 4) != crc32_ieee(&bytes[..KPH10_POINT_LENGTH - 4]) {
            return Err(GlobalRecordError::Checksum);
        }
        if bytes[36..KPH10_POINT_LENGTH - 4]
            .iter()
            .any(|value| *value != 0)
        {
            return Err(GlobalRecordError::Reserved);
        }
        Ok(Self {
            mission_time_q16: g32(bytes, 0),
            latitude_q28_rad: gi32(bytes, 4),
            longitude_q28_rad: gi32(bytes, 8),
            altitude_q12_km: gi32(bytes, 12),
            downrange_q12_km: gi32(bytes, 16),
            crossrange_q12_km: gi32(bytes, 20),
            speed_q24_km_s: gi32(bytes, 24),
            frame: parse_frame(bytes[28])?,
            segment: parse_segment(bytes[29])?,
            events: g16(bytes, 30),
            truth_checksum: g32(bytes, 32),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalCampaignConfig {
    pub identity: u32,
    pub earth_identity: u32,
    pub transform_identity: u32,
    pub atmosphere_identity: u32,
    pub vehicle_identity: u32,
    pub mission_identity: u32,
    pub avionics_identity: u32,
    pub master_seed: u32,
    pub run_count: u16,
    pub catalog_version: u16,
    pub variation_mask: u32,
}

impl GlobalCampaignConfig {
    pub fn encode(self, output: &mut [u8; KSC10_LENGTH]) -> Result<(), GlobalRecordError> {
        if self.identity == 0
            || self.earth_identity == 0
            || self.transform_identity == 0
            || self.atmosphere_identity == 0
            || self.vehicle_identity == 0
            || self.mission_identity == 0
            || self.avionics_identity == 0
            || !matches!(self.run_count, 64 | 256)
            || self.catalog_version == 0
            || self.variation_mask == 0
        {
            return Err(GlobalRecordError::Range);
        }
        write_header(output, RecordKind::Campaign, self.identity)?;
        output[32] = ModelProfileId::GlobalEcef6DofV1 as u8;
        for (at, value) in [
            (36, self.earth_identity),
            (40, self.transform_identity),
            (44, self.atmosphere_identity),
            (48, self.vehicle_identity),
            (52, self.mission_identity),
            (56, self.avionics_identity),
            (60, self.master_seed),
        ] {
            p32(output, at, value);
        }
        p16(output, 64, self.run_count);
        p16(output, 66, self.catalog_version);
        p32(output, 68, self.variation_mask);
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalRecordError> {
        let identity = validate_record(bytes, RecordKind::Campaign)?;
        if bytes[32] != ModelProfileId::GlobalEcef6DofV1 as u8
            || bytes[33..36].iter().any(|value| *value != 0)
            || bytes[72..KSC10_LENGTH - 4].iter().any(|value| *value != 0)
        {
            return Err(GlobalRecordError::Reserved);
        }
        let result = Self {
            identity,
            earth_identity: g32(bytes, 36),
            transform_identity: g32(bytes, 40),
            atmosphere_identity: g32(bytes, 44),
            vehicle_identity: g32(bytes, 48),
            mission_identity: g32(bytes, 52),
            avionics_identity: g32(bytes, 56),
            master_seed: g32(bytes, 60),
            run_count: g16(bytes, 64),
            catalog_version: g16(bytes, 66),
            variation_mask: g32(bytes, 68),
        };
        if result.earth_identity == 0
            || result.transform_identity == 0
            || result.atmosphere_identity == 0
            || result.vehicle_identity == 0
            || result.mission_identity == 0
            || result.avionics_identity == 0
            || !matches!(result.run_count, 64 | 256)
            || result.catalog_version == 0
            || result.variation_mask == 0
        {
            return Err(GlobalRecordError::Range);
        }
        Ok(result)
    }
}

fn parse_frame(value: u8) -> Result<ReferenceFrameId, GlobalRecordError> {
    match value {
        1 => Ok(ReferenceFrameId::LocalEnuV1),
        2 => Ok(ReferenceFrameId::EarthFixedEcefV1),
        3 => Ok(ReferenceFrameId::EarthInertialEciV1),
        _ => Err(GlobalRecordError::Frame),
    }
}

fn parse_segment(value: u8) -> Result<GlobalSegment, GlobalRecordError> {
    match value {
        1 => Ok(GlobalSegment::LocalLaunch),
        2 => Ok(GlobalSegment::EcefAscent),
        3 => Ok(GlobalSegment::EciCoast),
        4 => Ok(GlobalSegment::EcefEntry),
        5 => Ok(GlobalSegment::LocalRecovery),
        _ => Err(GlobalRecordError::Segment),
    }
}

fn parse_outcome(value: u8) -> Result<EvaluationOutcome, GlobalRecordError> {
    match value {
        0 => Ok(EvaluationOutcome::Complete),
        1 => Ok(EvaluationOutcome::StableOrbit),
        2 => Ok(EvaluationOutcome::CompleteNotOrbit),
        3 => Ok(EvaluationOutcome::GroundContact),
        4 => Ok(EvaluationOutcome::Aborted),
        5 => Ok(EvaluationOutcome::NumericFault),
        6 => Ok(EvaluationOutcome::StepLimit),
        7 => Ok(EvaluationOutcome::NoLiftoff),
        8 => Ok(EvaluationOutcome::ConfigurationFault),
        9 => Ok(EvaluationOutcome::RecoveryIncomplete),
        10 => Ok(EvaluationOutcome::ModelEnvelopeExceeded),
        _ => Err(GlobalRecordError::Outcome),
    }
}

fn write_header(
    output: &mut [u8],
    kind: RecordKind,
    identity: u32,
) -> Result<(), GlobalRecordError> {
    if output.len() != kind.length() {
        return Err(GlobalRecordError::Length);
    }
    if identity == 0 {
        return Err(GlobalRecordError::Identity);
    }
    output.fill(0);
    output[..5].copy_from_slice(&kind.magic());
    p16(output, 6, VERSION);
    p16(output, 8, 32);
    p16(output, 10, kind as u16);
    p32(output, 12, output.len() as u32);
    p32(output, 16, PHASE10_CONTRACT_ID);
    p32(output, 20, identity);
    Ok(())
}

fn validate_record(bytes: &[u8], kind: RecordKind) -> Result<u32, GlobalRecordError> {
    if bytes.len() != kind.length() {
        return Err(GlobalRecordError::Length);
    }
    if bytes[..5] != kind.magic() || bytes[5] != 0 {
        return Err(GlobalRecordError::Magic);
    }
    if g16(bytes, 6) != VERSION || g16(bytes, 8) != 32 {
        return Err(GlobalRecordError::Version);
    }
    if g16(bytes, 10) != kind as u16 || g32(bytes, 12) as usize != kind.length() {
        return Err(GlobalRecordError::Kind);
    }
    if g32(bytes, 16) != PHASE10_CONTRACT_ID {
        return Err(GlobalRecordError::Contract);
    }
    if bytes[24..32].iter().any(|value| *value != 0) {
        return Err(GlobalRecordError::Reserved);
    }
    let identity = g32(bytes, 20);
    if identity == 0 {
        return Err(GlobalRecordError::Identity);
    }
    if g32(bytes, bytes.len() - 4) != crc32_ieee(&bytes[..bytes.len() - 4]) {
        return Err(GlobalRecordError::Checksum);
    }
    Ok(identity)
}

fn seal(output: &mut [u8]) {
    let at = output.len() - 4;
    p32(output, at, crc32_ieee(&output[..at]));
}

fn fnv_word(mut hash: u32, word: u32) -> u32 {
    for byte in word.to_le_bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    hash
}

fn write_i32_array(output: &mut [u8], at: usize, values: &[i32]) {
    for (index, value) in values.iter().enumerate() {
        pi32(output, at + index * 4, *value);
    }
}

fn write_u32_array(output: &mut [u8], at: usize, values: &[u32]) {
    for (index, value) in values.iter().enumerate() {
        p32(output, at + index * 4, *value);
    }
}

fn read_i32_array<const N: usize>(bytes: &[u8], at: usize) -> [i32; N] {
    let mut result = [0; N];
    for (index, value) in result.iter_mut().enumerate() {
        *value = gi32(bytes, at + index * 4);
    }
    result
}

fn read_u32_array<const N: usize>(bytes: &[u8], at: usize) -> [u32; N] {
    let mut result = [0; N];
    for (index, value) in result.iter_mut().enumerate() {
        *value = g32(bytes, at + index * 4);
    }
    result
}

fn p16(output: &mut [u8], at: usize, value: u16) {
    output[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn pi16(output: &mut [u8], at: usize, value: i16) {
    p16(output, at, value as u16);
}

fn p32(output: &mut [u8], at: usize, value: u32) {
    output[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn pi32(output: &mut [u8], at: usize, value: i32) {
    p32(output, at, value as u32);
}

fn g16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn gi16(bytes: &[u8], at: usize) -> i16 {
    g16(bytes, at) as i16
}

fn g32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn gi32(bytes: &[u8], at: usize) -> i32 {
    g32(bytes, at) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::{
        MetricSlot, MetricValidity, EVALUATION_CHECKSUM_COUNT, EVALUATION_IDENTITY_COUNT,
        EVALUATION_METRIC_COUNT,
    };

    fn common() -> EvaluationSummary {
        let mut value = EvaluationSummary::empty(ModelProfileId::GlobalEcef6DofV1);
        value.outcome = EvaluationOutcome::GroundContact;
        value.steps = 12_345;
        value.identities = [1, 2, 3, 4, 5, 6];
        value.source_checksums = [7, 8, 9, 10, 11];
        value.set_metric(MetricSlot::ApogeeAltitude, 205 << 12);
        value
    }

    #[test]
    fn telemetry_header_and_frame_are_strict() {
        let header = GlobalTelemetryHeader {
            identity: 1,
            earth_identity: 2,
            transform_identity: 3,
            atmosphere_identity: 4,
            vehicle_identity: 5,
            mission_identity: 6,
            avionics_identity: 7,
            case_seed: 8,
            telemetry_period_q16: 2_048,
            max_mission_time_q16: 2_700 << 16,
        };
        let mut header_bytes = [0; KTT10_HEADER_LENGTH];
        header.encode(&mut header_bytes).unwrap();
        assert_eq!(
            GlobalTelemetryHeader::decode(&header_bytes).unwrap(),
            header
        );
        header_bytes[80] = 1;
        let crc = crc32_ieee(&header_bytes[..KTT10_HEADER_LENGTH - 4]);
        p32(&mut header_bytes, KTT10_HEADER_LENGTH - 4, crc);
        assert_eq!(
            GlobalTelemetryHeader::decode(&header_bytes),
            Err(GlobalRecordError::Reserved)
        );

        let frame = GlobalTelemetryFrame {
            step: 1,
            mission_time_q16: 2_048,
            frame: ReferenceFrameId::EarthFixedEcefV1,
            segment: GlobalSegment::EcefAscent,
            flight_mode: 1,
            events: 3,
            truth_position_q12: [1, 2, 3],
            truth_velocity_q24: [4, 5, 6],
            truth_attitude_q30: [1 << 30, 0, 0, 0],
            truth_angular_rate_q24: [7, 8, 9],
            ecef_position_q12: [10, 11, 12],
            ecef_velocity_q24: [13, 14, 15],
            navigation_position_q12: [16, 17, 18],
            navigation_velocity_q24: [19, 20, 21],
            navigation_attitude_q30: [1 << 30, 0, 0, 0],
            altitude_q12_km: 22,
            mach_q24: 23,
            dynamic_pressure_q14_pa: 24,
            total_mass_q21_kg: 25,
            main_propellant_q21_kg: 26,
            rcs_propellant_q21_kg: 27,
            gimbal_q15: [28, 29],
            rcs_pulses: [1; 12],
            command_flags: 30,
            command_discrete: 31,
            alarms: 32,
            transition_count: 1,
            checksums: [33; GLOBAL_CHECKSUM_COUNT],
        };
        let mut bytes = [0; KTT10_FRAME_LENGTH];
        frame.encode(&mut bytes).unwrap();
        assert_eq!(GlobalTelemetryFrame::decode(&bytes).unwrap(), frame);
        bytes[100] ^= 1;
        assert_eq!(
            GlobalTelemetryFrame::decode(&bytes),
            Err(GlobalRecordError::Checksum)
        );
    }

    #[test]
    fn global_summary_round_trips_and_rejects_reserved_data() {
        let value = GlobalEvaluationSummary {
            common: common(),
            terminal_frame: ReferenceFrameId::LocalEnuV1,
            terminal_segment: GlobalSegment::LocalRecovery,
            transition_count: 4,
            earth_identity: 12,
            transform_identity: 13,
            atmosphere_identity: 14,
            terminal_ecef_position_q12: [15; 3],
            terminal_ecef_velocity_q24: [16; 3],
            terminal_gcrf_position_q12: [17; 3],
            terminal_gcrf_velocity_q24: [18; 3],
            landing_geodetic_q28_q12: [19; 3],
            apogee_q12_km: 20,
            downrange_q12_km: 21,
            crossrange_q12_km: 22,
            max_navigation_position_error_q12_km: 23,
            max_navigation_velocity_error_q24_km_s: 24,
            max_dynamic_pressure_q14_pa: 25,
            max_acceleration_q28_km_s2: 26,
            max_mach_q24: 27,
            terminal_rcs_propellant_q21_kg: 28,
            time_identity: 29,
            transition_position_error_q12_km: 30,
            transition_velocity_error_q24_km_s: 31,
            transition_attitude_error_q30: 32,
            transition_angular_rate_error_q24: 33,
            global_checksums: [34; GLOBAL_CHECKSUM_COUNT],
            transition_checksums: [35; GLOBAL_TRANSITION_COUNT],
        };
        let mut bytes = [0; KSR10_LENGTH];
        value.encode(&mut bytes).unwrap();
        assert_eq!(GlobalEvaluationSummary::decode(&bytes).unwrap(), value);
        bytes[430] = 1;
        seal(&mut bytes);
        assert_eq!(
            GlobalEvaluationSummary::decode(&bytes),
            Err(GlobalRecordError::Reserved)
        );
    }

    #[test]
    fn plot_and_campaign_contracts_are_bounded() {
        let header = GlobalPlotHeader {
            identity: 1,
            evaluation_identity: 2,
            point_count: 32,
            stride_releases: 16,
        };
        let mut header_bytes = [0; KPH10_HEADER_LENGTH];
        header.encode(&mut header_bytes).unwrap();
        assert_eq!(GlobalPlotHeader::decode(&header_bytes).unwrap(), header);
        let point = GlobalPlotPoint {
            mission_time_q16: 1,
            latitude_q28_rad: 2,
            longitude_q28_rad: 3,
            altitude_q12_km: 4,
            downrange_q12_km: 5,
            crossrange_q12_km: 6,
            speed_q24_km_s: 7,
            frame: ReferenceFrameId::EarthInertialEciV1,
            segment: GlobalSegment::EciCoast,
            events: 8,
            truth_checksum: 9,
        };
        let mut point_bytes = [0; KPH10_POINT_LENGTH];
        point.encode(&mut point_bytes).unwrap();
        assert_eq!(GlobalPlotPoint::decode(&point_bytes).unwrap(), point);
        let config = GlobalCampaignConfig {
            identity: 10,
            earth_identity: 11,
            transform_identity: 12,
            atmosphere_identity: 13,
            vehicle_identity: 14,
            mission_identity: 15,
            avionics_identity: 16,
            master_seed: 0x4b53_41a0,
            run_count: 256,
            catalog_version: 1,
            variation_mask: 0x000f_ffff,
        };
        let mut config_bytes = [0; KSC10_LENGTH];
        config.encode(&mut config_bytes).unwrap();
        assert_eq!(GlobalCampaignConfig::decode(&config_bytes).unwrap(), config);
        config_bytes[100] = 1;
        seal(&mut config_bytes);
        assert_eq!(
            GlobalCampaignConfig::decode(&config_bytes),
            Err(GlobalRecordError::Reserved)
        );
    }

    #[test]
    fn metric_layout_is_not_silently_truncated() {
        assert_eq!(EVALUATION_METRIC_COUNT, 32);
        assert_eq!(EVALUATION_IDENTITY_COUNT, 6);
        assert_eq!(EVALUATION_CHECKSUM_COUNT, 5);
        let validity = MetricValidity::from_bits(u32::MAX);
        assert_eq!(validity.bits(), u32::MAX);
    }
}
