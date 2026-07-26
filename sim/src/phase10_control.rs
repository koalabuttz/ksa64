//! Phase 10 deterministic gimbal and cold-gas RCS actuation.

use ksa64_core::numeric::{multiply_scaled, NumericStatus};
use ksa64_core::phase10_attitude::GlobalBodyTorque;
use ksa64_core::phase10_vehicle::GlobalVehiclePack;
use ksa64_core::spatial_numeric::FixedVec3;
use ksa64_interface::phase10::{GlobalCommandCell, GLOBAL_COMMAND_SAFE};

pub const RCS_QUANTUM_Q16: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalActuatorState {
    pub gimbal_turn16: [i16; 2],
    pub pulse_remaining_q16: [u16; 12],
    last_source_epoch: u16,
    has_source_epoch: bool,
}

impl GlobalActuatorState {
    pub const NEUTRAL: Self = Self {
        gimbal_turn16: [0; 2],
        pulse_remaining_q16: [0; 12],
        last_source_epoch: 0,
        has_source_epoch: false,
    };

    pub fn accept(
        &mut self,
        command: &GlobalCommandCell,
        vehicle: &GlobalVehiclePack,
        powered: bool,
        rail_constrained: bool,
        remaining_propellant_q21: i32,
    ) {
        if self.has_source_epoch && self.last_source_epoch == command.source_epoch {
            return;
        }
        self.has_source_epoch = true;
        self.last_source_epoch = command.source_epoch;
        let safe = command.discrete & GLOBAL_COMMAND_SAFE != 0;
        let target = if powered && !rail_constrained && !safe {
            [
                clamp_turn16(
                    i32::from(command.gimbal_q15[0]) * 2,
                    vehicle.gimbal_limit_turn16,
                ),
                clamp_turn16(
                    i32::from(command.gimbal_q15[1]) * 2,
                    vehicle.gimbal_limit_turn16,
                ),
            ]
        } else {
            [0; 2]
        };
        for (applied, requested) in self.gimbal_turn16.iter_mut().zip(target) {
            let delta = i32::from(requested) - i32::from(*applied);
            let limited = delta.clamp(
                -i32::from(vehicle.gimbal_slew_turn16_per_release),
                i32::from(vehicle.gimbal_slew_turn16_per_release),
            );
            *applied = (i32::from(*applied) + limited) as i16;
        }
        let reserve = ((i64::from(vehicle.rcs_propellant_q21_kg)
            * i64::from(vehicle.rcs_reserve_q16))
            >> 16) as i32;
        if !powered && !rail_constrained && !safe && remaining_propellant_q21 > reserve {
            for (remaining, quanta) in self
                .pulse_remaining_q16
                .iter_mut()
                .zip(command.rcs_pulse_quanta)
            {
                *remaining = u16::from(quanta) * RCS_QUANTUM_Q16 as u16;
            }
        } else {
            self.pulse_remaining_q16 = [0; 12];
        }
    }

    pub fn next_edge_q16(self, maximum_q16: u32) -> u32 {
        self.pulse_remaining_q16
            .iter()
            .copied()
            .filter(|value| *value != 0)
            .map(u32::from)
            .fold(maximum_q16, u32::min)
    }

    pub fn active_mask(self) -> u16 {
        self.pulse_remaining_q16
            .iter()
            .enumerate()
            .fold(0u16, |mask, (index, remaining)| {
                if *remaining == 0 {
                    mask
                } else {
                    mask | (1 << index)
                }
            })
    }

    pub fn advance_pulses(&mut self, duration_q16: u32) {
        for remaining in &mut self.pulse_remaining_q16 {
            *remaining = remaining.saturating_sub(duration_q16 as u16);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalEffectorSample {
    pub force_body_q13: FixedVec3<13>,
    pub torque_body_q12: GlobalBodyTorque,
    pub active_jets: u8,
}

pub fn sample_rcs_effectors(
    state: GlobalActuatorState,
    vehicle: &GlobalVehiclePack,
    cg_from_nose_q28: i32,
    status: &mut NumericStatus,
) -> GlobalEffectorSample {
    let radius_q28 = vehicle.diameter_q13_m << 14;
    let nose_arm_q28 = cg_from_nose_q28;
    let cg_q13 = cg_from_nose_q28 >> 15;
    let tail_arm_q28 = vehicle
        .length_q13_m
        .saturating_sub(cg_q13)
        .saturating_mul(1 << 15);
    let axial_arm_q28 = nose_arm_q28.min(tail_arm_q28).max(1);
    let thrust = vehicle.rcs_nominal_thrust_q13_n;
    let mut force = FixedVec3::<13>::ZERO;
    let mut torque = GlobalBodyTorque::ZERO;
    let mut active = 0u8;
    for jet in 0..12 {
        if state.pulse_remaining_q16[jet] == 0 {
            continue;
        }
        active = active.saturating_add(1);
        let (position_q28, force_q13) = jet_geometry(jet, radius_q28, axial_arm_q28, thrust);
        force = force.checked_add(force_q13, status);
        let cross = GlobalBodyTorque::new(
            multiply_scaled(position_q28[1], force_q13.z(), 29, status)
                - multiply_scaled(position_q28[2], force_q13.y(), 29, status),
            multiply_scaled(position_q28[2], force_q13.x(), 29, status)
                - multiply_scaled(position_q28[0], force_q13.z(), 29, status),
            multiply_scaled(position_q28[0], force_q13.y(), 29, status)
                - multiply_scaled(position_q28[1], force_q13.x(), 29, status),
        );
        torque = torque.checked_add(cross, status);
    }
    GlobalEffectorSample {
        force_body_q13: force,
        torque_body_q12: torque,
        active_jets: active,
    }
}

fn jet_geometry(
    jet: usize,
    radius_q28: i32,
    axial_arm_q28: i32,
    thrust_q13: i32,
) -> ([i32; 3], FixedVec3<13>) {
    match jet {
        0 => ([0, radius_q28, 0], FixedVec3::new(0, 0, thrust_q13)),
        1 => ([0, -radius_q28, 0], FixedVec3::new(0, 0, -thrust_q13)),
        2 => ([0, radius_q28, 0], FixedVec3::new(0, 0, -thrust_q13)),
        3 => ([0, -radius_q28, 0], FixedVec3::new(0, 0, thrust_q13)),
        4 => ([axial_arm_q28, 0, 0], FixedVec3::new(0, 0, -thrust_q13)),
        5 => ([-axial_arm_q28, 0, 0], FixedVec3::new(0, 0, thrust_q13)),
        6 => ([axial_arm_q28, 0, 0], FixedVec3::new(0, 0, thrust_q13)),
        7 => ([-axial_arm_q28, 0, 0], FixedVec3::new(0, 0, -thrust_q13)),
        8 => ([axial_arm_q28, 0, 0], FixedVec3::new(0, thrust_q13, 0)),
        9 => ([-axial_arm_q28, 0, 0], FixedVec3::new(0, -thrust_q13, 0)),
        10 => ([axial_arm_q28, 0, 0], FixedVec3::new(0, -thrust_q13, 0)),
        _ => ([-axial_arm_q28, 0, 0], FixedVec3::new(0, thrust_q13, 0)),
    }
}

pub fn rcs_propellant_consumed_q21(
    vehicle: &GlobalVehiclePack,
    active_jets: u8,
    duration_q16: u32,
) -> Option<i32> {
    if active_jets == 0 {
        return Some(0);
    }
    const G0_Q16: i64 = 642_689;
    let numerator = i64::from(vehicle.rcs_nominal_thrust_q13_n)
        .checked_mul(1i64 << 40)?
        .checked_mul(i64::from(active_jets))?;
    let denominator = i64::from(vehicle.rcs_isp_q16_s).checked_mul(G0_Q16)?;
    let flow_q21_per_second = (numerator + denominator / 2) / denominator;
    let consumed_numerator = flow_q21_per_second.checked_mul(i64::from(duration_q16))?;
    let consumed = (consumed_numerator + (1 << 15)) >> 16;
    i32::try_from(consumed).ok()
}

fn clamp_turn16(value: i32, limit: i16) -> i16 {
    value.clamp(-i32::from(limit), i32::from(limit)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase10_vehicle::GlobalVehiclePack;

    fn vehicle() -> GlobalVehiclePack {
        GlobalVehiclePack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgv10")).unwrap()
    }

    #[test]
    fn balanced_pair_has_torque_without_translation() {
        let pack = vehicle();
        let mut state = GlobalActuatorState::NEUTRAL;
        state.pulse_remaining_q16[4] = 256;
        state.pulse_remaining_q16[5] = 256;
        let mut status = NumericStatus::CLEAR;
        let sample = sample_rcs_effectors(state, &pack, pack.wet_cg_q28_m, &mut status);
        assert!(status.is_clear());
        assert_eq!(sample.force_body_q13, FixedVec3::ZERO);
        assert!(sample.torque_body_q12.y() > 0);
        assert_eq!(sample.active_jets, 2);
    }

    #[test]
    fn pulse_edges_are_exact_quanta() {
        let mut state = GlobalActuatorState::NEUTRAL;
        state.pulse_remaining_q16[0] = 3 * RCS_QUANTUM_Q16 as u16;
        assert_eq!(state.next_edge_q16(2_048), 768);
        state.advance_pulses(512);
        assert_eq!(state.next_edge_q16(2_048), 256);
        state.advance_pulses(256);
        assert_eq!(state.active_mask(), 0);
    }

    #[test]
    fn mass_flow_is_positive_and_bounded() {
        let consumed = rcs_propellant_consumed_q21(&vehicle(), 2, 256).unwrap();
        assert!(consumed > 0);
        assert!(consumed < 1 << 18);
    }
}
