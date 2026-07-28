//! Estimate-bound Phase 11 host prediction products.
//!
//! These predictors consume transported onboard navigation or the independent
//! ground estimate. They never receive world truth and never co-own the
//! authoritative trajectory.

use ksa64_core::numeric::{magnitude4_floor, NumericStatus};
use ksa64_core::phase10_contract::{EarthModelPack, WGS84_SEMI_MAJOR_Q12_KM};
use ksa64_core::phase10_environment::central_j2_gravity;
use ksa64_core::phase10_numeric::GlobalPositionVec;
use ksa64_flight::phase10::GlobalNavigation;
use ksa64_interface::phase11::{
    GroundEstimate, MissionPlan, PredictionPathHeader, PredictionPathPoint, PredictionProductKind,
    PredictionSummary, PredictionTerminalReason,
};

pub const HOST_PREDICTION_MODEL_ID: u32 = 0x11d0_0002;
pub const ONBOARD_ESTIMATE_SOURCE_ID: u32 = 0x11e1_0001;
pub const DEFAULT_CADENCE_RELEASES: u16 = 32;
pub const DEFAULT_POINT_CAPACITY: u16 = 600;
pub const MAX_POINT_CAPACITY: u16 = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPredictionError {
    Configuration,
    Identity,
    Environment,
    Numeric,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPrediction {
    pub summary: PredictionSummary,
    pub header: PredictionPathHeader,
    pub points: Vec<PredictionPathPoint>,
}

#[derive(Clone, Copy)]
struct EstimateState {
    product: PredictionProductKind,
    identity: u32,
    checksum: u32,
    source_epoch: u32,
    frame: ksa64_interface::phase10::GlobalFrameId,
    position_q12: [i32; 3],
    velocity_q24: [i32; 3],
}

pub fn project_onboard_estimate(
    navigation: GlobalNavigation,
    source_epoch: u32,
    generation_epoch: u32,
    plan: &MissionPlan,
    earth: &EarthModelPack,
) -> Result<HostPrediction, HostPredictionError> {
    project(
        EstimateState {
            product: PredictionProductKind::OnboardEstimateGroundPropagated,
            identity: ONBOARD_ESTIMATE_SOURCE_ID,
            checksum: navigation.checksum,
            source_epoch,
            frame: navigation.frame,
            position_q12: navigation.position_q12,
            velocity_q24: navigation.velocity_q24,
        },
        generation_epoch,
        plan,
        earth,
        DEFAULT_CADENCE_RELEASES,
        DEFAULT_POINT_CAPACITY,
    )
}

pub fn project_ground_estimate(
    estimate: GroundEstimate,
    generation_epoch: u32,
    plan: &MissionPlan,
    earth: &EarthModelPack,
) -> Result<HostPrediction, HostPredictionError> {
    project(
        EstimateState {
            product: PredictionProductKind::GroundEstimate,
            identity: estimate.estimate_identity,
            checksum: estimate.estimator_checksum,
            source_epoch: estimate.production_epoch,
            frame: estimate.frame,
            position_q12: estimate.position_q12_km,
            velocity_q24: estimate.velocity_q24_km_s,
        },
        generation_epoch,
        plan,
        earth,
        DEFAULT_CADENCE_RELEASES,
        DEFAULT_POINT_CAPACITY,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn project_bounded_estimate(
    product: PredictionProductKind,
    source_identity: u32,
    source_checksum: u32,
    source_epoch: u32,
    generation_epoch: u32,
    frame: ksa64_interface::phase10::GlobalFrameId,
    position_q12: [i32; 3],
    velocity_q24: [i32; 3],
    plan: &MissionPlan,
    earth: &EarthModelPack,
    cadence_releases: u16,
    point_capacity: u16,
) -> Result<HostPrediction, HostPredictionError> {
    if matches!(
        product,
        PredictionProductKind::OnboardCompact | PredictionProductKind::SimTruthCounterfactual
    ) {
        return Err(HostPredictionError::Configuration);
    }
    project(
        EstimateState {
            product,
            identity: source_identity,
            checksum: source_checksum,
            source_epoch,
            frame,
            position_q12,
            velocity_q24,
        },
        generation_epoch,
        plan,
        earth,
        cadence_releases,
        point_capacity,
    )
}

fn project(
    source: EstimateState,
    generation_epoch: u32,
    plan: &MissionPlan,
    earth: &EarthModelPack,
    cadence_releases: u16,
    point_capacity: u16,
) -> Result<HostPrediction, HostPredictionError> {
    if source.identity == 0
        || source.checksum == 0
        || generation_epoch < source.source_epoch
        || cadence_releases == 0
        || !(2..=MAX_POINT_CAPACITY).contains(&point_capacity)
    {
        return Err(HostPredictionError::Configuration);
    }
    if plan.ground_prediction_model != HOST_PREDICTION_MODEL_ID
        || plan.package_manifest_identity == 0
        || plan.plan_identity == 0
    {
        return Err(HostPredictionError::Identity);
    }
    earth
        .validate()
        .map_err(|_| HostPredictionError::Identity)?;

    let mut position = source.position_q12;
    let mut velocity = source.velocity_q24;
    let initial_position = position;
    let mut points = Vec::with_capacity(usize::from(point_capacity));
    let mut maximum_altitude = i32::MIN;
    let mut minimum_altitude = i32::MAX;
    let mut maximum_altitude_epoch = source.source_epoch;
    let mut terminal_reason = PredictionTerminalReason::ValidHorizon;

    for index in 0..point_capacity {
        let epoch = source
            .source_epoch
            .saturating_add(u32::from(index) * u32::from(cadence_releases));
        let altitude = altitude_q12(source.frame, position)?;
        if altitude > maximum_altitude {
            maximum_altitude = altitude;
            maximum_altitude_epoch = epoch;
        }
        minimum_altitude = minimum_altitude.min(altitude);
        let horizontal = downrange_q12(source.frame, initial_position, position)?;
        let is_terminal =
            index > 0 && altitude <= 0 && radial_velocity(source.frame, position, velocity) < 0;
        points.push(PredictionPathPoint {
            epoch,
            frame: source.frame,
            flags: u8::from(index == 0) | (u8::from(is_terminal) << 1),
            position_q12_km: position,
            altitude_q12_km: altitude,
            downrange_q12_km: horizontal,
            crossrange_q12_km: 0,
        });
        if is_terminal {
            terminal_reason = PredictionTerminalReason::AtmosphericImpact;
            break;
        }
        let next = midpoint_step(earth, source.frame, position, velocity, cadence_releases)?;
        position = next.0;
        velocity = next.1;
    }

    let path_checksum = hash_points(source.checksum ^ plan.plan_identity, &points);
    let path_identity = hash_words(&[
        HOST_PREDICTION_MODEL_ID,
        source.identity,
        source.checksum,
        source.source_epoch,
        generation_epoch,
        path_checksum,
    ]);
    let final_point = points.last().copied().ok_or(HostPredictionError::Numeric)?;
    let time_to_apogee_q16 = maximum_altitude_epoch
        .saturating_sub(source.source_epoch)
        .saturating_mul(2_048);
    let time_to_impact_q16 = if terminal_reason == PredictionTerminalReason::AtmosphericImpact {
        final_point
            .epoch
            .saturating_sub(source.source_epoch)
            .saturating_mul(2_048)
    } else {
        0
    };
    let summary = PredictionSummary {
        prediction_identity: path_identity,
        model_identity: HOST_PREDICTION_MODEL_ID,
        product: source.product,
        source_estimate_identity: source.identity,
        source_estimate_checksum: source.checksum,
        package_manifest_identity: plan.package_manifest_identity,
        plan_identity: plan.plan_identity,
        source_epoch: source.source_epoch,
        generation_epoch,
        valid_until_epoch: final_point.epoch.max(generation_epoch),
        frame: source.frame,
        terminal_reason,
        apogee_q12_km: maximum_altitude,
        perigee_q12_km: minimum_altitude,
        time_to_apogee_q16,
        time_to_impact_q16,
        impact_position_q12_km: final_point.position_q12_km,
        transition_epochs: [u32::MAX; 3],
        assumptions: 0b11,
        prediction_checksum: path_checksum,
    };
    let header = PredictionPathHeader {
        path_identity,
        model_identity: HOST_PREDICTION_MODEL_ID,
        product: source.product,
        source_estimate_identity: source.identity,
        source_estimate_checksum: source.checksum,
        package_manifest_identity: plan.package_manifest_identity,
        plan_identity: plan.plan_identity,
        source_epoch: source.source_epoch,
        generation_epoch,
        point_count: points.len() as u16,
        cadence_releases,
        terminal_reason,
        path_checksum,
    };
    Ok(HostPrediction {
        summary,
        header,
        points,
    })
}

fn midpoint_step(
    earth: &EarthModelPack,
    frame: ksa64_interface::phase10::GlobalFrameId,
    position: [i32; 3],
    velocity: [i32; 3],
    cadence_releases: u16,
) -> Result<([i32; 3], [i32; 3]), HostPredictionError> {
    let acceleration = acceleration_for_frame(earth, frame, position)?;
    let mut midpoint_position = [0; 3];
    let mut midpoint_velocity = [0; 3];
    for axis in 0..3 {
        midpoint_position[axis] = add_scaled(
            position[axis],
            velocity[axis],
            i64::from(cadence_releases),
            262_144,
        )?;
        midpoint_velocity[axis] = add_scaled(
            velocity[axis],
            acceleration[axis],
            i64::from(cadence_releases),
            1_024,
        )?;
    }
    let midpoint_acceleration = acceleration_for_frame(earth, frame, midpoint_position)?;
    let mut next_position = [0; 3];
    let mut next_velocity = [0; 3];
    for axis in 0..3 {
        next_position[axis] = add_scaled(
            position[axis],
            midpoint_velocity[axis],
            i64::from(cadence_releases),
            131_072,
        )?;
        next_velocity[axis] = add_scaled(
            velocity[axis],
            midpoint_acceleration[axis],
            i64::from(cadence_releases),
            512,
        )?;
    }
    Ok((next_position, next_velocity))
}

fn acceleration_for_frame(
    earth: &EarthModelPack,
    frame: ksa64_interface::phase10::GlobalFrameId,
    position: [i32; 3],
) -> Result<[i32; 3], HostPredictionError> {
    if frame == ksa64_interface::phase10::GlobalFrameId::LocalEnuV1 {
        return Ok([0, 0, -2_632_453]);
    }
    let acceleration = central_j2_gravity(
        earth,
        GlobalPositionVec::new(position[0], position[1], position[2]),
    )
    .map_err(|_| HostPredictionError::Environment)?;
    Ok([acceleration.x(), acceleration.y(), acceleration.z()])
}

fn add_scaled(
    base: i32,
    value: i32,
    numerator: i64,
    denominator: i64,
) -> Result<i32, HostPredictionError> {
    let delta = div_round_away(i64::from(value).saturating_mul(numerator), denominator);
    i64::from(base)
        .checked_add(delta)
        .and_then(|result| i32::try_from(result).ok())
        .ok_or(HostPredictionError::Numeric)
}

fn div_round_away(numerator: i64, denominator: i64) -> i64 {
    let half = denominator / 2;
    if numerator >= 0 {
        numerator.saturating_add(half) / denominator
    } else {
        numerator.saturating_sub(half) / denominator
    }
}

fn altitude_q12(
    frame: ksa64_interface::phase10::GlobalFrameId,
    position: [i32; 3],
) -> Result<i32, HostPredictionError> {
    if frame == ksa64_interface::phase10::GlobalFrameId::LocalEnuV1 {
        return Ok(position[2]);
    }
    let mut status = NumericStatus::CLEAR;
    let radius = magnitude4_floor(0, position[0], position[1], position[2], &mut status);
    if !status.is_clear() || radius > i32::MAX as u32 {
        return Err(HostPredictionError::Numeric);
    }
    Ok((radius as i32).saturating_sub(WGS84_SEMI_MAJOR_Q12_KM))
}

fn downrange_q12(
    frame: ksa64_interface::phase10::GlobalFrameId,
    origin: [i32; 3],
    position: [i32; 3],
) -> Result<i32, HostPredictionError> {
    let delta = [
        position[0].saturating_sub(origin[0]),
        position[1].saturating_sub(origin[1]),
        position[2].saturating_sub(origin[2]),
    ];
    let tangent = if frame == ksa64_interface::phase10::GlobalFrameId::LocalEnuV1 {
        [delta[0], delta[1], 0]
    } else {
        let origin_squared: i128 = origin
            .iter()
            .map(|value| i128::from(*value) * i128::from(*value))
            .sum();
        if origin_squared == 0 {
            return Err(HostPredictionError::Numeric);
        }
        let dot: i128 = origin
            .iter()
            .zip(delta.iter())
            .map(|(left, right)| i128::from(*left) * i128::from(*right))
            .sum();
        let scale_q30 = (dot << 30) / origin_squared;
        let mut tangent = [0; 3];
        for axis in 0..3 {
            let radial = (i128::from(origin[axis]) * scale_q30) >> 30;
            tangent[axis] = i32::try_from(i128::from(delta[axis]) - radial)
                .map_err(|_| HostPredictionError::Numeric)?;
        }
        tangent
    };
    let mut status = NumericStatus::CLEAR;
    let magnitude = magnitude4_floor(0, tangent[0], tangent[1], tangent[2], &mut status);
    if !status.is_clear() || magnitude > i32::MAX as u32 {
        Err(HostPredictionError::Numeric)
    } else {
        Ok(magnitude as i32)
    }
}

fn radial_velocity(
    frame: ksa64_interface::phase10::GlobalFrameId,
    position: [i32; 3],
    velocity: [i32; 3],
) -> i64 {
    if frame == ksa64_interface::phase10::GlobalFrameId::LocalEnuV1 {
        return i64::from(velocity[2]);
    }
    position
        .iter()
        .zip(velocity.iter())
        .map(|(position, velocity)| i64::from(*position) * i64::from(*velocity))
        .sum()
}

fn hash_points(seed: u32, points: &[PredictionPathPoint]) -> u32 {
    let mut hash = seed;
    for point in points {
        hash = hash_words(&[
            hash,
            point.epoch,
            point.frame as u32,
            point.position_q12_km[0] as u32,
            point.position_q12_km[1] as u32,
            point.position_q12_km[2] as u32,
            point.altitude_q12_km as u32,
        ]);
    }
    hash
}

fn hash_words(values: &[u32]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for value in values {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    hash.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> (
        EarthModelPack,
        MissionPlan,
        GlobalNavigation,
        GroundEstimate,
    ) {
        let earth = crate::global_fixtures::GlobalFixtureSet::embedded().earth;
        let plan = ksa64_flight::phase11::ksa_g10r_reference_mission_plan();
        let position = [WGS84_SEMI_MAJOR_Q12_KM + 409_600, 0, 0];
        let velocity = [100_000, 120_000_000, 0];
        let navigation = GlobalNavigation {
            frame: ksa64_interface::phase10::GlobalFrameId::EarthInertialEciV1,
            position_q12: position,
            velocity_q24: velocity,
            attitude_q30: [1 << 30, 0, 0, 0],
            covariance_proxy_q16: [1; 3],
            checksum: 0x5511_0011,
        };
        let ground = GroundEstimate {
            estimator_identity: 0x11e0_1001,
            estimate_identity: 0x11e0_2001,
            source_observation_identity: 0x11e0_3001,
            measurement_epoch: 96,
            production_epoch: 100,
            frame: navigation.frame,
            flags: 0,
            position_q12_km: position,
            velocity_q24_km_s: velocity,
            confidence_q16: [1; 3],
            residual_q16: [0; 3],
            estimator_checksum: 0x7711_0011,
        };
        (earth, plan, navigation, ground)
    }

    #[test]
    fn projections_are_repeatable_and_explicitly_source_bound() {
        let (earth, plan, navigation, ground) = fixtures();
        let onboard = project_onboard_estimate(navigation, 100, 104, &plan, &earth).unwrap();
        let repeated = project_onboard_estimate(navigation, 100, 104, &plan, &earth).unwrap();
        let ground_path = project_ground_estimate(ground, 104, &plan, &earth).unwrap();
        assert_eq!(onboard, repeated);
        assert_eq!(
            onboard.header.product,
            PredictionProductKind::OnboardEstimateGroundPropagated
        );
        assert_eq!(onboard.header.source_estimate_checksum, navigation.checksum);
        assert_eq!(
            ground_path.header.product,
            PredictionProductKind::GroundEstimate
        );
        assert_eq!(
            ground_path.header.source_estimate_identity,
            ground.estimate_identity
        );
        assert_ne!(
            onboard.header.path_identity,
            ground_path.header.path_identity
        );
    }

    #[test]
    fn serialized_header_and_points_match_the_derived_path() {
        let (earth, plan, navigation, _) = fixtures();
        let prediction = project_bounded_estimate(
            PredictionProductKind::OnboardEstimateGroundPropagated,
            ONBOARD_ESTIMATE_SOURCE_ID,
            navigation.checksum,
            100,
            104,
            navigation.frame,
            navigation.position_q12,
            navigation.velocity_q24,
            &plan,
            &earth,
            32,
            16,
        )
        .unwrap();
        let mut header = [0; ksa64_interface::phase11::KPP11_HEADER_LENGTH];
        let independent_float64_q12 = [26_530_432, 439_429, 0];
        let final_point = prediction.points.last().unwrap();
        for (actual, expected) in final_point
            .position_q12_km
            .iter()
            .zip(independent_float64_q12.iter())
        {
            assert!((i64::from(*actual) - i64::from(*expected)).unsigned_abs() <= 128);
        }
        assert!((i64::from(final_point.altitude_q12_km) - 409_222).unsigned_abs() <= 128);

        ksa64_interface::phase11::write_kpp11_header(&prediction.header, &mut header).unwrap();
        assert_eq!(
            ksa64_interface::phase11::parse_kpp11_header(&header).unwrap(),
            prediction.header
        );
        for point in prediction.points {
            let mut bytes = [0; ksa64_interface::phase11::KPP11_POINT_LENGTH];
            ksa64_interface::phase11::write_kpp11_point(&point, &mut bytes).unwrap();
            assert_eq!(
                ksa64_interface::phase11::parse_kpp11_point(&bytes).unwrap(),
                point
            );
        }
    }
}
