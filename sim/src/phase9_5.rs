//! Phase 9.5 composition helpers binding strict packs to the truth-blind flight allocator.

use ksa64_core::phase9_5_contract::{
    AdvancedEffectorPack, PriorityResidualAllocatorPack, MAX_CANARDS, MAX_RCS_JETS,
};
use ksa64_flight::phase9_5_allocator::AdvancedAllocatorConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedCompositionError {
    InvalidEffectorPack,
    InvalidAllocatorPack,
    IdentityMismatch,
    SetMismatch,
    ReserveMismatch,
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
