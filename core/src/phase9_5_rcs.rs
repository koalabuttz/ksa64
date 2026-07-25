//! Deterministic twelve-jet cold-gas RCS, supply, pulse, and mass-property model.

use crate::numeric::{
    add, divide_scaled, interpolate_clamped, multiply_scaled, subtract, NumericStatus,
};
use crate::phase9_5_contract::{AdvancedEffectorPack, MAX_RCS_JETS, MAX_SUPPLY_KNOTS};
use crate::phase9_5_numeric::RCS_PULSE_QUANTUM_Q18;
use crate::spatial_numeric::{cross_mixed_scaled, FixedVec3};

pub const AVIONICS_INTERVAL_Q18: i32 = 8_192;
const G0_Q16: i32 = 642_689;
const ONE_Q30: i32 = 1 << 30;
const HALF_Q30: i32 = 1 << 29;
const LEAK_Q30: i32 = 1 << 26;
const NO_EDGE: i32 = i32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RcsJetFault {
    Healthy = 0,
    StuckClosed = 1,
    StuckOpen = 2,
    Leak = 3,
    DegradedThrustHalf = 4,
    DelayedValveOneQuantum = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RcsError {
    InvalidPack,
    InvalidEpoch,
    InvalidCommand,
    DuplicateOrStaleCommand,
    PendingPulse,
    EventSplitRequired(i32),
    Numeric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RcsPulseCommand {
    pub quanta: [u8; MAX_RCS_JETS],
}
impl RcsPulseCommand {
    pub const ZERO: Self = Self {
        quanta: [0; MAX_RCS_JETS],
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupplySample {
    pub pressure_q8: i32,
    pub thrust_scale_q30: i32,
    pub mass_flow_scale_q30: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RcsPropellantMassProperties {
    pub mass_q21: i32,
    pub first_moment_q21: [i32; 3],
    pub inertia_q19: [i32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RcsSegmentResult {
    pub integrated_end_q18: i32,
    pub active_mask: u16,
    pub force_body_q23: [i32; 3],
    pub torque_body_q12: [i32; 3],
    pub individual_force_q23: [[i32; 3]; MAX_RCS_JETS],
    pub individual_thrust_q23: [i32; MAX_RCS_JETS],
    pub total_mass_flow_q28: i32,
    pub total_impulse_q26: i32,
    pub consumed_propellant_q21: i32,
    pub remaining_propellant_q21: i32,
    pub pressure_q8: i32,
    pub thrust_scale_q30: i32,
    pub depleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RcsState {
    pub remaining_propellant_q21: i32,
    accumulator_quanta: [u8; MAX_RCS_JETS],
    open_at_q18: [i32; MAX_RCS_JETS],
    close_at_q18: [i32; MAX_RCS_JETS],
    last_command_epoch_q18: i32,
}
impl RcsState {
    pub fn new(pack: &AdvancedEffectorPack) -> Result<Self, RcsError> {
        if !pack.is_valid() || !pack.set.has_rcs() {
            return Err(RcsError::InvalidPack);
        }
        Ok(Self {
            remaining_propellant_q21: pack.propellant_wet_mass_q21,
            accumulator_quanta: [0; MAX_RCS_JETS],
            open_at_q18: [NO_EDGE; MAX_RCS_JETS],
            close_at_q18: [NO_EDGE; MAX_RCS_JETS],
            last_command_epoch_q18: -AVIONICS_INTERVAL_Q18,
        })
    }

    pub fn accumulated_quanta(&self) -> [u8; MAX_RCS_JETS] {
        self.accumulator_quanta
    }

    pub fn schedule_successor(
        &mut self,
        release_epoch_q18: i32,
        command: RcsPulseCommand,
        pack: &AdvancedEffectorPack,
        faults: [RcsJetFault; MAX_RCS_JETS],
    ) -> Result<(), RcsError> {
        if !pack.is_valid() || !pack.set.has_rcs() {
            return Err(RcsError::InvalidPack);
        }
        if release_epoch_q18 < 0 || release_epoch_q18 % AVIONICS_INTERVAL_Q18 != 0 {
            return Err(RcsError::InvalidEpoch);
        }
        if release_epoch_q18 <= self.last_command_epoch_q18 {
            return Err(RcsError::DuplicateOrStaleCommand);
        }
        for index in 0..MAX_RCS_JETS {
            if command.quanta[index] > 8 {
                return Err(RcsError::InvalidCommand);
            }
            if (self.close_at_q18[index] != NO_EDGE && self.close_at_q18[index] > release_epoch_q18)
                || (self.open_at_q18[index] != NO_EDGE
                    && self.open_at_q18[index] > release_epoch_q18)
            {
                return Err(RcsError::PendingPulse);
            }
        }
        self.last_command_epoch_q18 = release_epoch_q18;
        for (index, fault) in faults.iter().copied().enumerate() {
            self.open_at_q18[index] = NO_EDGE;
            self.close_at_q18[index] = NO_EDGE;
            let jet = pack.jets[index];
            let requested = command.quanta[index];
            if requested == 0 {
                continue;
            }
            let total = self.accumulator_quanta[index].saturating_add(requested);
            if total < jet.min_pulse_quanta {
                self.accumulator_quanta[index] = total;
                continue;
            }
            let emitted = total.min(jet.max_pulse_quanta);
            self.accumulator_quanta[index] = total - emitted;
            if matches!(fault, RcsJetFault::StuckClosed | RcsJetFault::StuckOpen) {
                continue;
            }
            let extra = u8::from(fault == RcsJetFault::DelayedValveOneQuantum);
            let delay = jet.valve_delay_quanta.saturating_add(extra);
            self.open_at_q18[index] =
                release_epoch_q18.saturating_add(i32::from(delay) * RCS_PULSE_QUANTUM_Q18);
            self.close_at_q18[index] =
                self.open_at_q18[index].saturating_add(i32::from(emitted) * RCS_PULSE_QUANTUM_Q18);
        }
        Ok(())
    }

    pub fn safe(&mut self) {
        self.accumulator_quanta = [0; MAX_RCS_JETS];
        self.open_at_q18 = [NO_EDGE; MAX_RCS_JETS];
        self.close_at_q18 = [NO_EDGE; MAX_RCS_JETS];
    }

    pub fn next_valve_edge_after(&self, after_q18: i32, through_q18: i32) -> Option<i32> {
        let mut edge = NO_EDGE;
        for index in 0..MAX_RCS_JETS {
            let open = self.open_at_q18[index];
            let close = self.close_at_q18[index];
            if open > after_q18 && open <= through_q18 {
                edge = edge.min(open);
            }
            if close > after_q18 && close <= through_q18 {
                edge = edge.min(close);
            }
        }
        (edge != NO_EDGE).then_some(edge)
    }

    fn scheduled_open(&self, index: usize, time_q18: i32) -> bool {
        self.open_at_q18[index] <= time_q18 && time_q18 < self.close_at_q18[index]
    }
}

pub fn sample_supply(
    pack: &AdvancedEffectorPack,
    remaining_propellant_q21: i32,
    status: &mut NumericStatus,
) -> Result<SupplySample, RcsError> {
    if !pack.is_valid()
        || !pack.set.has_rcs()
        || !(0..=pack.propellant_wet_mass_q21).contains(&remaining_propellant_q21)
    {
        return Err(RcsError::InvalidPack);
    }
    let count = pack.supply_count as usize;
    let mut remaining = [0; MAX_SUPPLY_KNOTS];
    let mut pressure = [0; MAX_SUPPLY_KNOTS];
    let mut thrust = [0; MAX_SUPPLY_KNOTS];
    let mut flow = [0; MAX_SUPPLY_KNOTS];
    for index in 0..count {
        let knot = pack.supply_knots[index];
        remaining[index] = knot.remaining_propellant_q21;
        pressure[index] = knot.pressure_q8;
        thrust[index] = knot.thrust_scale_q30;
        flow[index] = knot.mass_flow_scale_q30;
    }
    let sample = SupplySample {
        pressure_q8: interpolate_clamped(
            remaining_propellant_q21,
            &remaining[..count],
            &pressure[..count],
            status,
        ),
        thrust_scale_q30: interpolate_clamped(
            remaining_propellant_q21,
            &remaining[..count],
            &thrust[..count],
            status,
        ),
        mass_flow_scale_q30: interpolate_clamped(
            remaining_propellant_q21,
            &remaining[..count],
            &flow[..count],
            status,
        ),
    };
    if status.is_clear() {
        Ok(sample)
    } else {
        Err(RcsError::Numeric)
    }
}

pub fn propellant_mass_properties(
    pack: &AdvancedEffectorPack,
    remaining_propellant_q21: i32,
    status: &mut NumericStatus,
) -> Result<RcsPropellantMassProperties, RcsError> {
    if !pack.is_valid()
        || !pack.set.has_rcs()
        || !(0..=pack.propellant_wet_mass_q21).contains(&remaining_propellant_q21)
    {
        return Err(RcsError::InvalidPack);
    }
    let p = pack.tank_position_q28;
    let mut first = [0; 3];
    for index in 0..3 {
        first[index] = multiply_scaled(remaining_propellant_q21, p[index], 28, status);
    }
    let x2 = multiply_scaled(p[0], p[0], 28, status);
    let y2 = multiply_scaled(p[1], p[1], 28, status);
    let z2 = multiply_scaled(p[2], p[2], 28, status);
    let inertia = [
        multiply_scaled(remaining_propellant_q21, add(y2, z2, status), 30, status),
        multiply_scaled(remaining_propellant_q21, add(x2, z2, status), 30, status),
        multiply_scaled(remaining_propellant_q21, add(x2, y2, status), 30, status),
    ];
    if !status.is_clear() {
        return Err(RcsError::Numeric);
    }
    Ok(RcsPropellantMassProperties {
        mass_q21: remaining_propellant_q21,
        first_moment_q21: first,
        inertia_q19: inertia,
    })
}

fn fault_scale(fault: RcsJetFault, scheduled: bool) -> i32 {
    match fault {
        RcsJetFault::StuckClosed => 0,
        RcsJetFault::StuckOpen => ONE_Q30,
        RcsJetFault::Leak if scheduled => ONE_Q30,
        RcsJetFault::Leak => LEAK_Q30,
        RcsJetFault::DegradedThrustHalf if scheduled => HALF_Q30,
        RcsJetFault::DegradedThrustHalf => 0,
        RcsJetFault::Healthy | RcsJetFault::DelayedValveOneQuantum if scheduled => ONE_Q30,
        RcsJetFault::Healthy | RcsJetFault::DelayedValveOneQuantum => 0,
    }
}

pub fn integrate_rcs_segment(
    state: &mut RcsState,
    pack: &AdvancedEffectorPack,
    start_q18: i32,
    end_q18: i32,
    cg_from_nose_q28: i32,
    faults: [RcsJetFault; MAX_RCS_JETS],
    status: &mut NumericStatus,
) -> Result<RcsSegmentResult, RcsError> {
    if !pack.is_valid() || !pack.set.has_rcs() || start_q18 < 0 || end_q18 <= start_q18 {
        return Err(RcsError::InvalidPack);
    }
    if let Some(edge) = state.next_valve_edge_after(start_q18, end_q18) {
        if edge < end_q18 {
            return Err(RcsError::EventSplitRequired(edge));
        }
    }
    let supply = sample_supply(pack, state.remaining_propellant_q21, status)?;
    let mut individual = [[0; 3]; MAX_RCS_JETS];
    let mut force = FixedVec3::<23>::ZERO;
    let mut torque = FixedVec3::<12>::ZERO;
    let mut total_flow_q28 = 0;
    let mut individual_thrust_q23 = [0; MAX_RCS_JETS];
    let mut active_mask = 0u16;
    for index in 0..MAX_RCS_JETS {
        let scheduled = state.scheduled_open(index, start_q18);
        let failure_scale = fault_scale(faults[index], scheduled);
        if failure_scale == 0 || state.remaining_propellant_q21 == 0 {
            continue;
        }
        active_mask |= 1 << index;
        let jet = pack.jets[index];
        let thrust_q23 = multiply_scaled(
            multiply_scaled(jet.nominal_thrust_q23, supply.thrust_scale_q30, 30, status),
            failure_scale,
            30,
            status,
        );
        let jet_force = FixedVec3::<23>::new(
            multiply_scaled(thrust_q23, jet.direction_q30[0], 30, status),
            multiply_scaled(thrust_q23, jet.direction_q30[1], 30, status),
            multiply_scaled(thrust_q23, jet.direction_q30[2], 30, status),
        );
        individual[index] = [jet_force.x(), jet_force.y(), jet_force.z()];
        force = force.checked_add(jet_force, status);
        individual_thrust_q23[index] = thrust_q23;
        let arm = FixedVec3::<28>::new(
            subtract(jet.position_q28[0], cg_from_nose_q28, status),
            jet.position_q28[1],
            jet.position_q28[2],
        );
        let torque_force = FixedVec3::<13>::new(
            jet_force.x() / 1_024,
            jet_force.y() / 1_024,
            jet_force.z() / 1_024,
        );
        torque = torque.checked_add(
            cross_mixed_scaled::<28, 13, 12>(arm, torque_force, status),
            status,
        );
        let denominator_q16 = multiply_scaled(jet.specific_impulse_q16, G0_Q16, 16, status);
        let base_flow_q28 = divide_scaled(jet.nominal_thrust_q23, denominator_q16, 21, status);
        let flow_q28 = multiply_scaled(
            multiply_scaled(base_flow_q28, supply.mass_flow_scale_q30, 30, status),
            failure_scale,
            30,
            status,
        );
        total_flow_q28 = add(total_flow_q28, flow_q28, status);
    }
    if !status.is_clear() {
        return Err(RcsError::Numeric);
    }
    let requested_dt = end_q18 - start_q18;
    let requested_consumption = multiply_scaled(total_flow_q28, requested_dt, 25, status).max(0);
    let (actual_dt, consumed, depleted) =
        if total_flow_q28 > 0 && requested_consumption >= state.remaining_propellant_q21 {
            let dt = divide_scaled(state.remaining_propellant_q21, total_flow_q28, 25, status)
                .clamp(0, requested_dt);
            (dt, state.remaining_propellant_q21, true)
        } else {
            (requested_dt, requested_consumption, false)
        };
    let mut total_impulse_q26 = 0;
    for thrust_q23 in individual_thrust_q23 {
        total_impulse_q26 = add(
            total_impulse_q26,
            multiply_scaled(thrust_q23, actual_dt, 15, status),
            status,
        );
    }
    state.remaining_propellant_q21 =
        subtract(state.remaining_propellant_q21, consumed, status).max(0);
    if !status.is_clear() {
        return Err(RcsError::Numeric);
    }
    Ok(RcsSegmentResult {
        integrated_end_q18: start_q18 + actual_dt,
        active_mask,
        force_body_q23: [force.x(), force.y(), force.z()],
        torque_body_q12: [torque.x(), torque.y(), torque.z()],
        individual_force_q23: individual,
        individual_thrust_q23,
        total_mass_flow_q28: total_flow_q28,
        total_impulse_q26,
        consumed_propellant_q21: consumed,
        remaining_propellant_q21: state.remaining_propellant_q21,
        pressure_q8: supply.pressure_q8,
        thrust_scale_q30: supply.thrust_scale_q30,
        depleted,
    })
}

#[cfg(any(test, feature = "fixtures"))]
#[allow(dead_code)]
mod independent_vectors {
    include!("../../phase9_5/generated/rcs_vectors_v1.rs");
}

#[cfg(any(test, feature = "fixtures"))]
pub fn run_phase95_rcs_case(index: u8) -> u32 {
    let pack = match crate::phase9_5_contract::parse_effector_pack(include_bytes!(
        "../../phase9_5/examples/firestorm-r9.kpe9"
    )) {
        Ok(value) => value,
        Err(_) => return u32::MAX,
    };
    let mut state = match RcsState::new(&pack) {
        Ok(value) => value,
        Err(_) => return u32::MAX,
    };
    if index == 2 {
        state.remaining_propellant_q21 = pack.propellant_wet_mass_q21 / 2;
    }
    let mut command = RcsPulseCommand::ZERO;
    match index {
        0 | 2 => {
            command.quanta[4] = 1;
            command.quanta[5] = 1;
        }
        1 => command.quanta[0] = 1,
        _ => return u32::MAX,
    }
    if state
        .schedule_successor(0, command, &pack, [RcsJetFault::Healthy; MAX_RCS_JETS])
        .is_err()
    {
        return u32::MAX;
    }
    let mut status = NumericStatus::CLEAR;
    let result = match integrate_rcs_segment(
        &mut state,
        &pack,
        0,
        RCS_PULSE_QUANTUM_Q18,
        255_013_683,
        [RcsJetFault::Healthy; MAX_RCS_JETS],
        &mut status,
    ) {
        Ok(value) => value,
        Err(_) => return u32::MAX,
    };
    let expected = match index {
        0 => (
            independent_vectors::BALANCED_FORCE_Q23,
            independent_vectors::BALANCED_TORQUE_Q12,
            independent_vectors::BALANCED_MASS_FLOW_Q28,
            independent_vectors::BALANCED_IMPULSE_Q26,
            independent_vectors::BALANCED_CONSUMED_Q21,
            independent_vectors::BALANCED_THRUST_SCALE_Q30,
        ),
        1 => (
            independent_vectors::SINGLE_FORCE_Q23,
            independent_vectors::SINGLE_TORQUE_Q12,
            independent_vectors::SINGLE_MASS_FLOW_Q28,
            independent_vectors::SINGLE_IMPULSE_Q26,
            independent_vectors::SINGLE_CONSUMED_Q21,
            independent_vectors::SINGLE_THRUST_SCALE_Q30,
        ),
        2 => (
            independent_vectors::HALF_SUPPLY_FORCE_Q23,
            independent_vectors::HALF_SUPPLY_TORQUE_Q12,
            independent_vectors::HALF_SUPPLY_MASS_FLOW_Q28,
            independent_vectors::HALF_SUPPLY_IMPULSE_Q26,
            independent_vectors::HALF_SUPPLY_CONSUMED_Q21,
            independent_vectors::HALF_SUPPLY_THRUST_SCALE_Q30,
        ),
        _ => return u32::MAX,
    };
    u32::from(result.force_body_q23 != expected.0)
        | (u32::from(result.torque_body_q12 != expected.1) << 1)
        | (u32::from(result.total_mass_flow_q28 != expected.2) << 2)
        | (u32::from(result.total_impulse_q26 != expected.3) << 3)
        | (u32::from(result.consumed_propellant_q21 != expected.4) << 4)
        | (u32::from(result.thrust_scale_q30 != expected.5) << 5)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pack() -> AdvancedEffectorPack {
        crate::phase9_5_contract::parse_effector_pack(include_bytes!(
            "../../phase9_5/examples/firestorm-r9.kpe9"
        ))
        .unwrap()
    }
    #[test]
    fn independent_exact_vectors_match() {
        for index in 0..3 {
            assert_eq!(run_phase95_rcs_case(index), 0, "case {index}");
        }
    }
    #[test]
    fn exact_edges_and_split_requirement_are_enforced() {
        let p = pack();
        let mut s = RcsState::new(&p).unwrap();
        let mut c = RcsPulseCommand::ZERO;
        c.quanta[0] = 2;
        s.schedule_successor(0, c, &p, [RcsJetFault::Healthy; 12])
            .unwrap();
        assert_eq!(s.next_valve_edge_after(-1, 8192), Some(0));
        assert_eq!(s.next_valve_edge_after(0, 8192), Some(2048));
        let mut n = NumericStatus::CLEAR;
        assert_eq!(
            integrate_rcs_segment(
                &mut s,
                &p,
                0,
                4096,
                255_013_683,
                [RcsJetFault::Healthy; 12],
                &mut n
            ),
            Err(RcsError::EventSplitRequired(2048))
        );
    }
    #[test]
    fn minimum_impulse_accumulates_and_duplicates_fail_closed() {
        let mut p = pack();
        p.jets[0].min_pulse_quanta = 2;
        let mut s = RcsState::new(&p).unwrap();
        let mut c = RcsPulseCommand::ZERO;
        c.quanta[0] = 1;
        s.schedule_successor(0, c, &p, [RcsJetFault::Healthy; 12])
            .unwrap();
        assert_eq!(s.next_valve_edge_after(0, 8192), None);
        assert_eq!(s.accumulated_quanta()[0], 1);
        assert_eq!(
            s.schedule_successor(0, c, &p, [RcsJetFault::Healthy; 12]),
            Err(RcsError::DuplicateOrStaleCommand)
        );
        s.schedule_successor(8192, c, &p, [RcsJetFault::Healthy; 12])
            .unwrap();
        assert_eq!(s.next_valve_edge_after(8191, 16384), Some(8192));
    }
    #[test]
    fn faults_preserve_translation_and_safeing_cannot_close_stuck_open() {
        let p = pack();
        let mut s = RcsState::new(&p).unwrap();
        s.safe();
        let mut n = NumericStatus::CLEAR;
        let mut f = [RcsJetFault::Healthy; 12];
        f[0] = RcsJetFault::StuckOpen;
        let r = integrate_rcs_segment(&mut s, &p, 0, 1024, 255_013_683, f, &mut n).unwrap();
        assert_ne!(r.force_body_q23, [0; 3]);
        assert_eq!(r.active_mask, 1);
    }
    #[test]
    fn delayed_leak_degraded_and_stuck_closed_faults_are_explicit() {
        let p = pack();
        let mut delayed = RcsState::new(&p).unwrap();
        let mut command = RcsPulseCommand::ZERO;
        command.quanta[0] = 1;
        let mut delayed_faults = [RcsJetFault::Healthy; 12];
        delayed_faults[0] = RcsJetFault::DelayedValveOneQuantum;
        delayed
            .schedule_successor(0, command, &p, delayed_faults)
            .unwrap();
        assert_eq!(delayed.next_valve_edge_after(0, 8_192), Some(1_024));
        assert_eq!(delayed.next_valve_edge_after(1_024, 8_192), Some(2_048));

        let mut degraded = RcsState::new(&p).unwrap();
        degraded
            .schedule_successor(0, command, &p, [RcsJetFault::Healthy; 12])
            .unwrap();
        let mut faults = [RcsJetFault::Healthy; 12];
        faults[0] = RcsJetFault::DegradedThrustHalf;
        let mut status = NumericStatus::CLEAR;
        let half = integrate_rcs_segment(
            &mut degraded,
            &p,
            0,
            1_024,
            255_013_683,
            faults,
            &mut status,
        )
        .unwrap();
        assert_eq!(half.individual_thrust_q23[0], 1 << 22);

        let mut leaking = RcsState::new(&p).unwrap();
        faults = [RcsJetFault::Healthy; 12];
        faults[0] = RcsJetFault::Leak;
        status = NumericStatus::CLEAR;
        let leak =
            integrate_rcs_segment(&mut leaking, &p, 0, 1_024, 255_013_683, faults, &mut status)
                .unwrap();
        assert_eq!(leak.individual_thrust_q23[0], 1 << 19);

        let mut closed = RcsState::new(&p).unwrap();
        faults[0] = RcsJetFault::StuckClosed;
        closed.schedule_successor(0, command, &p, faults).unwrap();
        assert_eq!(closed.next_valve_edge_after(-1, 8_192), None);
    }

    #[test]
    fn regulated_and_blowdown_sources_share_runtime_path() {
        let blow = pack();
        let regulated = crate::phase9_5_contract::parse_effector_pack(include_bytes!(
            "../../phase9_5/examples/ksa-x1.kpe9"
        ))
        .unwrap();
        let mut n = NumericStatus::CLEAR;
        let b = sample_supply(&blow, blow.propellant_wet_mass_q21 / 2, &mut n).unwrap();
        let r = sample_supply(&regulated, regulated.propellant_wet_mass_q21 / 2, &mut n).unwrap();
        assert!(b.thrust_scale_q30 < ONE_Q30);
        assert_eq!(r.thrust_scale_q30, ONE_Q30);
    }
    #[test]
    fn propellant_mass_properties_follow_remaining_mass() {
        let p = pack();
        let mut n = NumericStatus::CLEAR;
        let full = propellant_mass_properties(&p, p.propellant_wet_mass_q21, &mut n).unwrap();
        let half = propellant_mass_properties(&p, p.propellant_wet_mass_q21 / 2, &mut n).unwrap();
        assert!(half.first_moment_q21[0] < full.first_moment_q21[0]);
        assert!(half.inertia_q19[1] < full.inertia_q19[1]);
    }
    #[test]
    fn exact_depletion_stops_before_requested_end() {
        let p = pack();
        let mut s = RcsState::new(&p).unwrap();
        s.remaining_propellant_q21 = 5;
        let mut c = RcsPulseCommand::ZERO;
        c.quanta[0] = 8;
        s.schedule_successor(0, c, &p, [RcsJetFault::Healthy; 12])
            .unwrap();
        let mut n = NumericStatus::CLEAR;
        let r = integrate_rcs_segment(
            &mut s,
            &p,
            0,
            8192,
            255_013_683,
            [RcsJetFault::Healthy; 12],
            &mut n,
        )
        .unwrap();
        assert!(r.depleted);
        assert_eq!(r.remaining_propellant_q21, 0);
        assert!(r.integrated_end_q18 < 8192);
    }
}
