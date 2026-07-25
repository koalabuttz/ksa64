//! Deterministic four-surface canard actuator and incremental aerodynamics.

use crate::numeric::{
    add, divide_scaled, interpolate_clamped, multiply_scaled, subtract, NumericStatus,
};
use crate::phase8_aero::{HOBBY_SPATIAL_MAX_AOA_Q28, HOBBY_SPATIAL_MAX_MACH_Q24};
use crate::phase8_numeric::{BodyTorque, EnuForce};
use crate::phase9_5_contract::{AdvancedEffectorPack, MAX_CANARDS, MAX_CANARD_COEFFICIENT_KNOTS};
use crate::spatial_numeric::{cross_mixed_scaled, FixedVec3};

pub const CANARD_MAX_DYNAMIC_PRESSURE_Q13: i32 = 20_000 << 13;
const TWO_PI_Q28: i32 = 1_686_629_714;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CanardFaultMode {
    Healthy = 0,
    JamAtCurrent = 1,
    FailNeutral = 2,
    HardoverPositive = 3,
    HardoverNegative = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanardError {
    InvalidPack,
    ModelEnvelopeExceeded,
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanardActuatorState {
    pub applied_turn16: [i16; MAX_CANARDS],
    queue: [[i16; MAX_CANARDS]; 8],
    cursor: u8,
}
impl CanardActuatorState {
    pub const NEUTRAL: Self = Self {
        applied_turn16: [0; MAX_CANARDS],
        queue: [[0; MAX_CANARDS]; 8],
        cursor: 0,
    };

    pub fn release(
        &mut self,
        requested: [i16; MAX_CANARDS],
        pack: &AdvancedEffectorPack,
        faults: [CanardFaultMode; MAX_CANARDS],
    ) -> Result<[i16; MAX_CANARDS], CanardError> {
        if !pack.is_valid() || !pack.set.has_canards() {
            return Err(CanardError::InvalidPack);
        }
        let cursor = self.cursor as usize;
        for (index, request) in requested.iter().copied().enumerate() {
            let limit = pack.canards[index].limit_turn16;
            self.queue[cursor][index] = request.clamp(-limit, limit);
        }
        for (index, fault) in faults.iter().copied().enumerate() {
            if fault == CanardFaultMode::JamAtCurrent {
                continue;
            }
            let lag = pack.canards[index].lag_releases as usize;
            let delayed = (cursor + 8 - lag) & 7;
            let limit = pack.canards[index].limit_turn16;
            let target = match fault {
                CanardFaultMode::Healthy => self.queue[delayed][index],
                CanardFaultMode::JamAtCurrent => self.applied_turn16[index],
                CanardFaultMode::FailNeutral => 0,
                CanardFaultMode::HardoverPositive => limit,
                CanardFaultMode::HardoverNegative => -limit,
            };
            let slew = pack.canards[index].slew_turn16_per_release;
            let delta = i32::from(target) - i32::from(self.applied_turn16[index]);
            let bounded = delta.clamp(-i32::from(slew), i32::from(slew));
            self.applied_turn16[index] = (i32::from(self.applied_turn16[index]) + bounded) as i16;
        }
        self.cursor = ((cursor + 1) & 7) as u8;
        Ok(self.applied_turn16)
    }

    pub fn accept_load_limits(&mut self, effective_turn16: [i16; MAX_CANARDS]) {
        self.applied_turn16 = effective_turn16;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanardEvaluationInput {
    pub mach_q24: i32,
    pub dynamic_pressure_q13: i32,
    pub vehicle_angle_of_attack_q28: i32,
    pub cg_from_nose_q28: i32,
    pub deflection_turn16: [i16; MAX_CANARDS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanardSurfaceResult {
    pub force_body_q13: [i32; 3],
    pub torque_body_q12: [i32; 3],
    pub hinge_moment_q24: i32,
    pub effective_turn16: i16,
}
impl CanardSurfaceResult {
    pub const ZERO: Self = Self {
        force_body_q13: [0; 3],
        torque_body_q12: [0; 3],
        hinge_moment_q24: 0,
        effective_turn16: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanardEvaluation {
    pub force_body: EnuForce,
    pub torque_body: BodyTorque,
    pub induced_drag_q13: i32,
    pub load_limited_mask: u8,
    pub surfaces: [CanardSurfaceResult; MAX_CANARDS],
}

fn abs_i32(value: i32) -> u32 {
    value.unsigned_abs()
}
fn angle_q28(turn16: i16, status: &mut NumericStatus) -> i32 {
    multiply_scaled(i32::from(turn16), TWO_PI_Q28, 16, status)
}
fn sample_coefficients(
    pack: &AdvancedEffectorPack,
    mach_q24: i32,
    status: &mut NumericStatus,
) -> (i32, i32, i32) {
    let mut mach = [0; MAX_CANARD_COEFFICIENT_KNOTS];
    let mut control = [0; MAX_CANARD_COEFFICIENT_KNOTS];
    let mut drag = [0; MAX_CANARD_COEFFICIENT_KNOTS];
    let mut hinge = [0; MAX_CANARD_COEFFICIENT_KNOTS];
    let count = pack.coefficient_count as usize;
    let mut index = 0;
    while index < count {
        let knot = pack.coefficient_knots[index];
        mach[index] = knot.mach_q24;
        control[index] = knot.control_q24;
        drag[index] = knot.drag_q24;
        hinge[index] = knot.hinge_q24;
        index += 1;
    }
    (
        interpolate_clamped(mach_q24, &mach[..count], &control[..count], status),
        interpolate_clamped(mach_q24, &mach[..count], &drag[..count], status),
        interpolate_clamped(mach_q24, &mach[..count], &hinge[..count], status),
    )
}

fn evaluate_surface(
    pack: &AdvancedEffectorPack,
    index: usize,
    input: CanardEvaluationInput,
    control_q24: i32,
    drag_q24: i32,
    hinge_q24: i32,
    status: &mut NumericStatus,
) -> Result<(CanardSurfaceResult, bool), CanardError> {
    let installation = pack.canards[index];
    let requested_turn =
        input.deflection_turn16[index].clamp(-installation.limit_turn16, installation.limit_turn16);
    let requested_angle = angle_q28(requested_turn, status);
    if abs_i32(input.vehicle_angle_of_attack_q28).saturating_add(abs_i32(requested_angle))
        > HOBBY_SPATIAL_MAX_AOA_Q28 as u32
    {
        return Err(CanardError::ModelEnvelopeExceeded);
    }
    let chord_q28 = add(installation.root_q28, installation.tip_q28, status) / 2;
    let area_q28 = multiply_scaled(
        add(installation.root_q28, installation.tip_q28, status),
        installation.span_q28,
        28,
        status,
    ) / 2;
    let q_area_q13 = multiply_scaled(input.dynamic_pressure_q13, area_q28, 28, status);
    let q_area_chord_q24 = multiply_scaled(q_area_q13, chord_q28, 17, status);
    let hinge_per_rad_q24 = multiply_scaled(q_area_chord_q24, hinge_q24, 24, status);
    let requested_hinge_q24 = multiply_scaled(hinge_per_rad_q24, requested_angle, 28, status)
        .unsigned_abs()
        .min(i32::MAX as u32) as i32;
    let limit_q24 = pack.canard_hinge_limits_q24[index];
    let (effective_turn, effective_angle, limited) = if requested_hinge_q24 > limit_q24 {
        let ratio_q30 = divide_scaled(limit_q24, requested_hinge_q24, 30, status).clamp(0, 1 << 30);
        (
            multiply_scaled(i32::from(requested_turn), ratio_q30, 30, status) as i16,
            multiply_scaled(requested_angle, ratio_q30, 30, status),
            true,
        )
    } else {
        (requested_turn, requested_angle, false)
    };
    let normal_per_rad_q13 = multiply_scaled(q_area_q13, control_q24, 24, status);
    let normal_q13 = multiply_scaled(normal_per_rad_q13, effective_angle, 28, status);
    let angle_squared_q28 = multiply_scaled(effective_angle, effective_angle, 28, status);
    let induced_drag_q13 = multiply_scaled(
        multiply_scaled(q_area_q13, drag_q24, 24, status),
        angle_squared_q28,
        28,
        status,
    )
    .max(0);
    let normal = installation.normal_q15;
    let force = FixedVec3::<13>::new(
        -induced_drag_q13,
        multiply_scaled(normal_q13, i32::from(normal[1]), 15, status),
        multiply_scaled(normal_q13, i32::from(normal[2]), 15, status),
    );
    let arm = FixedVec3::<28>::new(
        subtract(installation.position_q28[0], input.cg_from_nose_q28, status),
        installation.position_q28[1],
        installation.position_q28[2],
    );
    let torque = cross_mixed_scaled::<28, 13, 12>(arm, force, status);
    let hinge = multiply_scaled(hinge_per_rad_q24, effective_angle, 28, status)
        .unsigned_abs()
        .min(i32::MAX as u32) as i32;
    Ok((
        CanardSurfaceResult {
            force_body_q13: [force.x(), force.y(), force.z()],
            torque_body_q12: [torque.x(), torque.y(), torque.z()],
            hinge_moment_q24: hinge,
            effective_turn16: effective_turn,
        },
        limited,
    ))
}

pub fn evaluate_canards(
    pack: &AdvancedEffectorPack,
    input: CanardEvaluationInput,
    status: &mut NumericStatus,
) -> Result<CanardEvaluation, CanardError> {
    if !pack.is_valid() || !pack.set.has_canards() {
        return Err(CanardError::InvalidPack);
    }
    if !(0..=HOBBY_SPATIAL_MAX_MACH_Q24).contains(&input.mach_q24)
        || !(0..=CANARD_MAX_DYNAMIC_PRESSURE_Q13).contains(&input.dynamic_pressure_q13)
        || abs_i32(input.vehicle_angle_of_attack_q28) > HOBBY_SPATIAL_MAX_AOA_Q28 as u32
    {
        return Err(CanardError::ModelEnvelopeExceeded);
    }
    let (control, drag, hinge) = sample_coefficients(pack, input.mach_q24, status);
    let mut surfaces = [CanardSurfaceResult::ZERO; MAX_CANARDS];
    let mut force = EnuForce::ZERO;
    let mut torque = BodyTorque::ZERO;
    let mut induced_drag = 0;
    let mut mask = 0u8;
    for (index, slot) in surfaces.iter_mut().enumerate() {
        let (surface, limited) =
            evaluate_surface(pack, index, input, control, drag, hinge, status)?;
        *slot = surface;
        force = force.checked_add(
            EnuForce::new(
                surface.force_body_q13[0],
                surface.force_body_q13[1],
                surface.force_body_q13[2],
            ),
            status,
        );
        torque = torque.checked_add(
            BodyTorque::new(
                surface.torque_body_q12[0],
                surface.torque_body_q12[1],
                surface.torque_body_q12[2],
            ),
            status,
        );
        induced_drag = add(induced_drag, -surface.force_body_q13[0], status);
        if limited {
            mask |= 1 << index;
        }
    }
    if !status.is_clear() {
        return Err(CanardError::Numeric);
    }
    Ok(CanardEvaluation {
        force_body: force,
        torque_body: torque,
        induced_drag_q13: induced_drag,
        load_limited_mask: mask,
        surfaces,
    })
}

#[cfg(feature = "fixtures")]
#[allow(dead_code)]
mod independent_vectors {
    include!("../../phase9_5/generated/canard_vectors_v1.rs");
}

#[cfg(feature = "fixtures")]
struct CanardFixture {
    turns: [i16; 4],
    pressure_pa: i32,
    force: [i32; 3],
    torque: [i32; 3],
    hinge: [i32; 4],
    effective: [i16; 4],
    mask: u8,
}

#[cfg(feature = "fixtures")]
const PITCH_FIXTURE: CanardFixture = CanardFixture {
    turns: independent_vectors::PITCH_TURN16,
    pressure_pa: 5_000,
    force: independent_vectors::PITCH_FORCE_Q13,
    torque: independent_vectors::PITCH_TORQUE_Q12,
    hinge: independent_vectors::PITCH_HINGE_Q24,
    effective: independent_vectors::PITCH_EFFECTIVE_TURN16,
    mask: independent_vectors::PITCH_MASK,
};

#[cfg(feature = "fixtures")]
const ROLL_FIXTURE: CanardFixture = CanardFixture {
    turns: independent_vectors::ROLL_TURN16,
    pressure_pa: 5_000,
    force: independent_vectors::ROLL_FORCE_Q13,
    torque: independent_vectors::ROLL_TORQUE_Q12,
    hinge: independent_vectors::ROLL_HINGE_Q24,
    effective: independent_vectors::ROLL_EFFECTIVE_TURN16,
    mask: independent_vectors::ROLL_MASK,
};

#[cfg(feature = "fixtures")]
const LOAD_FIXTURE: CanardFixture = CanardFixture {
    turns: independent_vectors::LOAD_TURN16,
    pressure_pa: 20_000,
    force: independent_vectors::LOAD_FORCE_Q13,
    torque: independent_vectors::LOAD_TORQUE_Q12,
    hinge: independent_vectors::LOAD_HINGE_Q24,
    effective: independent_vectors::LOAD_EFFECTIVE_TURN16,
    mask: independent_vectors::LOAD_MASK,
};

#[cfg(feature = "fixtures")]
fn fixture_matches(pack: &AdvancedEffectorPack, fixture: &CanardFixture) -> bool {
    let mut status = NumericStatus::CLEAR;
    match evaluate_canards(
        pack,
        CanardEvaluationInput {
            mach_q24: 1 << 23,
            dynamic_pressure_q13: fixture.pressure_pa << 13,
            vehicle_angle_of_attack_q28: 0,
            cg_from_nose_q28: 250_000_000,
            deflection_turn16: fixture.turns,
        },
        &mut status,
    ) {
        Ok(result) => {
            [
                result.force_body.x(),
                result.force_body.y(),
                result.force_body.z(),
            ] == fixture.force
                && [
                    result.torque_body.x(),
                    result.torque_body.y(),
                    result.torque_body.z(),
                ] == fixture.torque
                && result.surfaces.map(|value| value.hinge_moment_q24) == fixture.hinge
                && result.surfaces.map(|value| value.effective_turn16) == fixture.effective
                && result.load_limited_mask == fixture.mask
        }
        Err(_) => false,
    }
}

#[cfg(feature = "fixtures")]
pub fn run_phase95_canard_case(index: u8) -> u32 {
    let mut pack = match crate::phase9_5_contract::parse_effector_pack(include_bytes!(
        "../../phase9_5/examples/firestorm-c9.kpe9"
    )) {
        Ok(value) => value,
        Err(_) => return u32::MAX,
    };
    let fixture = match index {
        0 => &PITCH_FIXTURE,
        1 => &ROLL_FIXTURE,
        2 => {
            pack.canard_hinge_limits_q24 = [1 << 12; 4];
            &LOAD_FIXTURE
        }
        _ => return u32::MAX,
    };
    u32::from(!fixture_matches(&pack, fixture))
}

#[cfg(feature = "fixtures")]
pub fn run_phase95_canard_self_tests() -> u32 {
    let mut failures = 0u32;
    for index in 0..3 {
        failures = failures.saturating_add(run_phase95_canard_case(index));
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(dead_code)]
    mod independent {
        include!("../../phase9_5/generated/canard_vectors_v1.rs");
    }
    use crate::phase9_5_contract::parse_effector_pack;
    fn pack() -> AdvancedEffectorPack {
        parse_effector_pack(include_bytes!("../../phase9_5/examples/firestorm-c9.kpe9")).unwrap()
    }

    fn assert_vector(
        mut pack: AdvancedEffectorPack,
        turns: [i16; 4],
        pressure_pa: i32,
        hinge_limits: Option<[i32; 4]>,
        expected_force: [i32; 3],
        expected_torque: [i32; 3],
        expected_hinge: [i32; 4],
        expected_effective: [i16; 4],
        expected_mask: u8,
    ) {
        if let Some(limits) = hinge_limits {
            pack.canard_hinge_limits_q24 = limits;
        }
        let mut status = NumericStatus::CLEAR;
        let result = evaluate_canards(
            &pack,
            CanardEvaluationInput {
                mach_q24: 1 << 23,
                dynamic_pressure_q13: pressure_pa << 13,
                vehicle_angle_of_attack_q28: 0,
                cg_from_nose_q28: 250_000_000,
                deflection_turn16: turns,
            },
            &mut status,
        )
        .unwrap();
        assert_eq!(
            [
                result.force_body.x(),
                result.force_body.y(),
                result.force_body.z()
            ],
            expected_force
        );
        assert_eq!(
            [
                result.torque_body.x(),
                result.torque_body.y(),
                result.torque_body.z()
            ],
            expected_torque
        );
        assert_eq!(
            result.surfaces.map(|surface| surface.hinge_moment_q24),
            expected_hinge
        );
        assert_eq!(
            result.surfaces.map(|surface| surface.effective_turn16),
            expected_effective
        );
        assert_eq!(result.load_limited_mask, expected_mask);
    }
    #[test]
    fn exact_results_match_independent_vectors() {
        let pack = pack();
        assert_vector(
            pack,
            independent::PITCH_TURN16,
            5_000,
            None,
            independent::PITCH_FORCE_Q13,
            independent::PITCH_TORQUE_Q12,
            independent::PITCH_HINGE_Q24,
            independent::PITCH_EFFECTIVE_TURN16,
            independent::PITCH_MASK,
        );
        assert_vector(
            pack,
            independent::ROLL_TURN16,
            5_000,
            None,
            independent::ROLL_FORCE_Q13,
            independent::ROLL_TORQUE_Q12,
            independent::ROLL_HINGE_Q24,
            independent::ROLL_EFFECTIVE_TURN16,
            independent::ROLL_MASK,
        );
        assert_vector(
            pack,
            independent::LOAD_TURN16,
            20_000,
            Some([1 << 12; 4]),
            independent::LOAD_FORCE_Q13,
            independent::LOAD_TORQUE_Q12,
            independent::LOAD_HINGE_Q24,
            independent::LOAD_EFFECTIVE_TURN16,
            independent::LOAD_MASK,
        );
    }

    #[test]
    fn zero_deflection_is_exactly_neutral() {
        let mut status = NumericStatus::CLEAR;
        let result = evaluate_canards(
            &pack(),
            CanardEvaluationInput {
                mach_q24: 1 << 23,
                dynamic_pressure_q13: 2_000 << 13,
                vehicle_angle_of_attack_q28: 0,
                cg_from_nose_q28: 250_000_000,
                deflection_turn16: [0; 4],
            },
            &mut status,
        )
        .unwrap();
        assert_eq!(result.force_body, EnuForce::ZERO);
        assert_eq!(result.torque_body, BodyTorque::ZERO);
        assert_eq!(result.induced_drag_q13, 0);
    }
    #[test]
    fn actuator_lag_slew_and_faults_are_bounded() {
        let pack = pack();
        let mut actuator = CanardActuatorState::NEUTRAL;
        let command = [pack.canards[0].limit_turn16; 4];
        assert_eq!(
            actuator
                .release(command, &pack, [CanardFaultMode::Healthy; 4])
                .unwrap(),
            [0; 4]
        );
        let second = actuator
            .release(
                command,
                &pack,
                [
                    CanardFaultMode::Healthy,
                    CanardFaultMode::JamAtCurrent,
                    CanardFaultMode::FailNeutral,
                    CanardFaultMode::HardoverNegative,
                ],
            )
            .unwrap();
        assert_eq!(second[0], pack.canards[0].slew_turn16_per_release);
        assert_eq!(second[1], 0);
        assert_eq!(second[2], 0);
        assert_eq!(second[3], -pack.canards[3].slew_turn16_per_release);
    }
    #[test]
    fn symmetry_and_envelopes_fail_closed() {
        let pack = pack();
        let mut status = NumericStatus::CLEAR;
        let pitch = evaluate_canards(
            &pack,
            CanardEvaluationInput {
                mach_q24: 1 << 23,
                dynamic_pressure_q13: 2_000 << 13,
                vehicle_angle_of_attack_q28: 0,
                cg_from_nose_q28: 250_000_000,
                deflection_turn16: [910, -910, 0, 0],
            },
            &mut status,
        )
        .unwrap();
        assert_eq!(pitch.torque_body.x(), 0);
        assert_eq!(pitch.torque_body.z(), 0);
        assert_ne!(pitch.torque_body.y(), 0);
        let mut envelope_status = NumericStatus::CLEAR;
        assert_eq!(
            evaluate_canards(
                &pack,
                CanardEvaluationInput {
                    vehicle_angle_of_attack_q28: HOBBY_SPATIAL_MAX_AOA_Q28,
                    mach_q24: 0,
                    dynamic_pressure_q13: 0,
                    cg_from_nose_q28: 0,
                    deflection_turn16: [1, 0, 0, 0]
                },
                &mut envelope_status
            ),
            Err(CanardError::ModelEnvelopeExceeded)
        );
    }
    #[test]
    fn load_limit_reduces_effective_deflection() {
        let mut pack = pack();
        pack.canard_hinge_limits_q24 = [1 << 12; 4];
        let mut status = NumericStatus::CLEAR;
        let result = evaluate_canards(
            &pack,
            CanardEvaluationInput {
                mach_q24: 1 << 23,
                dynamic_pressure_q13: 20_000 << 13,
                vehicle_angle_of_attack_q28: 0,
                cg_from_nose_q28: 250_000_000,
                deflection_turn16: [1820; 4],
            },
            &mut status,
        )
        .unwrap();
        assert_eq!(result.load_limited_mask, 15);
        assert!(result
            .surfaces
            .iter()
            .all(|s| s.effective_turn16.abs() < 1820 && s.hinge_moment_q24 <= 1 << 12));
    }
}
