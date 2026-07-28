//! Portable Phase 10 nominal capture and strict artifact construction.

use crate::global_fixtures::GlobalFixtureSet;
use ksa64_core::numeric::{magnitude3_floor, NumericStatus};
use ksa64_core::phase10_environment::ecef_to_geodetic;
use ksa64_core::phase10_telemetry::{
    global_evaluation_identity, GlobalEvaluationSummary, GlobalPlotHeader, GlobalPlotPoint,
    GlobalTelemetryFrame, GlobalTelemetryHeader, KPH10_HEADER_LENGTH, KPH10_POINT_LENGTH,
    KSR10_LENGTH, KTT10_FRAME_LENGTH, KTT10_HEADER_LENGTH,
};
use ksa64_sim::phase10::{FrameTransitionRecord, GlobalWorldError};
use ksa64_sim::phase10_avionics::{reference_global_flight_config, GlobalSensorFaults};
use ksa64_sim::phase10_avionics::{GlobalAvionicsMission, GlobalFlightReleaseProcessor};
use ksa64_sim::phase10_evaluation::{evaluate_global, GlobalEvaluationRequest};

pub const PHASE10_NOMINAL_CASE_SEED: u32 = 0x4b53_41a0;
pub const PHASE10_NOMINAL_SESSION: u16 = 0x10a0;
pub const PHASE10_TELEMETRY_IDENTITY: u32 = 0x10a1_0001;
pub const PHASE10_PLOT_IDENTITY: u32 = 0x10a1_0002;
pub const PHASE10_RECORD_STRIDE_RELEASES: u16 = 32;
pub const PHASE10_OBSERVER_STRIDE_RELEASES: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalMissionUpdate {
    pub frame: GlobalTelemetryFrame,
    pub plot: GlobalPlotPoint,
    pub release: u32,
}

#[derive(Clone, Debug)]
pub struct GlobalMissionCapture {
    pub telemetry_header: GlobalTelemetryHeader,
    pub plot_identity: u32,
    pub frames: Vec<GlobalTelemetryFrame>,
    pub plot_points: Vec<GlobalPlotPoint>,
    pub summary: GlobalEvaluationSummary,
    pub transition_records: [FrameTransitionRecord; 4],
    pub releases: u32,
    pub wall_seconds: f64,
}

pub fn capture_nominal_global_mission_portable<F>(
    mut observer: F,
) -> Result<GlobalMissionCapture, GlobalWorldError>
where
    F: FnMut(&GlobalMissionUpdate),
{
    let fixtures = GlobalFixtureSet::embedded();
    let initial_world = ksa64_sim::phase10::GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )?;
    let flight_config = reference_global_flight_config(
        PHASE10_NOMINAL_SESSION,
        initial_world.active_state()?,
        fixtures.mission,
    )?;
    let mut runner = GlobalAvionicsMission::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
        flight_config,
        GlobalSensorFaults::NONE,
        PHASE10_NOMINAL_CASE_SEED,
    )?;
    let mut frames = Vec::new();
    let mut plot_points = Vec::new();
    let mut releases = 0u32;
    let mut previous_transitions = 0u8;
    loop {
        let flight = runner.release()?;
        let snapshot = runner.world().snapshot()?;
        releases = releases.saturating_add(1);
        let update = mission_update(&runner, snapshot, flight, releases)?;
        let important = releases == 1
            || releases.is_multiple_of(u32::from(PHASE10_RECORD_STRIDE_RELEASES))
            || snapshot.events != 0
            || snapshot.transition_count != previous_transitions
            || runner.world().is_complete();
        if important {
            frames.push(update.frame);
            plot_points.push(update.plot);
        }
        if important || releases.is_multiple_of(PHASE10_OBSERVER_STRIDE_RELEASES) {
            observer(&update);
        }
        previous_transitions = snapshot.transition_count;
        if runner.world().is_complete() {
            break;
        }
        runner.advance_to_next_release()?;
        if releases > 460_800 {
            return Err(GlobalWorldError::Timeout);
        }
    }
    let transitions = *runner.world().transitions();

    let evaluation_world = ksa64_sim::phase10::GlobalWorldMachine::new(
        &fixtures.earth,
        &fixtures.transforms,
        &fixtures.atmosphere,
        &fixtures.vehicle,
        fixtures.mission,
    )?;
    let evaluation_flight = reference_global_flight_config(
        PHASE10_NOMINAL_SESSION,
        evaluation_world.active_state()?,
        fixtures.mission,
    )?;
    let summary = evaluate_global(GlobalEvaluationRequest {
        earth: &fixtures.earth,
        transforms: &fixtures.transforms,
        atmosphere: &fixtures.atmosphere,
        vehicle: &fixtures.vehicle,
        mission: fixtures.mission,
        avionics: evaluation_flight,
        uncertainty: GlobalSensorFaults::NONE,
        case_seed: PHASE10_NOMINAL_CASE_SEED,
    })?;
    let telemetry_header = GlobalTelemetryHeader {
        identity: PHASE10_TELEMETRY_IDENTITY,
        earth_identity: fixtures.earth.identity,
        transform_identity: fixtures.transforms.identity,
        atmosphere_identity: fixtures.atmosphere.identity,
        vehicle_identity: fixtures.vehicle.identity,
        mission_identity: fixtures.mission.identity,
        avionics_identity: ksa64_flight::phase10::GLOBAL_FLIGHT_CONTRACT_ID,
        case_seed: PHASE10_NOMINAL_CASE_SEED,
        telemetry_period_q16: u32::from(PHASE10_RECORD_STRIDE_RELEASES) * 2_048,
        max_mission_time_q16: fixtures.mission.max_mission_time_q16_s,
    };
    Ok(GlobalMissionCapture {
        telemetry_header,
        plot_identity: PHASE10_PLOT_IDENTITY,
        frames,
        plot_points,
        summary,
        transition_records: transitions,
        releases,
        wall_seconds: 0.0,
    })
}

pub(crate) fn mission_update<P: GlobalFlightReleaseProcessor>(
    runner: &GlobalAvionicsMission<'_, P>,
    snapshot: ksa64_sim::phase10::GlobalWorldSnapshot,
    flight: ksa64_flight::phase10::GlobalFlightEvidence,
    release: u32,
) -> Result<GlobalMissionUpdate, GlobalWorldError> {
    mission_update_with_case(runner, snapshot, flight, release, PHASE10_NOMINAL_CASE_SEED)
}

pub(crate) fn mission_update_with_case<P: GlobalFlightReleaseProcessor>(
    runner: &GlobalAvionicsMission<'_, P>,
    snapshot: ksa64_sim::phase10::GlobalWorldSnapshot,
    flight: ksa64_flight::phase10::GlobalFlightEvidence,
    release: u32,
    case_seed: u32,
) -> Result<GlobalMissionUpdate, GlobalWorldError> {
    let state = snapshot.state;
    let ecef = runner.world().ecef_state_public()?;
    let geodetic = ecef_to_geodetic(ecef.position)?;
    let offset = runner.world().ecef_to_launch_offset_public(ecef)?;
    let mut status = NumericStatus::CLEAR;
    let speed = magnitude3_floor(
        ecef.velocity.x(),
        ecef.velocity.y(),
        ecef.velocity.z(),
        &mut status,
    );
    if !status.is_clear() || speed > i32::MAX as u32 {
        return Err(GlobalWorldError::Numeric);
    }
    let truth_attitude = state.attitude;
    let frame = GlobalTelemetryFrame {
        step: release,
        mission_time_q16: state.time.raw(),
        frame: snapshot.frame,
        segment: snapshot.segment,
        flight_mode: flight.mode as u8,
        events: snapshot.events,
        truth_position_q12: vector(state.position),
        truth_velocity_q24: vector(state.velocity),
        truth_attitude_q30: [
            truth_attitude.w(),
            truth_attitude.x(),
            truth_attitude.y(),
            truth_attitude.z(),
        ],
        truth_angular_rate_q24: vector(state.angular_rate),
        ecef_position_q12: vector(ecef.position),
        ecef_velocity_q24: vector(ecef.velocity),
        navigation_position_q12: flight.navigation.position_q12,
        navigation_velocity_q24: flight.navigation.velocity_q24,
        navigation_attitude_q30: flight.navigation.attitude_q30,
        altitude_q12_km: snapshot.altitude_q12_km,
        mach_q24: snapshot.mach_q24,
        dynamic_pressure_q14_pa: snapshot.dynamic_pressure_q14_pa,
        total_mass_q21_kg: snapshot.total_mass_q21_kg,
        main_propellant_q21_kg: snapshot.main_propellant_q21_kg,
        rcs_propellant_q21_kg: snapshot.rcs_propellant_q21_kg,
        gimbal_q15: flight.command.gimbal_q15,
        rcs_pulses: flight.command.rcs_pulse_quanta,
        command_flags: flight.command.flags,
        command_discrete: flight.command.discrete,
        alarms: flight.alarms,
        transition_count: snapshot.transition_count,
        checksums: [
            snapshot.checksum,
            flight.navigation.checksum,
            flight.flight_checksum,
            flight.sensor_checksum,
            flight.command.command_checksum,
            flight.status.map_or(0, |status| status.flight_checksum),
            flight.deadline_misses.into(),
            case_seed,
        ],
    };
    let plot = GlobalPlotPoint {
        mission_time_q16: state.time.raw(),
        latitude_q28_rad: geodetic.latitude_q28_rad,
        longitude_q28_rad: geodetic.longitude_q28_rad,
        altitude_q12_km: geodetic.height_q12_km,
        downrange_q12_km: offset.x(),
        crossrange_q12_km: offset.y(),
        speed_q24_km_s: speed as i32,
        frame: snapshot.frame,
        segment: snapshot.segment,
        events: snapshot.events,
        truth_checksum: snapshot.checksum,
    };
    Ok(GlobalMissionUpdate {
        frame,
        plot,
        release,
    })
}

fn vector<const FRACTIONAL_BITS: u8>(
    value: ksa64_core::spatial_numeric::FixedVec3<FRACTIONAL_BITS>,
) -> [i32; 3] {
    [value.x(), value.y(), value.z()]
}

pub fn encode_ktt10(capture: &GlobalMissionCapture) -> Result<Vec<u8>, String> {
    let mut output = vec![0; KTT10_HEADER_LENGTH + capture.frames.len() * KTT10_FRAME_LENGTH];
    let mut header = [0; KTT10_HEADER_LENGTH];
    capture
        .telemetry_header
        .encode(&mut header)
        .map_err(|error| format!("{error:?}"))?;
    output[..KTT10_HEADER_LENGTH].copy_from_slice(&header);
    for (index, frame) in capture.frames.iter().enumerate() {
        let mut bytes = [0; KTT10_FRAME_LENGTH];
        frame
            .encode(&mut bytes)
            .map_err(|error| format!("{error:?}"))?;
        let offset = KTT10_HEADER_LENGTH + index * KTT10_FRAME_LENGTH;
        output[offset..offset + KTT10_FRAME_LENGTH].copy_from_slice(&bytes);
    }
    Ok(output)
}

pub fn encode_kph10(capture: &GlobalMissionCapture) -> Result<Vec<u8>, String> {
    let point_count: u16 = capture
        .plot_points
        .len()
        .try_into()
        .map_err(|_| "too many KPH10 points")?;
    let header = GlobalPlotHeader {
        identity: capture.plot_identity,
        evaluation_identity: global_evaluation_identity(&capture.summary),
        point_count,
        stride_releases: PHASE10_RECORD_STRIDE_RELEASES,
    };
    let mut output = vec![0; KPH10_HEADER_LENGTH + capture.plot_points.len() * KPH10_POINT_LENGTH];
    let mut header_bytes = [0; KPH10_HEADER_LENGTH];
    header
        .encode(&mut header_bytes)
        .map_err(|error| format!("{error:?}"))?;
    output[..KPH10_HEADER_LENGTH].copy_from_slice(&header_bytes);
    for (index, point) in capture.plot_points.iter().enumerate() {
        let mut bytes = [0; KPH10_POINT_LENGTH];
        point
            .encode(&mut bytes)
            .map_err(|error| format!("{error:?}"))?;
        let offset = KPH10_HEADER_LENGTH + index * KPH10_POINT_LENGTH;
        output[offset..offset + KPH10_POINT_LENGTH].copy_from_slice(&bytes);
    }
    Ok(output)
}

pub fn encode_ksr10(capture: &GlobalMissionCapture) -> Result<[u8; KSR10_LENGTH], String> {
    let mut output = [0; KSR10_LENGTH];
    capture
        .summary
        .encode(&mut output)
        .map_err(|error| format!("{error:?}"))?;
    Ok(output)
}

pub const fn q16(raw: u32) -> f64 {
    raw as f64 / 65_536.0
}
pub const fn q12(raw: i32) -> f64 {
    raw as f64 / 4_096.0
}
pub const fn q14(raw: i32) -> f64 {
    raw as f64 / 16_384.0
}
pub const fn q21(raw: i32) -> f64 {
    raw as f64 / 2_097_152.0
}
pub const fn q24(raw: i32) -> f64 {
    raw as f64 / 16_777_216.0
}
pub fn q28_radians_to_degrees(raw: i32) -> f64 {
    raw as f64 / 268_435_456.0 * 180.0 / std::f64::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase10_telemetry::{
        GlobalPlotHeader, GlobalPlotPoint, GlobalTelemetryFrame, GlobalTelemetryHeader,
    };

    #[test]
    fn strict_artifacts_round_trip_without_platform_services() {
        let capture = capture_nominal_global_mission_portable(|_| {}).unwrap();
        let before = capture.summary;
        let telemetry = encode_ktt10(&capture).unwrap();
        GlobalTelemetryHeader::decode(&telemetry[..KTT10_HEADER_LENGTH]).unwrap();
        for frame in telemetry[KTT10_HEADER_LENGTH..].chunks_exact(KTT10_FRAME_LENGTH) {
            GlobalTelemetryFrame::decode(frame).unwrap();
        }
        let plot = encode_kph10(&capture).unwrap();
        GlobalPlotHeader::decode(&plot[..KPH10_HEADER_LENGTH]).unwrap();
        for point in plot[KPH10_HEADER_LENGTH..].chunks_exact(KPH10_POINT_LENGTH) {
            GlobalPlotPoint::decode(point).unwrap();
        }
        GlobalEvaluationSummary::decode(&encode_ksr10(&capture).unwrap()).unwrap();
        assert_eq!(capture.summary, before);
    }
}
