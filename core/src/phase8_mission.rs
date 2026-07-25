//! Phase 8 deterministic Firestorm-class spatial mission composition.

use crate::evaluation::EvaluationOutcome;
use crate::phase8_numeric::{
    EnuPosition, EnuVelocity, SpatialInertia, SpatialMass, SpatialMomentArm, SpatialTime,
};
use crate::phase8_world::HobbySpatialState;

mod advance;
mod forces;
mod machine;
mod propulsion;
mod step;

pub use machine::Phase8MissionMachine;
pub use step::run_phase8_mission;

pub const EVENT_RAIL_EXIT: u16 = 1 << 0;
pub const EVENT_BURNOUT: u16 = 1 << 1;
pub const EVENT_APOGEE: u16 = 1 << 2;
pub const EVENT_DROGUE: u16 = 1 << 3;
pub const EVENT_MAIN: u16 = 1 << 4;
pub const EVENT_LANDING: u16 = 1 << 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase85AppliedControl {
    /// Signed full-turn binary angles: 65536 units per revolution.
    pub gimbal_turn16: [i16; 2],
    pub pivot_from_nose_q28: i32,
}

impl Phase85AppliedControl {
    pub const NEUTRAL: Self = Self {
        gimbal_turn16: [0; 2],
        pivot_from_nose_q28: 0,
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Phase85DeploymentCommand {
    pub drogue: bool,
    pub main: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HobbySpatialPhase {
    ConstrainedPowered = 0,
    PoweredFlight = 1,
    Coast = 2,
    DrogueRecovery = 3,
    MainRecovery = 4,
    Complete = 5,
    Failed = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase8MissionError {
    Configuration,
    Numeric,
    ModelEnvelopeExceeded,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialMassProperties {
    pub mass: SpatialMass,
    pub cg_from_nose: SpatialMomentArm,
    pub inertia: [SpatialInertia; 3],
    pub propellant_remaining: SpatialMass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialAeroState {
    pub mach_q24: i32,
    pub angle_of_attack_q28: i32,
    pub dynamic_pressure_q13: i32,
    pub axial_drag_q13: i32,
    pub normal_force_q13: i32,
    pub static_margin_q24: i32,
}

impl SpatialAeroState {
    pub const ZERO: Self = Self {
        mach_q24: 0,
        angle_of_attack_q28: 0,
        dynamic_pressure_q13: 0,
        axial_drag_q13: 0,
        normal_force_q13: 0,
        static_margin_q24: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase8Milestone {
    pub time: SpatialTime,
    pub position: EnuPosition,
    pub velocity: EnuVelocity,
}

impl Phase8Milestone {
    pub const ZERO: Self = Self {
        time: SpatialTime::ZERO,
        position: EnuPosition::ZERO,
        velocity: EnuVelocity::ZERO,
    };

    pub(super) fn from_state(state: HobbySpatialState) -> Self {
        Self {
            time: state.time,
            position: state.position,
            velocity: state.velocity,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase8MissionSnapshot {
    pub state: HobbySpatialState,
    pub phase: HobbySpatialPhase,
    pub events: u16,
    pub mass: SpatialMassProperties,
    pub thrust_q13: i32,
    pub aero: SpatialAeroState,
    pub wind_q22: [i32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase8MissionResult {
    pub outcome: EvaluationOutcome,
    pub steps: u32,
    pub final_snapshot: Phase8MissionSnapshot,
    pub rail_exit: Phase8Milestone,
    pub burnout: Phase8Milestone,
    pub apogee: Phase8Milestone,
    pub drogue: Phase8Milestone,
    pub main: Phase8Milestone,
    pub landing: Phase8Milestone,
    pub max_altitude_raw_q13: i32,
    pub max_speed_raw_q19: i32,
    pub max_acceleration_raw_q19: i32,
    pub max_dynamic_pressure_raw_q13: i32,
    pub max_aoa_raw_q28: i32,
    pub max_angular_rate_raw_q24: i32,
    pub max_wind_raw_q22: i32,
    pub minimum_static_margin_raw_q24: i32,
    pub rail_exit_static_margin_raw_q24: i32,
    pub burnout_static_margin_raw_q24: i32,
    pub max_lateral_acceleration_raw_q19: i32,
    pub event_history: u16,
    pub checksum: u32,
}

/// Small completion view for memory-constrained target runners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase8CompactResult {
    pub outcome: EvaluationOutcome,
    pub steps: u32,
    pub max_altitude_raw_q13: i32,
    pub event_history: u16,
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialMissionVariation {
    pub mass_scale_ppm: i32,
    pub thrust_scale_ppm: i32,
    pub axial_drag_scale_ppm: i32,
    pub normal_force_scale_ppm: i32,
    pub cp_offset_q28: i32,
    pub density_scale_ppm: i32,
    pub wind_scale_ppm: i32,
    pub recovery_cda_scale_ppm: i32,
    pub inflation_scale_ppm: i32,
}

impl SpatialMissionVariation {
    pub const NOMINAL: Self = Self {
        mass_scale_ppm: 1_000_000,
        thrust_scale_ppm: 1_000_000,
        axial_drag_scale_ppm: 1_000_000,
        normal_force_scale_ppm: 1_000_000,
        cp_offset_q28: 0,
        density_scale_ppm: 1_000_000,
        wind_scale_ppm: 1_000_000,
        recovery_cda_scale_ppm: 1_000_000,
        inflation_scale_ppm: 1_000_000,
    };

    pub const fn is_valid(self) -> bool {
        self.mass_scale_ppm >= 500_000
            && self.mass_scale_ppm <= 1_500_000
            && self.thrust_scale_ppm >= 500_000
            && self.thrust_scale_ppm <= 1_500_000
            && self.axial_drag_scale_ppm >= 500_000
            && self.axial_drag_scale_ppm <= 1_500_000
            && self.normal_force_scale_ppm >= 500_000
            && self.normal_force_scale_ppm <= 1_500_000
            && self.density_scale_ppm >= 500_000
            && self.density_scale_ppm <= 1_500_000
            && self.wind_scale_ppm >= 0
            && self.wind_scale_ppm <= 2_000_000
            && self.recovery_cda_scale_ppm >= 500_000
            && self.recovery_cda_scale_ppm <= 1_500_000
            && self.inflation_scale_ppm >= 500_000
            && self.inflation_scale_ppm <= 1_500_000
    }
}

/// Advance the canonical exact-arithmetic trace checksum by one snapshot.
///
/// This is public solely so bounded host and MOS acceptance probes can compare
/// the portable evaluator without duplicating its frozen operation order.
pub fn phase8_snapshot_checksum(mut hash: u32, snapshot: Phase8MissionSnapshot) -> u32 {
    for value in [
        snapshot.state.time.raw(),
        snapshot.state.position.x(),
        snapshot.state.position.y(),
        snapshot.state.position.z(),
        snapshot.state.velocity.x(),
        snapshot.state.velocity.y(),
        snapshot.state.velocity.z(),
        snapshot.state.attitude.w(),
        snapshot.state.attitude.x(),
        snapshot.state.attitude.y(),
        snapshot.state.attitude.z(),
        snapshot.mass.mass.raw(),
        snapshot.mass.propellant_remaining.raw(),
        snapshot.thrust_q13,
        snapshot.aero.dynamic_pressure_q13,
        snapshot.events as i32,
        snapshot.phase as i32,
    ] {
        hash ^= value as u32;
        hash = hash.wrapping_mul(0x0100_0193).rotate_left(5);
    }
    hash
}

pub(super) fn magnitude3_i32<const F: u8>(
    vector: crate::spatial_numeric::FixedVec3<F>,
    status: &mut crate::numeric::NumericStatus,
) -> i32 {
    crate::numeric::magnitude3_floor(vector.x(), vector.y(), vector.z(), status)
        .min(i32::MAX as u32) as i32
}
