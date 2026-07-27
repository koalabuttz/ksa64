//! Phase 10 evaluator façade over the deterministic global world and avionics.

use crate::phase10::{GlobalWorldError, EVENT_LANDING};
use crate::phase10_avionics::{
    GlobalAvionicsMission, GlobalAvionicsMissionSummary, GlobalSensorFaults,
};
use ksa64_core::evaluation::{EvaluationOutcome, EvaluationSummary, MetricSlot, ModelProfileId};
use ksa64_core::numeric::{magnitude3_floor, NumericStatus};
use ksa64_core::phase10_contract::{EarthModelPack, TransformPack, PHASE10_CONTRACT_ID};
use ksa64_core::phase10_environment::{ecef_to_geodetic, CompiledAtmospherePack};
use ksa64_core::phase10_telemetry::{
    GlobalEvaluationSummary, GLOBAL_CHECKSUM_COUNT, GLOBAL_TRANSITION_COUNT,
};
use ksa64_core::phase10_vehicle::{GlobalMissionPack, GlobalVehiclePack};
use ksa64_flight::phase10::GlobalFlightConfig;

pub const GLOBAL_TIME_POLICY_ID: u32 = 0x1054_0001;

#[derive(Clone, Copy)]
pub struct GlobalEvaluationRequest<'a> {
    pub earth: &'a EarthModelPack,
    pub transforms: &'a TransformPack,
    pub atmosphere: &'a CompiledAtmospherePack,
    pub vehicle: &'a GlobalVehiclePack,
    pub mission: GlobalMissionPack,
    pub avionics: GlobalFlightConfig,
    pub uncertainty: GlobalSensorFaults,
    pub case_seed: u32,
}

pub fn evaluate_global(
    request: GlobalEvaluationRequest<'_>,
) -> Result<GlobalEvaluationSummary, GlobalWorldError> {
    let runner = GlobalAvionicsMission::new(
        request.earth,
        request.transforms,
        request.atmosphere,
        request.vehicle,
        request.mission,
        request.avionics,
        request.uncertainty,
        request.case_seed,
    )?;
    let result = runner.run()?;
    adapt_global_result(request, result)
}

pub fn adapt_global_result(
    request: GlobalEvaluationRequest<'_>,
    result: GlobalAvionicsMissionSummary,
) -> Result<GlobalEvaluationSummary, GlobalWorldError> {
    let terminal = result.terminal.state;
    let landing = ecef_to_geodetic(result.terminal_ecef.position)?;
    let frame_world = crate::phase10::GlobalWorldMachine::new(
        request.earth,
        request.transforms,
        request.atmosphere,
        request.vehicle,
        request.mission,
    )?;
    let terminal_launch_offset = frame_world.ecef_to_launch_offset_public(result.terminal_ecef)?;
    let downrange_q12 = terminal_launch_offset.x();
    let crossrange_q12 = terminal_launch_offset.y();
    let mut status = NumericStatus::CLEAR;
    let landing_distance = magnitude3_floor(downrange_q12, crossrange_q12, 0, &mut status) as i32;
    if !status.is_clear() {
        return Err(GlobalWorldError::Numeric);
    }
    let outcome = if result.flight.safe {
        EvaluationOutcome::Aborted
    } else if result.terminal.events & EVENT_LANDING != 0 {
        EvaluationOutcome::GroundContact
    } else {
        EvaluationOutcome::RecoveryIncomplete
    };
    let mut common = EvaluationSummary::empty(ModelProfileId::GlobalEcef6DofV1);
    common.outcome = outcome;
    common.steps = result.physical_steps;
    common.terminal_state_a = vector3(terminal.position);
    common.terminal_state_b = vector3(terminal.velocity);
    common.set_metric(MetricSlot::ApogeeAltitude, result.terminal.apogee_q12_km);
    common.set_metric(
        MetricSlot::MaxDynamicPressure,
        result.max_dynamic_pressure_q14,
    );
    common.set_metric(MetricSlot::MaxAcceleration, result.max_acceleration_q28);
    common.set_metric(MetricSlot::MaxMach, result.max_mach_q24);
    common.set_metric(
        MetricSlot::GroundContactTime,
        result.terminal.state.time.raw() as i32,
    );
    common.set_metric(
        MetricSlot::MaxNavigationError,
        result.max_navigation_position_error_q12,
    );
    common.set_metric(MetricSlot::TerminalMass, result.terminal.total_mass_q21_kg);
    common.set_metric(MetricSlot::LandingDistance, landing_distance);
    common.events = u32::from(result.terminal.events);
    common.identities = [
        PHASE10_CONTRACT_ID,
        request.earth.identity,
        request.transforms.identity,
        request.atmosphere.identity,
        request.vehicle.identity,
        request.mission.identity,
    ];
    common.source_checksums = [
        result.terminal.checksum,
        result.sensor_checksum,
        result.flight.navigation.checksum,
        result.command_checksum,
        result.flight.flight_checksum,
    ];
    let mut transition_checksums = [0; GLOBAL_TRANSITION_COUNT];
    let mut transition_position_error = 0;
    let mut transition_velocity_error = 0;
    let mut transition_attitude_error = 0;
    let mut transition_rate_error = 0;
    for (index, record) in result
        .transition_records
        .iter()
        .take(result.transition_count as usize)
        .enumerate()
    {
        transition_checksums[index] = record.checksum;
        transition_position_error = transition_position_error.max(record.position_delta_raw);
        transition_velocity_error = transition_velocity_error.max(record.velocity_delta_raw);
        transition_attitude_error = transition_attitude_error.max(record.attitude_delta_raw);
        transition_rate_error = transition_rate_error.max(record.angular_rate_delta_raw);
    }
    let transition_chain = transition_checksums
        .iter()
        .fold(0x811c_9dc5_u32, |hash, value| hash.rotate_left(5) ^ value);
    let mut global_checksums = [0; GLOBAL_CHECKSUM_COUNT];
    global_checksums.copy_from_slice(&[
        result.terminal.checksum,
        result.sensor_checksum,
        result.flight.navigation.checksum,
        result.command_checksum,
        result.flight.flight_checksum,
        result.placement_checksum,
        request.case_seed,
        transition_chain,
    ]);
    Ok(GlobalEvaluationSummary {
        common,
        terminal_frame: result.terminal.frame,
        terminal_segment: result.terminal.segment,
        transition_count: result.transition_count,
        earth_identity: request.earth.identity,
        transform_identity: request.transforms.identity,
        atmosphere_identity: request.atmosphere.identity,
        terminal_ecef_position_q12: vector3(result.terminal_ecef.position),
        terminal_ecef_velocity_q24: vector3(result.terminal_ecef.velocity),
        terminal_gcrf_position_q12: vector3(result.terminal_gcrf.position),
        terminal_gcrf_velocity_q24: vector3(result.terminal_gcrf.velocity),
        landing_geodetic_q28_q12: [
            landing.latitude_q28_rad,
            landing.longitude_q28_rad,
            landing.height_q12_km,
        ],
        apogee_q12_km: result.terminal.apogee_q12_km,
        downrange_q12_km: downrange_q12,
        crossrange_q12_km: crossrange_q12,
        max_navigation_position_error_q12_km: result.max_navigation_position_error_q12,
        max_navigation_velocity_error_q24_km_s: result.max_navigation_velocity_error_q24,
        max_dynamic_pressure_q14_pa: result.max_dynamic_pressure_q14,
        max_acceleration_q28_km_s2: result.max_acceleration_q28,
        max_mach_q24: result.max_mach_q24,
        terminal_rcs_propellant_q21_kg: result.terminal.rcs_propellant_q21_kg,
        time_identity: time_policy_identity(request.earth),
        transition_position_error_q12_km: transition_position_error,
        transition_velocity_error_q24_km_s: transition_velocity_error,
        transition_attitude_error_q30: transition_attitude_error,
        transition_angular_rate_error_q24: transition_rate_error,
        global_checksums,
        transition_checksums,
    })
}

fn time_policy_identity(earth: &EarthModelPack) -> u32 {
    GLOBAL_TIME_POLICY_ID
        ^ earth.leap_source_hash.rotate_left(7)
        ^ earth.eop_source_hash.rotate_left(13)
        ^ (earth.epoch_unix_day as u32)
}

fn vector3<const FRACTIONAL_BITS: u8>(
    value: ksa64_core::spatial_numeric::FixedVec3<FRACTIONAL_BITS>,
) -> [i32; 3] {
    [value.x(), value.y(), value.z()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase10::GlobalWorldMachine;
    use crate::phase10_avionics::reference_global_flight_config;
    use ksa64_core::phase10_telemetry::{GlobalEvaluationSummary, KSR10_LENGTH};

    #[test]
    fn nominal_global_evaluation_is_strict_and_complete() {
        let earth =
            EarthModelPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kem10"))
                .unwrap();
        let transforms =
            TransformPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kft10"))
                .unwrap();
        let atmosphere = CompiledAtmospherePack::decode(include_bytes!(
            "../../phase10/generated/ksa-g10r.kat10"
        ))
        .unwrap();
        let vehicle =
            GlobalVehiclePack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgv10"))
                .unwrap();
        let mission =
            GlobalMissionPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgm10"))
                .unwrap();
        let world =
            GlobalWorldMachine::new(&earth, &transforms, &atmosphere, &vehicle, mission).unwrap();
        let avionics =
            reference_global_flight_config(0x10a0, world.active_state().unwrap(), mission).unwrap();
        let summary = evaluate_global(GlobalEvaluationRequest {
            earth: &earth,
            transforms: &transforms,
            atmosphere: &atmosphere,
            vehicle: &vehicle,
            mission,
            avionics,
            uncertainty: GlobalSensorFaults::NONE,
            case_seed: 0x4b53_41a0,
        })
        .unwrap();
        assert_eq!(summary.common.outcome, EvaluationOutcome::GroundContact);
        assert_eq!(summary.transition_count, 4);
        assert!(summary.apogee_q12_km >= 200 << 12);
        assert!(summary.downrange_q12_km >= 300 << 12);
        let mut bytes = [0; KSR10_LENGTH];
        summary.encode(&mut bytes).unwrap();
        assert_eq!(GlobalEvaluationSummary::decode(&bytes).unwrap(), summary);
    }
}
