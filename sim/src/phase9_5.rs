//! Phase 9.5 composition helpers binding strict packs to the truth-blind flight allocator.

use ksa64_core::phase8_5_contract::AvionicsProfilePack;
use ksa64_core::phase8_pack::SpatialMotorPack;
use ksa64_core::phase9_5_contract::{
    AdvancedEffectorPack, PriorityResidualAllocatorPack, MAX_CANARDS, MAX_RCS_JETS,
};
use ksa64_flight::phase9_5::AdvancedFlightConfig;
use ksa64_flight::phase9_5_allocator::AdvancedAllocatorConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedCompositionError {
    InvalidEffectorPack,
    InvalidAllocatorPack,
    IdentityMismatch,
    SetMismatch,
    ReserveMismatch,
    FlightConfig,
}

pub fn advanced_flight_config(
    vehicle_identity: u32,
    motor: &SpatialMotorPack,
    avionics: AvionicsProfilePack,
    effectors: &AdvancedEffectorPack,
    allocator: &PriorityResidualAllocatorPack,
) -> Result<AdvancedFlightConfig, AdvancedCompositionError> {
    let capability = crate::phase8_5::reference_monitor_capability(vehicle_identity);
    let mut local = crate::phase8_5::local_flight_config(avionics, capability, motor)
        .map_err(|_| AdvancedCompositionError::FlightConfig)?;
    local.proportional_gain_q15 = allocator.roll_kp_q15.clamp(0, i16::MAX as i32) as i16;
    local.derivative_gain_q15 = allocator.roll_kd_q15.clamp(0, i16::MAX as i32) as i16;
    let mut torque_limit_q12 = [0; 3];
    for (axis, limit) in torque_limit_q12.iter_mut().enumerate() {
        *limit = allocator.group_authority_q12[axis]
            .iter()
            .copied()
            .sum::<i32>()
            .max(1);
    }
    let config = AdvancedFlightConfig {
        local,
        roll_proportional_gain_q15: allocator.roll_kp_q15.clamp(0, i16::MAX as i32) as i16,
        roll_derivative_gain_q15: allocator.roll_kd_q15.clamp(0, i16::MAX as i32) as i16,
        torque_limit_q12,
        fallback_density_upper_q10: 2 << 10,
        maximum_wind_q19: 20 << 19,
        minimum_sound_speed_mps: 250,
        maximum_navigation_speed_mps: 1500,
        propellant_wet_q21: effectors.propellant_wet_mass_q21.max(1),
        reserve_q15: allocator.reserve_q15,
    };
    config
        .is_valid()
        .then_some(config)
        .ok_or(AdvancedCompositionError::FlightConfig)
}

pub fn allocator_config(
    allocator: &PriorityResidualAllocatorPack,
    effectors: &AdvancedEffectorPack,
    gimbal_limit_turn16: [i16; 2],
) -> Result<AdvancedAllocatorConfig, AdvancedCompositionError> {
    if !effectors.is_valid() {
        return Err(AdvancedCompositionError::InvalidEffectorPack);
    }
    if !allocator.is_valid() {
        return Err(AdvancedCompositionError::InvalidAllocatorPack);
    }
    if allocator.effector_identity != effectors.identity {
        return Err(AdvancedCompositionError::IdentityMismatch);
    }
    if allocator.set != effectors.set {
        return Err(AdvancedCompositionError::SetMismatch);
    }
    if effectors.set.has_rcs() && allocator.reserve_q15 != effectors.reserve_q15 {
        return Err(AdvancedCompositionError::ReserveMismatch);
    }
    let has_gimbal = allocator.legacy_gimbal_identity != 0;
    if has_gimbal && gimbal_limit_turn16.iter().any(|v| *v <= 0) {
        return Err(AdvancedCompositionError::InvalidAllocatorPack);
    }
    let mut canard_limits = [1i16; MAX_CANARDS];
    for (target, installed) in canard_limits
        .iter_mut()
        .zip(effectors.canards.iter())
        .take(effectors.canard_count as usize)
    {
        *target = installed.limit_turn16;
    }
    let mut rcs_maximum = [1u8; MAX_RCS_JETS];
    for (target, installed) in rcs_maximum
        .iter_mut()
        .zip(effectors.jets.iter())
        .take(effectors.jet_count as usize)
    {
        *target = installed.max_pulse_quanta;
    }
    let config = AdvancedAllocatorConfig {
        priorities: allocator.priorities,
        canard_enable_q10: allocator.canard_enable_q10,
        canard_full_q10: allocator.canard_full_q10,
        canard_disable_q10: allocator.canard_disable_q10,
        reserve_q15: allocator.reserve_q15,
        propellant_wet_q21: effectors.propellant_wet_mass_q21.max(1),
        group_authority_q12: allocator.group_authority_q12,
        gimbal_mix_q15: allocator.gimbal_mix_q15,
        canard_mix_q15: allocator.canard_mix_q15,
        rcs_mix_q15: allocator.rcs_mix_q15,
        gimbal_limit_turn16: if has_gimbal {
            gimbal_limit_turn16
        } else {
            [1; 2]
        },
        canard_limit_turn16: canard_limits,
        rcs_max_quanta: rcs_maximum,
        has_gimbal,
        has_canards: effectors.set.has_canards(),
        has_rcs: effectors.set.has_rcs(),
    };
    config
        .is_valid()
        .then_some(config)
        .ok_or(AdvancedCompositionError::InvalidAllocatorPack)
}

#[cfg(feature = "fixtures")]
const fn reference_canard(
    position_q28: [i32; 3],
    normal_q15: [i16; 3],
    hinge_axis_q15: [i16; 3],
    failure_identity: u16,
) -> ksa64_core::phase9_5_contract::CanardInstallation {
    ksa64_core::phase9_5_contract::CanardInstallation {
        position_q28,
        normal_q15,
        hinge_axis_q15,
        root_q28: 16_106_127,
        tip_q28: 8_053_064,
        span_q28: 6_710_886,
        sweep_q28: 5_368_709,
        mass_q21: 52_429,
        inertia_q19: [1; 3],
        limit_turn16: 1_820,
        slew_turn16_per_release: 683,
        lag_releases: 1,
        flags: 0,
        failure_identity,
    }
}

#[cfg(feature = "fixtures")]
const fn reference_jet(
    position_q28: [i32; 3],
    direction_q30: [i32; 3],
    index: u16,
) -> ksa64_core::phase9_5_contract::RcsJetInstallation {
    ksa64_core::phase9_5_contract::RcsJetInstallation {
        position_q28,
        direction_q30,
        nominal_thrust_q23: 8_388_608,
        specific_impulse_q16: 3_604_480,
        min_pulse_quanta: 1,
        max_pulse_quanta: 8,
        valve_delay_quanta: 0,
        flags: 0,
        failure_identity: 38_400 + index,
        provenance_identity: 2_623_746_312 + index as u32,
    }
}

#[cfg(feature = "fixtures")]
pub const fn reference_mixed_effectors() -> AdvancedEffectorPack {
    use ksa64_core::phase9_5_contract::{
        AdvancedEffectorSetId, CanardCoefficientKnot, RcsSupplySourceId, SupplyKnot,
        MAX_CANARD_COEFFICIENT_KNOTS, MAX_SUPPLY_KNOTS,
    };
    let mut coefficients = [CanardCoefficientKnot::ZERO; MAX_CANARD_COEFFICIENT_KNOTS];
    coefficients[0] = CanardCoefficientKnot {
        mach_q24: 0,
        control_q24: 40_265_318,
        drag_q24: 838_861,
        hinge_q24: 3_690_988,
    };
    coefficients[1] = CanardCoefficientKnot {
        mach_q24: 6_710_886,
        control_q24: 38_587_597,
        drag_q24: 1_006_633,
        hinge_q24: 4_026_532,
    };
    coefficients[2] = CanardCoefficientKnot {
        mach_q24: 13_421_773,
        control_q24: 35_232_154,
        drag_q24: 1_342_177,
        hinge_q24: 4_697_620,
    };
    let mut supply = [SupplyKnot::ZERO; MAX_SUPPLY_KNOTS];
    supply[0] = SupplyKnot {
        remaining_propellant_q21: 0,
        pressure_q8: 256_000_000,
        thrust_scale_q30: 214_748_365,
        mass_flow_scale_q30: 214_748_365,
    };
    supply[1] = SupplyKnot {
        remaining_propellant_q21: 52_429,
        pressure_q8: 512_000_000,
        thrust_scale_q30: 429_496_730,
        mass_flow_scale_q30: 429_496_730,
    };
    supply[2] = SupplyKnot {
        remaining_propellant_q21: 104_858,
        pressure_q8: 768_000_000,
        thrust_scale_q30: 644_245_094,
        mass_flow_scale_q30: 644_245_094,
    };
    supply[3] = SupplyKnot {
        remaining_propellant_q21: 157_286,
        pressure_q8: 1_024_000_000,
        thrust_scale_q30: 858_993_459,
        mass_flow_scale_q30: 858_993_459,
    };
    supply[4] = SupplyKnot {
        remaining_propellant_q21: 209_715,
        pressure_q8: 1_280_000_000,
        thrust_scale_q30: 1_073_741_824,
        mass_flow_scale_q30: 1_073_741_824,
    };
    AdvancedEffectorPack {
        identity: 258_105_403,
        set: AdvancedEffectorSetId::GimbalCanardRcs,
        supply_source: RcsSupplySourceId::IdealIsothermalBlowdownV1,
        flags: 0,
        vehicle_identity: 1_166_510_380,
        neutral_vehicle_identity: 1_166_510_380,
        supply_identity: 2_737_865_755,
        provenance_identity: 2_623_746_311,
        tank_position_q28: [255_013_683, 0, 0],
        tank_dry_mass_q21: 314_573,
        propellant_wet_mass_q21: 209_715,
        reserve_q15: 6_554,
        canard_hinge_limits_q24: [8_388_608; 4],
        canard_count: 4,
        jet_count: 12,
        coefficient_count: 3,
        supply_count: 5,
        canards: [
            reference_canard(
                [120_795_955, 11_094_169, 0],
                [0, 0, 32_767],
                [0, 32_767, 0],
                38_144,
            ),
            reference_canard(
                [120_795_955, -11_094_169, 0],
                [0, 0, -32_767],
                [0, 32_767, 0],
                38_145,
            ),
            reference_canard(
                [120_795_955, 0, 11_094_169],
                [0, 32_767, 0],
                [0, 0, 32_767],
                38_146,
            ),
            reference_canard(
                [120_795_955, 0, -11_094_169],
                [0, -32_767, 0],
                [0, 0, 32_767],
                38_147,
            ),
        ],
        coefficient_knots: coefficients,
        jets: [
            reference_jet([255_013_683, 7_738_726, 0], [0, 0, 1_073_741_824], 0),
            reference_jet([255_013_683, -7_738_726, 0], [0, 0, -1_073_741_824], 1),
            reference_jet([255_013_683, 7_738_726, 0], [0, 0, -1_073_741_824], 2),
            reference_jet([255_013_683, -7_738_726, 0], [0, 0, 1_073_741_824], 3),
            reference_jet([107_374_182, 0, 0], [0, 0, 1_073_741_824], 4),
            reference_jet([402_653_184, 0, 0], [0, 0, -1_073_741_824], 5),
            reference_jet([107_374_182, 0, 0], [0, 0, -1_073_741_824], 6),
            reference_jet([402_653_184, 0, 0], [0, 0, 1_073_741_824], 7),
            reference_jet([107_374_182, 0, 0], [0, -1_073_741_824, 0], 8),
            reference_jet([402_653_184, 0, 0], [0, 1_073_741_824, 0], 9),
            reference_jet([107_374_182, 0, 0], [0, 1_073_741_824, 0], 10),
            reference_jet([402_653_184, 0, 0], [0, -1_073_741_824, 0], 11),
        ],
        supply_knots: supply,
    }
}

#[cfg(feature = "fixtures")]
pub const fn reference_mixed_vehicle() -> ksa64_core::phase8_pack::SpatialVehiclePack {
    use ksa64_core::phase8_numeric::{SpatialInertia, SpatialMass, SpatialMomentArm};
    let mut vehicle = ksa64_core::phase8_fixtures::FIRESTORM_SPATIAL_VEHICLE;
    vehicle.identity = 1_166_510_380;
    vehicle.dry_mass = SpatialMass::from_raw(5_247_157);
    vehicle.dry_cg_from_nose = SpatialMomentArm::from_raw(239_381_868);
    vehicle.dry_inertia = [
        SpatialInertia::from_raw(806),
        SpatialInertia::from_raw(298_347),
        SpatialInertia::from_raw(298_347),
    ];
    vehicle.source_manifest_identity = 2_623_746_311;
    vehicle
}

#[cfg(feature = "fixtures")]
pub fn reference_mixed_allocator_config() -> AdvancedAllocatorConfig {
    AdvancedAllocatorConfig {
        priorities: [1, 2, 3],
        canard_enable_q10: 307_200,
        canard_full_q10: 2_048_000,
        canard_disable_q10: 204_800,
        reserve_q15: 6_554,
        propellant_wet_q21: 209_715,
        group_authority_q12: [
            [0, 1_638, 2_048],
            [2_048, 2_458, 2_048],
            [2_048, 2_458, 2_048],
        ],
        gimbal_mix_q15: [[0, 0], [32_767, 0], [0, 32_767]],
        canard_mix_q15: [
            [16_384, 16_384, -16_384, -16_384],
            [32_767, -32_767, 0, 0],
            [0, 0, -32_767, 32_767],
        ],
        rcs_mix_q15: [
            [32_767, 32_767, -32_767, -32_767, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 32_767, 32_767, -32_767, -32_767, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 32_767, 32_767, -32_767, -32_767],
        ],
        gimbal_limit_turn16: [910; 2],
        canard_limit_turn16: [1_820; 4],
        rcs_max_quanta: [8; 12],
        has_gimbal: true,
        has_canards: true,
        has_rcs: true,
    }
}

#[cfg(feature = "fixtures")]
pub fn reference_mixed_flight_config() -> Option<AdvancedFlightConfig> {
    let capability = crate::phase8_5::reference_monitor_capability(
        ksa64_core::phase8_fixtures::FIRESTORM_SPATIAL_VEHICLE.identity,
    );
    let mut local = crate::phase8_5::local_flight_config(
        crate::phase8_5::reference_avionics_profile(false),
        capability,
        &ksa64_core::phase8_fixtures::I211W_SPATIAL_MOTOR,
    )
    .ok()?;
    local.proportional_gain_q15 = 14_000;
    local.derivative_gain_q15 = 4_096;
    Some(AdvancedFlightConfig {
        local,
        roll_proportional_gain_q15: 14_000,
        roll_derivative_gain_q15: 4_096,
        torque_limit_q12: [3_686, 6_554, 6_554],
        fallback_density_upper_q10: 2 << 10,
        maximum_wind_q19: 20 << 19,
        minimum_sound_speed_mps: 250,
        maximum_navigation_speed_mps: 1_500,
        propellant_wet_q21: 209_715,
        reserve_q15: 6_554,
    })
}

#[cfg(feature = "fixtures")]
#[allow(dead_code)]
mod allocator_vectors {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase9_5/generated/allocator_vectors_v1.rs"
    ));
}

#[cfg(feature = "fixtures")]
fn fixture_config(group: u8) -> Option<AdvancedAllocatorConfig> {
    use ksa64_core::phase9_5_contract::{parse_allocator_pack, parse_effector_pack};
    let effectors = parse_effector_pack(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase9_5/examples/firestorm-m9.kpe9"
    )))
    .ok()?;
    let pack = parse_allocator_pack(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase9_5/examples/firestorm-m9.kpa9"
    )))
    .ok()?;
    let mut config = allocator_config(&pack, &effectors, [910; 2]).ok()?;
    config.has_gimbal = group == 0 || group == 3;
    config.has_canards = group == 1 || group == 3;
    config.has_rcs = group == 2 || group == 3;
    Some(config)
}

#[cfg(feature = "fixtures")]
fn check_allocator_case(index: u8, signature: &mut u32) -> u32 {
    use allocator_vectors::*;
    use ksa64_flight::phase9_5::AirDataSource;
    use ksa64_flight::phase9_5_allocator::{AllocatorFeedback, PriorityResidualAllocator};
    let config = match fixture_config(index) {
        Some(value) => value,
        None => return u32::MAX,
    };
    let demand = match index {
        0 => [0, 1000, -500],
        1 | 2 => [1000, 1000, -1000],
        3 => [4000, 7000, -6500],
        _ => return u32::MAX,
    };
    let mut allocator = match PriorityResidualAllocator::new(config) {
        Some(value) => value,
        None => return u32::MAX,
    };
    let result = match allocator.allocate(
        demand,
        AllocatorFeedback {
            on_rail: false,
            powered: true,
            recovery: false,
            safe: false,
            air_data_source: AirDataSource::Pitot,
            dynamic_pressure_q10: 2000 << 10,
            propellant_fraction_q15: 32768,
            supply_valid: true,
            gimbal_healthy_mask: 3,
            canard_healthy_mask: 15,
            rcs_healthy_mask: 0x0fff,
        },
    ) {
        Ok(value) => value,
        Err(_) => return u32::MAX,
    };
    let mut failures = 0u32;
    match index {
        0 => {
            failures |= u32::from(result.gimbal != GIMBAL_GIMBAL);
            failures |= u32::from(result.canards != GIMBAL_CANARDS) << 1;
            failures |= u32::from(result.rcs_pulse_quanta != GIMBAL_PULSES) << 2;
            failures |= u32::from(result.achieved_q12 != GIMBAL_ACHIEVED) << 3;
            failures |= u32::from(result.residual_q12 != GIMBAL_RESIDUAL) << 4;
            failures |= u32::from(result.saturation_count != GIMBAL_SATURATION) << 5;
        }
        1 => {
            failures |= u32::from(result.gimbal != CANARD_GIMBAL);
            failures |= u32::from(result.canards != CANARD_CANARDS) << 1;
            failures |= u32::from(result.rcs_pulse_quanta != CANARD_PULSES) << 2;
            failures |= u32::from(result.achieved_q12 != CANARD_ACHIEVED) << 3;
            failures |= u32::from(result.residual_q12 != CANARD_RESIDUAL) << 4;
            failures |= u32::from(result.saturation_count != CANARD_SATURATION) << 5;
        }
        2 => {
            failures |= u32::from(result.gimbal != RCS_GIMBAL);
            failures |= u32::from(result.canards != RCS_CANARDS) << 1;
            failures |= u32::from(result.rcs_pulse_quanta != RCS_PULSES) << 2;
            failures |= u32::from(result.achieved_q12 != RCS_ACHIEVED) << 3;
            failures |= u32::from(result.residual_q12 != RCS_RESIDUAL) << 4;
            failures |= u32::from(result.saturation_count != RCS_SATURATION) << 5;
        }
        3 => {
            failures |= u32::from(result.gimbal != MIXED_GIMBAL);
            failures |= u32::from(result.canards != MIXED_CANARDS) << 1;
            failures |= u32::from(result.rcs_pulse_quanta != MIXED_PULSES) << 2;
            failures |= u32::from(result.achieved_q12 != MIXED_ACHIEVED) << 3;
            failures |= u32::from(result.residual_q12 != MIXED_RESIDUAL) << 4;
            failures |= u32::from(result.saturation_count != MIXED_SATURATION) << 5;
        }
        _ => return u32::MAX,
    }
    for value in result.gimbal {
        *signature = signature.rotate_left(5).wrapping_add(value as u32);
    }
    for value in result.canards {
        *signature = signature.rotate_left(5).wrapping_add(value as u32);
    }
    for value in result.rcs_pulse_quanta {
        *signature = signature.rotate_left(5).wrapping_add(u32::from(value));
    }
    for value in result.achieved_q12 {
        *signature = signature.rotate_left(5).wrapping_add(value as u32);
    }
    for value in result.residual_q12 {
        *signature = signature.rotate_left(5).wrapping_add(value as u32);
    }
    *signature = signature
        .rotate_left(5)
        .wrapping_add(u32::from(result.saturation_count));
    failures
}

#[cfg(feature = "fixtures")]
pub fn run_phase95_allocator_self_tests() -> u32 {
    let mut signature = 0x0950a110u32;
    check_allocator_case(0, &mut signature)
        | check_allocator_case(1, &mut signature)
        | check_allocator_case(2, &mut signature)
        | check_allocator_case(3, &mut signature)
}

#[cfg(feature = "fixtures")]
pub fn phase95_allocator_probe_signature() -> u32 {
    let mut signature = 0x0950a110u32;
    let _ = check_allocator_case(0, &mut signature);
    let _ = check_allocator_case(1, &mut signature);
    let _ = check_allocator_case(2, &mut signature);
    let _ = check_allocator_case(3, &mut signature);
    signature
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase9_5_contract::{parse_allocator_pack, parse_effector_pack};

    const M9_KPE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase9_5/examples/firestorm-m9.kpe9"
    ));
    const M9_KPA: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../phase9_5/examples/firestorm-m9.kpa9"
    ));

    #[cfg(feature = "fixtures")]
    #[test]
    fn allocator_probe_matches_independent_signature() {
        assert_eq!(run_phase95_allocator_self_tests(), 0);
        assert_eq!(
            phase95_allocator_probe_signature(),
            allocator_vectors::ALLOCATOR_SIGNATURE
        );
    }

    #[test]
    fn checked_packs_build_the_exact_mixed_configuration() {
        let e = parse_effector_pack(M9_KPE).unwrap();
        let a = parse_allocator_pack(M9_KPA).unwrap();
        let c = allocator_config(&a, &e, [910; 2]).unwrap();
        assert!(c.has_gimbal && c.has_canards && c.has_rcs);
        assert_eq!(c.canard_limit_turn16, [1820; 4]);
        assert_eq!(c.rcs_max_quanta, [8; 12]);
        assert_eq!(c.propellant_wet_q21, e.propellant_wet_mass_q21);
    }

    #[test]
    fn identity_mismatch_fails_closed() {
        let e = parse_effector_pack(M9_KPE).unwrap();
        let mut a = parse_allocator_pack(M9_KPA).unwrap();
        a.effector_identity ^= 1;
        assert_eq!(
            allocator_config(&a, &e, [910; 2]),
            Err(AdvancedCompositionError::IdentityMismatch)
        );
    }
}

#[cfg(all(test, feature = "fixtures"))]
mod target_fixture_tests {
    use super::*;

    #[test]
    fn compact_target_fixtures_reconstruct_canonical_packs() {
        let effectors = reference_mixed_effectors();
        assert!(effectors.is_valid());
        let mut kpe = [0; ksa64_core::phase9_5_contract::KPE9_LENGTH];
        ksa64_core::phase9_5_contract::write_effector_pack(&effectors, &mut kpe).unwrap();
        assert_eq!(
            &kpe,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../phase9_5/examples/firestorm-m9.kpe9"
            ))
        );
        let vehicle = reference_mixed_vehicle();
        let mut kvp = [0; ksa64_core::phase8_format::KVP8_LENGTH];
        ksa64_core::phase8_pack::encode_spatial_vehicle_pack(&vehicle, &mut kvp).unwrap();
        assert_eq!(
            &kvp,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../phase9_5/examples/firestorm-m9.kvp8"
            ))
        );
        assert!(reference_mixed_allocator_config().is_valid());
        assert!(reference_mixed_flight_config().is_some());
    }
}
