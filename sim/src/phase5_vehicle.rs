//! Phase 5 multirate vehicle, gimbal, inertia, staging, and RCS coupling.
//!
//! This module is additive: the accepted Phase 3/4 planar machine remains
//! unchanged. The spatial machine advances exactly four 0.03125 s fast steps
//! per 0.125 s mission command and commits state only when every checked
//! operation succeeds.

use ksa64_core::aerodynamics::AeroTable;
use ksa64_core::flexible::{
    step_flexible_modes, FlexibleDriveQ24, FlexibleParametersQ16, FlexibleStateQ24,
    ModalParametersQ16,
};
use ksa64_core::numeric::{
    add, divide_scaled, multiply_scaled, subtract, NumericFault, NumericStatus,
};
use ksa64_core::phase2_mission::{EVENT_CUTOFF, EVENT_IGNITION, EVENT_SEPARATION};
use ksa64_core::phase2_quantities::Mach;
use ksa64_core::phase2_scenario::{Phase2Scenario, KSA2A_NOMINAL_SCENARIO_ID};
use ksa64_core::rigid_body::{step_rigid_body, DiagonalInertiaQ12, RigidBodyState};
use ksa64_core::spatial_numeric::{FixedVec3, ForceVec, QuaternionQ30, TorqueVec};
use ksa64_core::spatial_world::{
    advance_spatial_state, evaluate_spatial_aerodynamics, evaluate_spatial_environment,
    SpatialAeroConfig, SpatialState,
};
use ksa64_interface::EngineAction;

#[allow(dead_code)]
mod generated {
    include!("../../phase5/generated/vehicle_v1.rs");
}

#[allow(dead_code)]
mod world_vectors {
    include!("../../phase5/generated/spatial_world_tables_v1.rs");
}

pub const PHASE5_STAGE_COUNT: usize = 2;
pub const PHASE5_VEHICLE_SIGNATURE: u32 = generated::VEHICLE_SIGNATURE;
pub const PHASE5_FAST_STEP_Q16: i32 = generated::FAST_STEP_Q16;
pub const PHASE5_SUBSTEPS: u8 = generated::SUBSTEPS;
pub const PHASE5_GIMBAL_LIMIT_Q16: i32 = generated::GIMBAL_LIMIT_Q16;
pub const PHASE5_GIMBAL_SLEW_PER_FAST_STEP_Q16: i32 = generated::GIMBAL_SLEW_PER_FAST_STEP_Q16;
pub const PHASE5_RCS_PROPELLANT_Q12: i32 = generated::RCS_PROPELLANT_Q12;
pub const PHASE5_RCS_MAX_TORQUE_Q16: i32 = generated::RCS_MAX_TORQUE_Q16;
pub const EVENT_RCS_DEPLETED: u16 = 1 << 8;
pub const EVENT_GIMBAL_JAMMED: u16 = 1 << 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase5VehicleError {
    Configuration,
    NumericFault,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase5StagePhase {
    CoastBeforeIgnition,
    Burning,
    CoastBeforeSeparation,
    Complete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct GimbalCommandQ16 {
    pub pitch: i32,
    pub yaw: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GimbalSnapshotQ16 {
    pub requested: GimbalCommandQ16,
    pub lagged: GimbalCommandQ16,
    pub applied: GimbalCommandQ16,
    pub pitch_jammed: bool,
    pub yaw_jammed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GimbalAxisQ16 {
    requested: i32,
    lagged: i32,
    applied: i32,
    jammed: bool,
}

impl GimbalAxisQ16 {
    fn advance(&mut self, request: i32) {
        self.requested = request.clamp(-generated::GIMBAL_LIMIT_Q16, generated::GIMBAL_LIMIT_Q16);
        let error = self.requested - self.lagged;
        self.lagged += error / generated::GIMBAL_LAG_STEPS as i32;
        if !self.jammed {
            let delta = (self.lagged - self.applied).clamp(
                -generated::GIMBAL_SLEW_PER_FAST_STEP_Q16,
                generated::GIMBAL_SLEW_PER_FAST_STEP_Q16,
            );
            self.applied += delta;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TwoAxisGimbalQ16 {
    pitch: GimbalAxisQ16,
    yaw: GimbalAxisQ16,
}

impl TwoAxisGimbalQ16 {
    pub const fn new() -> Self {
        Self {
            pitch: GimbalAxisQ16 {
                requested: 0,
                lagged: 0,
                applied: 0,
                jammed: false,
            },
            yaw: GimbalAxisQ16 {
                requested: 0,
                lagged: 0,
                applied: 0,
                jammed: false,
            },
        }
    }

    pub fn set_jammed(&mut self, pitch: bool, yaw: bool) {
        self.pitch.jammed = pitch;
        self.yaw.jammed = yaw;
    }

    pub fn jam_at(&mut self, pitch_q16: i32, yaw_q16: i32) {
        self.pitch.applied =
            pitch_q16.clamp(-generated::GIMBAL_LIMIT_Q16, generated::GIMBAL_LIMIT_Q16);
        self.pitch.lagged = self.pitch.applied;
        self.pitch.jammed = true;
        self.yaw.applied = yaw_q16.clamp(-generated::GIMBAL_LIMIT_Q16, generated::GIMBAL_LIMIT_Q16);
        self.yaw.lagged = self.yaw.applied;
        self.yaw.jammed = true;
    }

    pub fn advance(&mut self, command: GimbalCommandQ16) -> GimbalSnapshotQ16 {
        self.pitch.advance(command.pitch);
        self.yaw.advance(command.yaw);
        self.snapshot()
    }

    pub const fn snapshot(self) -> GimbalSnapshotQ16 {
        GimbalSnapshotQ16 {
            requested: GimbalCommandQ16 {
                pitch: self.pitch.requested,
                yaw: self.yaw.requested,
            },
            lagged: GimbalCommandQ16 {
                pitch: self.pitch.lagged,
                yaw: self.yaw.lagged,
            },
            applied: GimbalCommandQ16 {
                pitch: self.pitch.applied,
                yaw: self.yaw.applied,
            },
            pitch_jammed: self.pitch.jammed,
            yaw_jammed: self.yaw.jammed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageInertiaScheduleQ12 {
    dry: DiagonalInertiaQ12,
    wet: DiagonalInertiaQ12,
    full_propellant_q12: i32,
}

impl StageInertiaScheduleQ12 {
    pub const fn new(
        dry: DiagonalInertiaQ12,
        wet: DiagonalInertiaQ12,
        full_propellant_q12: i32,
    ) -> Self {
        Self {
            dry,
            wet,
            full_propellant_q12,
        }
    }

    pub fn interpolate(
        self,
        remaining_propellant_q12: i32,
        status: &mut NumericStatus,
    ) -> DiagonalInertiaQ12 {
        if !self.dry.is_valid() || !self.wet.is_valid() || self.full_propellant_q12 <= 0 {
            status.record(NumericFault::InvalidInput);
            return self.dry;
        }
        let fraction_q16 = divide_scaled(
            remaining_propellant_q12.clamp(0, self.full_propellant_q12),
            self.full_propellant_q12,
            16,
            status,
        );
        let axis = |dry: i32, wet: i32, status: &mut NumericStatus| {
            add(
                dry,
                multiply_scaled(subtract(wet, dry, status), fraction_q16, 16, status),
                status,
            )
        };
        DiagonalInertiaQ12::new(
            axis(self.dry.x(), self.wet.x(), status),
            axis(self.dry.y(), self.wet.y(), status),
            axis(self.dry.z(), self.wet.z(), status),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5StageConfig {
    pub dry_mass_q12: i32,
    pub propellant_mass_q12: i32,
    pub thrust_q12: i32,
    pub mass_flow_q16: i32,
    pub burn_mission_steps: u32,
    pub separation_delay_steps: u16,
    pub ignition_delay_steps: u16,
    pub gimbal_arm_q16: i32,
    pub area_q16: i32,
    pub normal_slope_q14: i32,
    pub cp_aft_q16: i32,
    pub rate_damping_q16: i32,
    pub inertia: StageInertiaScheduleQ12,
    pub flexible: FlexibleParametersQ16,
}

pub fn ksa5a_stage(index: u8) -> Option<Phase5StageConfig> {
    let i = index as usize;
    if i >= PHASE5_STAGE_COUNT {
        return None;
    }
    let dry = generated::STAGE_INERTIA_DRY_Q12[i];
    let wet = generated::STAGE_INERTIA_WET_Q12[i];
    Some(Phase5StageConfig {
        dry_mass_q12: generated::STAGE_DRY_MASS_Q12[i],
        propellant_mass_q12: generated::STAGE_PROPELLANT_MASS_Q12[i],
        thrust_q12: generated::STAGE_THRUST_Q12[i],
        mass_flow_q16: generated::STAGE_MASS_FLOW_Q16[i],
        burn_mission_steps: generated::STAGE_BURN_MISSION_STEPS[i],
        separation_delay_steps: generated::STAGE_SEPARATION_DELAY_STEPS[i],
        ignition_delay_steps: generated::STAGE_IGNITION_DELAY_STEPS[i],
        gimbal_arm_q16: generated::STAGE_GIMBAL_ARM_Q16[i],
        area_q16: generated::STAGE_AREA_Q16[i],
        normal_slope_q14: generated::STAGE_NORMAL_SLOPE_Q14[i],
        cp_aft_q16: generated::STAGE_CP_AFT_Q16[i],
        rate_damping_q16: generated::STAGE_RATE_DAMPING_Q16[i],
        inertia: StageInertiaScheduleQ12::new(
            DiagonalInertiaQ12::new(dry[0], dry[1], dry[2]),
            DiagonalInertiaQ12::new(wet[0], wet[1], wet[2]),
            generated::STAGE_PROPELLANT_MASS_Q12[i],
        ),
        flexible: FlexibleParametersQ16::new(
            ModalParametersQ16::new(
                generated::STAGE_BEND_OMEGA_Q16[i],
                generated::STAGE_BEND_ZETA_Q16[i],
                generated::FLEX_BEND_DRIVE_GAIN_Q16,
            ),
            ModalParametersQ16::new(
                generated::STAGE_SLOSH_OMEGA_Q16[i],
                generated::STAGE_SLOSH_ZETA_Q16[i],
                generated::FLEX_SLOSH_DRIVE_GAIN_Q16,
            ),
        ),
    })
}

fn engine_force_and_torque(
    thrust_q12: i32,
    gimbal: GimbalCommandQ16,
    arm_q16: i32,
    status: &mut NumericStatus,
) -> (ForceVec, TorqueVec) {
    let pitch2_q16 = multiply_scaled(gimbal.pitch, gimbal.pitch, 16, status);
    let yaw2_q16 = multiply_scaled(gimbal.yaw, gimbal.yaw, 16, status);
    let axial_factor_q16 = subtract(65_536, add(pitch2_q16, yaw2_q16, status) >> 1, status);
    let force = ForceVec::new(
        multiply_scaled(thrust_q12, axial_factor_q16, 16, status),
        multiply_scaled(thrust_q12, gimbal.yaw, 16, status),
        subtract(
            0,
            multiply_scaled(thrust_q12, gimbal.pitch, 16, status),
            status,
        ),
    );
    let torque = TorqueVec::new(
        0,
        multiply_scaled(arm_q16, force.z(), 12, status),
        subtract(0, multiply_scaled(arm_q16, force.y(), 12, status), status),
    );
    (force, torque)
}

fn rate_damping_torque(
    rate: FixedVec3<24>,
    damping_q16: i32,
    status: &mut NumericStatus,
) -> TorqueVec {
    TorqueVec::new(
        0,
        subtract(
            0,
            multiply_scaled(rate.y(), damping_q16, 24, status),
            status,
        ),
        subtract(
            0,
            multiply_scaled(rate.z(), damping_q16, 24, status),
            status,
        ),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct RcsSystem {
    propellant_q12: i32,
    residual_q24: i32,
    leak_q15: FixedVec3<15>,
}

impl RcsSystem {
    pub const fn new() -> Self {
        Self {
            propellant_q12: generated::RCS_PROPELLANT_Q12,
            residual_q24: 0,
            leak_q15: FixedVec3::ZERO,
        }
    }

    pub const fn propellant_q12(self) -> i32 {
        self.propellant_q12
    }

    pub fn set_leak_q15(&mut self, leak: FixedVec3<15>) {
        self.leak_q15 = FixedVec3::new(
            leak.x().clamp(-32_767, 32_767),
            leak.y().clamp(-32_767, 32_767),
            leak.z().clamp(-32_767, 32_767),
        );
    }

    fn step(
        &mut self,
        command: FixedVec3<15>,
        available: bool,
        timestep_q16: i32,
        status: &mut NumericStatus,
    ) -> (TorqueVec, i32, bool) {
        if !available || self.propellant_q12 <= 0 {
            return (TorqueVec::ZERO, 0, self.propellant_q12 <= 0);
        }
        let effective = FixedVec3::<15>::new(
            add(command.x(), self.leak_q15.x(), status).clamp(-32_767, 32_767),
            add(command.y(), self.leak_q15.y(), status).clamp(-32_767, 32_767),
            add(command.z(), self.leak_q15.z(), status).clamp(-32_767, 32_767),
        );
        let torque = TorqueVec::new(
            multiply_scaled(generated::RCS_MAX_TORQUE_Q16, effective.x(), 15, status),
            multiply_scaled(generated::RCS_MAX_TORQUE_Q16, effective.y(), 15, status),
            multiply_scaled(generated::RCS_MAX_TORQUE_Q16, effective.z(), 15, status),
        );
        let flow_x = multiply_scaled(
            generated::RCS_MAX_AXIS_MASS_FLOW_Q24,
            effective.x().abs(),
            15,
            status,
        );
        let flow_y = multiply_scaled(
            generated::RCS_MAX_AXIS_MASS_FLOW_Q24,
            effective.y().abs(),
            15,
            status,
        );
        let flow_z = multiply_scaled(
            generated::RCS_MAX_AXIS_MASS_FLOW_Q24,
            effective.z().abs(),
            15,
            status,
        );
        let total_flow_q24 = add(add(flow_x, flow_y, status), flow_z, status);
        let consumed_q24 = multiply_scaled(total_flow_q24, timestep_q16, 16, status);
        self.residual_q24 = add(self.residual_q24, consumed_q24, status);
        let requested_q12 = self.residual_q24 >> 12;
        let consumed_q12 = requested_q12.min(self.propellant_q12);
        self.propellant_q12 = subtract(self.propellant_q12, consumed_q12, status);
        self.residual_q24 = subtract(self.residual_q24, consumed_q12 << 12, status);
        if self.propellant_q12 == 0 {
            self.residual_q24 = 0;
        }
        (torque, consumed_q12, self.propellant_q12 == 0)
    }
}

impl Default for RcsSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5VehicleTruth {
    spatial: SpatialState,
    rigid: RigidBodyState,
    flexible: FlexibleStateQ24,
    total_mass_q12: i32,
    active_propellant_q12: i32,
    active_stage: u8,
    phase: Phase5StagePhase,
    step: u32,
    time_q16: i32,
}

impl Phase5VehicleTruth {
    pub const fn spatial(self) -> SpatialState {
        self.spatial
    }
    pub const fn rigid(self) -> RigidBodyState {
        self.rigid
    }
    pub const fn flexible(self) -> FlexibleStateQ24 {
        self.flexible
    }
    pub const fn total_mass_q12(self) -> i32 {
        self.total_mass_q12
    }
    pub const fn active_propellant_q12(self) -> i32 {
        self.active_propellant_q12
    }
    pub const fn active_stage(self) -> u8 {
        self.active_stage
    }
    pub const fn phase(self) -> Phase5StagePhase {
        self.phase
    }
    pub const fn step(self) -> u32 {
        self.step
    }
    pub const fn time_q16(self) -> i32 {
        self.time_q16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5VehicleCommand {
    pub gimbal: GimbalCommandQ16,
    pub rcs_q15: FixedVec3<15>,
    pub engine_action: EngineAction,
    pub separate: bool,
    pub abort_safeing: bool,
}

impl Phase5VehicleCommand {
    pub const HOLD: Self = Self {
        gimbal: GimbalCommandQ16 { pitch: 0, yaw: 0 },
        rcs_q15: FixedVec3::ZERO,
        engine_action: EngineAction::Hold,
        separate: false,
        abort_safeing: false,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase5VehicleSnapshot {
    pub truth: Phase5VehicleTruth,
    pub gimbal: GimbalSnapshotQ16,
    pub inertia: DiagonalInertiaQ12,
    pub rcs_propellant_q12: i32,
    pub events: u16,
    pub mach: Mach,
    pub dynamic_pressure_q16: i32,
    pub angle_of_attack_sine_q16: i32,
}

#[derive(Clone, Copy)]
pub struct Phase5VehicleMachine {
    truth: Phase5VehicleTruth,
    gimbal: TwoAxisGimbalQ16,
    rcs: RcsSystem,
    engine_residual_q24: i32,
    burn_fast_steps: u32,
    phase_steps: u32,
}

impl Phase5VehicleMachine {
    pub fn new_ksa5a_checked(scenario: &Phase2Scenario) -> Result<Self, Phase5VehicleError> {
        if !validate_base_scenario(scenario) {
            return Err(Phase5VehicleError::Configuration);
        }
        Self::new_ksa5a()
    }

    pub fn new_ksa5a() -> Result<Self, Phase5VehicleError> {
        if generated::FAST_STEP_Q16 != ksa64_core::phase5_contract::PHASE5_FAST_STEP_Q16
            || generated::SUBSTEPS != ksa64_core::phase5_contract::PHASE5_ATTITUDE_SUBSTEPS
            || generated::RCS_PROPELLANT_Q12
                != ksa64_core::phase5_contract::PHASE5_RCS_PROPELLANT_Q12
        {
            return Err(Phase5VehicleError::Configuration);
        }
        let spatial = SpatialState::new(
            FixedVec3::new(
                world_vectors::LAUNCH_POSITION_Q12[0],
                world_vectors::LAUNCH_POSITION_Q12[1],
                world_vectors::LAUNCH_POSITION_Q12[2],
            ),
            FixedVec3::new(
                world_vectors::LAUNCH_COROTATION_VELOCITY_Q24[0],
                world_vectors::LAUNCH_COROTATION_VELOCITY_Q24[1],
                world_vectors::LAUNCH_COROTATION_VELOCITY_Q24[2],
            ),
        );
        let q = generated::INITIAL_ATTITUDE_Q30;
        let rigid =
            RigidBodyState::new(QuaternionQ30::new(q[0], q[1], q[2], q[3]), FixedVec3::ZERO);
        let mut status = NumericStatus::CLEAR;
        let nose = rigid
            .attitude()
            .rotate(FixedVec3::<30>::new(1 << 30, 0, 0), &mut status);
        if !status.is_clear() || nose.x() <= 0 || nose.z() <= 0 {
            return Err(Phase5VehicleError::NumericFault);
        }
        Ok(Self {
            truth: Phase5VehicleTruth {
                spatial,
                rigid,
                flexible: FlexibleStateQ24::ZERO,
                total_mass_q12: generated::INITIAL_TOTAL_MASS_Q12,
                active_propellant_q12: generated::STAGE_PROPELLANT_MASS_Q12[0],
                active_stage: 0,
                phase: Phase5StagePhase::CoastBeforeIgnition,
                step: 0,
                time_q16: 0,
            },
            gimbal: TwoAxisGimbalQ16::new(),
            rcs: RcsSystem::new(),
            engine_residual_q24: 0,
            burn_fast_steps: 0,
            phase_steps: 0,
        })
    }

    pub const fn truth(&self) -> Phase5VehicleTruth {
        self.truth
    }
    pub const fn gimbal(&self) -> GimbalSnapshotQ16 {
        self.gimbal.snapshot()
    }
    pub const fn rcs_propellant_q12(&self) -> i32 {
        self.rcs.propellant_q12()
    }

    pub fn set_gimbal_jammed(&mut self, pitch: bool, yaw: bool) {
        self.gimbal.set_jammed(pitch, yaw);
    }

    pub fn jam_gimbal_at(&mut self, pitch_q16: i32, yaw_q16: i32) {
        self.gimbal.jam_at(pitch_q16, yaw_q16);
    }

    pub fn set_rcs_leak_q15(&mut self, leak: FixedVec3<15>) {
        self.rcs.set_leak_q15(leak);
    }

    fn stage(&self) -> Result<Phase5StageConfig, Phase5VehicleError> {
        ksa5a_stage(self.truth.active_stage).ok_or(Phase5VehicleError::Configuration)
    }

    fn apply_boundary_command(
        &mut self,
        command: Phase5VehicleCommand,
        events: &mut u16,
        status: &mut NumericStatus,
    ) -> Result<(), Phase5VehicleError> {
        let stage = self.stage()?;
        if (command.abort_safeing || command.engine_action == EngineAction::Cutoff)
            && self.truth.phase == Phase5StagePhase::Burning
        {
            self.truth.phase = if self.truth.active_stage + 1 < PHASE5_STAGE_COUNT as u8 {
                Phase5StagePhase::CoastBeforeSeparation
            } else {
                Phase5StagePhase::Complete
            };
            self.phase_steps = 0;
            self.engine_residual_q24 = 0;
            *events |= EVENT_CUTOFF;
        }
        if command.separate
            && self.truth.phase == Phase5StagePhase::CoastBeforeSeparation
            && self.phase_steps >= stage.separation_delay_steps as u32
        {
            let discarded = add(stage.dry_mass_q12, self.truth.active_propellant_q12, status);
            self.truth.total_mass_q12 = subtract(self.truth.total_mass_q12, discarded, status);
            self.truth.active_stage += 1;
            let next = self.stage()?;
            self.truth.active_propellant_q12 = next.propellant_mass_q12;
            self.truth.phase = Phase5StagePhase::CoastBeforeIgnition;
            self.phase_steps = 0;
            self.burn_fast_steps = 0;
            self.engine_residual_q24 = 0;
            *events |= EVENT_SEPARATION;
        }
        let stage = self.stage()?;
        if command.engine_action == EngineAction::Ignite
            && self.truth.phase == Phase5StagePhase::CoastBeforeIgnition
            && self.phase_steps >= stage.ignition_delay_steps as u32
        {
            self.truth.phase = Phase5StagePhase::Burning;
            self.phase_steps = 0;
            self.burn_fast_steps = 0;
            self.engine_residual_q24 = 0;
            *events |= EVENT_IGNITION;
        }
        Ok(())
    }

    fn consume_engine_propellant(
        &mut self,
        stage: Phase5StageConfig,
        events: &mut u16,
        status: &mut NumericStatus,
    ) {
        let consumed_q24 =
            multiply_scaled(stage.mass_flow_q16, generated::FAST_STEP_Q16, 8, status);
        self.engine_residual_q24 = add(self.engine_residual_q24, consumed_q24, status);
        let requested_q12 = self.engine_residual_q24 >> 12;
        let consumed_q12 = requested_q12.min(self.truth.active_propellant_q12);
        self.truth.active_propellant_q12 =
            subtract(self.truth.active_propellant_q12, consumed_q12, status);
        self.truth.total_mass_q12 = subtract(self.truth.total_mass_q12, consumed_q12, status);
        self.engine_residual_q24 = subtract(self.engine_residual_q24, consumed_q12 << 12, status);
        self.burn_fast_steps += 1;
        let burn_limit = stage
            .burn_mission_steps
            .saturating_mul(generated::SUBSTEPS as u32);
        if self.truth.active_propellant_q12 == 0 || self.burn_fast_steps >= burn_limit {
            self.truth.phase = if self.truth.active_stage + 1 < PHASE5_STAGE_COUNT as u8 {
                Phase5StagePhase::CoastBeforeSeparation
            } else {
                Phase5StagePhase::Complete
            };
            self.phase_steps = 0;
            self.engine_residual_q24 = 0;
            *events |= EVENT_CUTOFF;
        }
    }

    fn fast_step(
        &mut self,
        command: Phase5VehicleCommand,
        events: &mut u16,
        status: &mut NumericStatus,
    ) -> Result<(Mach, i32, i32), Phase5VehicleError> {
        let stage = self.stage()?;
        let gimbal = self.gimbal.advance(command.gimbal);
        if gimbal.pitch_jammed || gimbal.yaw_jammed {
            *events |= EVENT_GIMBAL_JAMMED;
        }
        let inertia = stage
            .inertia
            .interpolate(self.truth.active_propellant_q12, status);
        let environment = evaluate_spatial_environment(self.truth.spatial, status);
        let mach_q16 = if environment.sound_speed_q24() == 0 {
            0
        } else {
            divide_scaled(
                environment.air_speed_q24(),
                environment.sound_speed_q24(),
                16,
                status,
            )
        };
        let table =
            stage_aero_table(self.truth.active_stage).ok_or(Phase5VehicleError::Configuration)?;
        let coefficient = table.coefficient(Mach::from_raw(mach_q16), status);
        let aero = evaluate_spatial_aerodynamics(
            self.truth.rigid.attitude(),
            environment,
            SpatialAeroConfig::new(
                stage.area_q16,
                coefficient.raw(),
                stage.normal_slope_q14,
                stage.cp_aft_q16,
            ),
            status,
        );
        let (engine_body, gimbal_torque) = if self.truth.phase == Phase5StagePhase::Burning {
            engine_force_and_torque(
                stage.thrust_q12,
                gimbal.applied,
                stage.gimbal_arm_q16,
                status,
            )
        } else {
            (ForceVec::ZERO, TorqueVec::ZERO)
        };
        let engine_eci = self.truth.rigid.attitude().rotate(engine_body, status);
        let total_force_eci = aero.force_eci().checked_add(engine_eci, status);
        let prior_rcs = self.rcs.propellant_q12();
        let (rcs_torque, rcs_consumed, rcs_depleted) = self.rcs.step(
            command.rcs_q15,
            self.truth.active_stage == 1,
            generated::FAST_STEP_Q16,
            status,
        );
        if rcs_consumed > 0 {
            self.truth.total_mass_q12 = subtract(self.truth.total_mass_q12, rcs_consumed, status);
        }
        if rcs_depleted && prior_rcs > 0 {
            *events |= EVENT_RCS_DEPLETED;
        }
        let damping = rate_damping_torque(
            self.truth.rigid.angular_rate(),
            stage.rate_damping_q16,
            status,
        );
        let total_torque = aero
            .torque_body()
            .checked_add(gimbal_torque, status)
            .checked_add(rcs_torque, status)
            .checked_add(damping, status);
        let rigid_step = step_rigid_body(
            self.truth.rigid,
            inertia,
            total_torque,
            generated::FAST_STEP_Q16,
            status,
        );
        let aero_body = self
            .truth
            .rigid
            .attitude()
            .conjugate()
            .rotate(aero.force_eci(), status);
        let body_force = aero_body.checked_add(engine_body, status);
        let lateral_y_km_q24 = divide_scaled(body_force.y(), self.truth.total_mass_q12, 24, status);
        let lateral_z_km_q24 = divide_scaled(body_force.z(), self.truth.total_mass_q12, 24, status);
        let lateral_y_ms_q24 = multiply_scaled(lateral_y_km_q24, 1_000, 0, status);
        let lateral_z_ms_q24 = multiply_scaled(lateral_z_km_q24, 1_000, 0, status);
        self.truth.flexible = step_flexible_modes(
            self.truth.flexible,
            stage.flexible,
            FlexibleDriveQ24::new(
                lateral_y_ms_q24,
                lateral_z_ms_q24,
                lateral_y_ms_q24,
                lateral_z_ms_q24,
            ),
            generated::FAST_STEP_Q16,
            status,
        );
        self.truth.rigid = rigid_step.state();
        self.truth.spatial = advance_spatial_state(
            self.truth.spatial,
            total_force_eci,
            self.truth.total_mass_q12,
            generated::FAST_STEP_Q16,
            status,
        );
        if self.truth.phase == Phase5StagePhase::Burning {
            self.consume_engine_propellant(stage, events, status);
        }
        Ok((
            aero.mach(),
            aero.dynamic_pressure().raw(),
            aero.angle_of_attack_sine_q16(),
        ))
    }

    pub fn step(
        &mut self,
        command: Phase5VehicleCommand,
    ) -> Result<Phase5VehicleSnapshot, Phase5VehicleError> {
        if self.truth.step >= ksa64_core::phase5_contract::PHASE5_MISSION_STEPS {
            return Err(Phase5VehicleError::Complete);
        }
        let mut next = *self;
        let mut status = NumericStatus::CLEAR;
        let mut events = 0u16;
        next.apply_boundary_command(command, &mut events, &mut status)?;
        let phase_during_step = next.truth.phase;
        let mut mach = Mach::from_raw(0);
        let mut dynamic_pressure_q16 = 0;
        let mut angle_of_attack_sine_q16 = 0;
        let mut substep = 0;
        while substep < generated::SUBSTEPS {
            (mach, dynamic_pressure_q16, angle_of_attack_sine_q16) =
                next.fast_step(command, &mut events, &mut status)?;
            if !status.is_clear() {
                return Err(Phase5VehicleError::NumericFault);
            }
            substep += 1;
        }
        if next.truth.phase == phase_during_step
            && matches!(
                next.truth.phase,
                Phase5StagePhase::CoastBeforeIgnition | Phase5StagePhase::CoastBeforeSeparation
            )
        {
            next.phase_steps += 1;
        }
        next.truth.step += 1;
        next.truth.time_q16 = add(
            next.truth.time_q16,
            ksa64_core::phase5_contract::PHASE5_MISSION_STEP_Q16,
            &mut status,
        );
        if !status.is_clear() {
            return Err(Phase5VehicleError::NumericFault);
        }
        let stage = next.stage()?;
        let inertia = stage
            .inertia
            .interpolate(next.truth.active_propellant_q12, &mut status);
        if !status.is_clear() {
            return Err(Phase5VehicleError::NumericFault);
        }
        let snapshot = Phase5VehicleSnapshot {
            truth: next.truth,
            gimbal: next.gimbal.snapshot(),
            inertia,
            rcs_propellant_q12: next.rcs.propellant_q12(),
            events,
            mach,
            dynamic_pressure_q16,
            angle_of_attack_sine_q16,
        };
        *self = next;
        Ok(snapshot)
    }
}

fn stage_aero_table(index: u8) -> Option<AeroTable<'static>> {
    match index {
        0 => Some(AeroTable::new(
            &generated::STAGE0_AERO_MACH_Q16,
            &generated::STAGE0_AERO_CD_Q14,
        )),
        1 => Some(AeroTable::new(
            &generated::STAGE1_AERO_MACH_Q16,
            &generated::STAGE1_AERO_CD_Q14,
        )),
        _ => None,
    }
}
fn validate_base_scenario(scenario: &Phase2Scenario) -> bool {
    if scenario.scenario_id() != KSA2A_NOMINAL_SCENARIO_ID || scenario.stage_count() != 2 {
        return false;
    }
    let mut index = 0u8;
    while index < PHASE5_STAGE_COUNT as u8 {
        let Some(base) = scenario.stage(index) else {
            return false;
        };
        let i = index as usize;
        if base.dry_mass().raw() != generated::STAGE_DRY_MASS_Q12[i]
            || base.propellant_mass().raw() != generated::STAGE_PROPELLANT_MASS_Q12[i]
            || base.thrust().raw() != generated::STAGE_THRUST_Q12[i]
            || base.mass_flow().raw() != generated::STAGE_MASS_FLOW_Q16[i]
            || base.burn_steps() != generated::STAGE_BURN_MISSION_STEPS[i]
            || base.separation_delay_steps() != generated::STAGE_SEPARATION_DELAY_STEPS[i]
            || base.ignition_delay_steps() != generated::STAGE_IGNITION_DELAY_STEPS[i]
            || base.reference_area().raw() != generated::STAGE_AREA_Q16[i]
        {
            return false;
        }
        index += 1;
    }
    true
}
