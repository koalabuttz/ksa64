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
    fn advance(&mut self, request: i32, lag_steps: u8, slew_q16: i32) {
        self.requested = request.clamp(-generated::GIMBAL_LIMIT_Q16, generated::GIMBAL_LIMIT_Q16);
        let error = self.requested - self.lagged;
        self.lagged += error / lag_steps as i32;
        if !self.jammed {
            let delta = (self.lagged - self.applied).clamp(-slew_q16, slew_q16);
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
        self.advance_parameterized(
            command,
            generated::GIMBAL_LAG_STEPS,
            generated::GIMBAL_SLEW_PER_FAST_STEP_Q16,
        )
    }

    pub fn advance_parameterized(
        &mut self,
        command: GimbalCommandQ16,
        lag_steps: u8,
        slew_q16: i32,
    ) -> GimbalSnapshotQ16 {
        self.pitch.advance(command.pitch, lag_steps, slew_q16);
        self.yaw.advance(command.yaw, lag_steps, slew_q16);
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

fn scale_ppm(value: i32, delta_ppm: i32, status: &mut NumericStatus) -> i32 {
    if delta_ppm == 0 {
        return value;
    }
    let scaled = (value as i64 * (1_000_000i64 + delta_ppm as i64)) / 1_000_000i64;
    if scaled < i32::MIN as i64 || scaled > i32::MAX as i64 {
        status.record(NumericFault::Saturation);
        if scaled < 0 {
            i32::MIN
        } else {
            i32::MAX
        }
    } else {
        scaled as i32
    }
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
pub struct Phase5VehicleParameters {
    pub payload_mass_ppm: i32,
    pub stage_thrust_ppm: [i32; 2],
    pub atmosphere_density_ppm: i32,
    pub aerodynamic_scale_ppm: i32,
    pub gimbal_lag_steps: u8,
    pub gimbal_slew_ppm: i32,
}
impl Phase5VehicleParameters {
    pub const DEFAULT: Self = Self {
        payload_mass_ppm: 0,
        stage_thrust_ppm: [0; 2],
        atmosphere_density_ppm: 0,
        aerodynamic_scale_ppm: 0,
        gimbal_lag_steps: 4,
        gimbal_slew_ppm: 0,
    };
    pub const fn is_valid(self) -> bool {
        self.payload_mass_ppm >= -100_000
            && self.payload_mass_ppm <= 100_000
            && self.stage_thrust_ppm[0] >= -100_000
            && self.stage_thrust_ppm[0] <= 100_000
            && self.stage_thrust_ppm[1] >= -100_000
            && self.stage_thrust_ppm[1] <= 100_000
            && self.atmosphere_density_ppm >= -250_000
            && self.atmosphere_density_ppm <= 250_000
            && self.aerodynamic_scale_ppm >= -250_000
            && self.aerodynamic_scale_ppm <= 250_000
            && self.gimbal_lag_steps >= 1
            && self.gimbal_lag_steps <= 16
            && self.gimbal_slew_ppm >= -500_000
            && self.gimbal_slew_ppm <= 500_000
    }
}
impl Default for Phase5VehicleParameters {
    fn default() -> Self {
        Self::DEFAULT
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
    /// Four body-specific-force samples at the 32 Hz fast cadence.
    pub imu_accel_body_q28: [[i32; 3]; PHASE5_SUBSTEPS as usize],
    /// Four rigid-plus-flex body-rate samples at the 32 Hz fast cadence.
    pub imu_gyro_body_q24: [[i32; 3]; PHASE5_SUBSTEPS as usize],
}

#[derive(Clone, Copy)]
struct FastStepObservation {
    mach: Mach,
    dynamic_pressure_q16: i32,
    angle_of_attack_sine_q16: i32,
    accel_body_q28: [i32; 3],
    gyro_body_q24: [i32; 3],
}

#[derive(Clone, Copy)]
pub struct Phase5VehicleMachine {
    truth: Phase5VehicleTruth,
    gimbal: TwoAxisGimbalQ16,
    rcs: RcsSystem,
    engine_residual_q24: i32,
    burn_fast_steps: u32,
    phase_steps: u32,
    disturbance_body_q12: ForceVec,
    damping_scale_q16: i32,
    parameters: Phase5VehicleParameters,
}

impl Phase5VehicleMachine {
    pub fn new_ksa5a_checked(scenario: &Phase2Scenario) -> Result<Self, Phase5VehicleError> {
        if !validate_base_scenario(scenario) {
            return Err(Phase5VehicleError::Configuration);
        }
        Self::new_ksa5a()
    }

    pub fn new_ksa5a() -> Result<Self, Phase5VehicleError> {
        Self::new_ksa5a_parameterized(Phase5VehicleParameters::DEFAULT)
    }

    pub fn new_ksa5a_parameterized(
        parameters: Phase5VehicleParameters,
    ) -> Result<Self, Phase5VehicleError> {
        if !parameters.is_valid() {
            return Err(Phase5VehicleError::Configuration);
        }
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
                total_mass_q12: generated::INITIAL_TOTAL_MASS_Q12.saturating_add(
                    scale_ppm(
                        ksa64_core::phase5_contract::PHASE5_PAYLOAD_MASS_Q12,
                        parameters.payload_mass_ppm,
                        &mut status,
                    )
                    .saturating_sub(ksa64_core::phase5_contract::PHASE5_PAYLOAD_MASS_Q12),
                ),
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
            disturbance_body_q12: ForceVec::ZERO,
            damping_scale_q16: 65_536,
            parameters,
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

    /// Produces a non-advancing observation for the initial sensor frame.
    /// It has no events or aerodynamic extrema and cannot mutate physics.
    pub fn current_snapshot(&self) -> Result<Phase5VehicleSnapshot, Phase5VehicleError> {
        let mut status = NumericStatus::CLEAR;
        let inertia = self
            .stage()?
            .inertia
            .interpolate(self.truth.active_propellant_q12, &mut status);
        if !status.is_clear() {
            return Err(Phase5VehicleError::NumericFault);
        }
        Ok(Phase5VehicleSnapshot {
            truth: self.truth,
            gimbal: self.gimbal.snapshot(),
            inertia,
            rcs_propellant_q12: self.rcs.propellant_q12(),
            events: 0,
            mach: Mach::from_raw(0),
            dynamic_pressure_q16: 0,
            angle_of_attack_sine_q16: 0,
            imu_accel_body_q28: [[0; 3]; PHASE5_SUBSTEPS as usize],
            imu_gyro_body_q24: [[0; 3]; PHASE5_SUBSTEPS as usize],
        })
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

    pub fn set_disturbance_body_q12(&mut self, force: ForceVec) {
        self.disturbance_body_q12 = force;
    }

    pub fn set_damping_scale_q16(&mut self, scale_q16: i32) {
        self.damping_scale_q16 = scale_q16.clamp(0, 65_536);
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
    ) -> Result<FastStepObservation, Phase5VehicleError> {
        let mut stage = self.stage()?;
        stage.thrust_q12 = scale_ppm(
            stage.thrust_q12,
            self.parameters.stage_thrust_ppm[self.truth.active_stage as usize],
            status,
        );
        let slew_q16 = scale_ppm(
            generated::GIMBAL_SLEW_PER_FAST_STEP_Q16,
            self.parameters.gimbal_slew_ppm,
            status,
        );
        let gimbal = self.gimbal.advance_parameterized(
            command.gimbal,
            self.parameters.gimbal_lag_steps,
            slew_q16,
        );
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
        let combined_aero_ppm = match (
            self.parameters.atmosphere_density_ppm,
            self.parameters.aerodynamic_scale_ppm,
        ) {
            (0, aerodynamic) => aerodynamic,
            (density, 0) => density,
            (density, aerodynamic) => {
                (((1_000_000i64 + density as i64) * (1_000_000i64 + aerodynamic as i64))
                    / 1_000_000i64
                    - 1_000_000i64) as i32
            }
        };
        let (aero_force_eci, aero_torque_body) = if combined_aero_ppm == 0 {
            (aero.force_eci(), aero.torque_body())
        } else {
            (
                ForceVec::new(
                    scale_ppm(aero.force_eci().x(), combined_aero_ppm, status),
                    scale_ppm(aero.force_eci().y(), combined_aero_ppm, status),
                    scale_ppm(aero.force_eci().z(), combined_aero_ppm, status),
                ),
                TorqueVec::new(
                    scale_ppm(aero.torque_body().x(), combined_aero_ppm, status),
                    scale_ppm(aero.torque_body().y(), combined_aero_ppm, status),
                    scale_ppm(aero.torque_body().z(), combined_aero_ppm, status),
                ),
            )
        };
        let dynamic_pressure_q16 = scale_ppm(
            aero.dynamic_pressure().raw(),
            self.parameters.atmosphere_density_ppm,
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
        let disturbance_eci = self
            .truth
            .rigid
            .attitude()
            .rotate(self.disturbance_body_q12, status);
        let total_force_eci = aero_force_eci
            .checked_add(engine_eci, status)
            .checked_add(disturbance_eci, status);
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
        let effective_rate_damping =
            multiply_scaled(stage.rate_damping_q16, self.damping_scale_q16, 16, status);
        let damping = rate_damping_torque(
            self.truth.rigid.angular_rate(),
            effective_rate_damping,
            status,
        );
        let total_torque = aero_torque_body
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
            .rotate(aero_force_eci, status);
        let body_force = aero_body
            .checked_add(engine_body, status)
            .checked_add(self.disturbance_body_q12, status);
        let imu_accel_body_q28 = [
            divide_scaled(body_force.x(), self.truth.total_mass_q12, 28, status),
            divide_scaled(body_force.y(), self.truth.total_mass_q12, 28, status),
            divide_scaled(body_force.z(), self.truth.total_mass_q12, 28, status),
        ];
        let lateral_y_km_q24 = divide_scaled(body_force.y(), self.truth.total_mass_q12, 24, status);
        let lateral_z_km_q24 = divide_scaled(body_force.z(), self.truth.total_mass_q12, 24, status);
        let lateral_y_ms_q24 = multiply_scaled(lateral_y_km_q24, 1_000, 0, status);
        let lateral_z_ms_q24 = multiply_scaled(lateral_z_km_q24, 1_000, 0, status);
        let bend = stage.flexible.bending();
        let slosh = stage.flexible.slosh();
        let flexible_parameters = FlexibleParametersQ16::new(
            ModalParametersQ16::new(
                bend.natural_frequency(),
                multiply_scaled(bend.damping_ratio(), self.damping_scale_q16, 16, status),
                bend.drive_gain(),
            ),
            ModalParametersQ16::new(
                slosh.natural_frequency(),
                multiply_scaled(slosh.damping_ratio(), self.damping_scale_q16, 16, status),
                slosh.drive_gain(),
            ),
        );
        self.truth.flexible = step_flexible_modes(
            self.truth.flexible,
            flexible_parameters,
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
        let rigid_rate = self.truth.rigid.angular_rate();
        let imu_gyro_body_q24 = [
            rigid_rate.x(),
            add(
                rigid_rate.y(),
                add(
                    self.truth.flexible.y().bending().rate(),
                    self.truth.flexible.y().slosh().rate(),
                    status,
                ),
                status,
            ),
            add(
                rigid_rate.z(),
                add(
                    self.truth.flexible.z().bending().rate(),
                    self.truth.flexible.z().slosh().rate(),
                    status,
                ),
                status,
            ),
        ];
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
        Ok(FastStepObservation {
            mach: aero.mach(),
            dynamic_pressure_q16,
            angle_of_attack_sine_q16: aero.angle_of_attack_sine_q16(),
            accel_body_q28: imu_accel_body_q28,
            gyro_body_q24: imu_gyro_body_q24,
        })
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
        let mut imu_accel_body_q28 = [[0; 3]; PHASE5_SUBSTEPS as usize];
        let mut imu_gyro_body_q24 = [[0; 3]; PHASE5_SUBSTEPS as usize];
        let mut substep = 0;
        while substep < generated::SUBSTEPS {
            let fast = next.fast_step(command, &mut events, &mut status)?;
            mach = fast.mach;
            dynamic_pressure_q16 = fast.dynamic_pressure_q16;
            angle_of_attack_sine_q16 = fast.angle_of_attack_sine_q16;
            imu_accel_body_q28[substep as usize] = fast.accel_body_q28;
            imu_gyro_body_q24[substep as usize] = fast.gyro_body_q24;
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
            imu_accel_body_q28,
            imu_gyro_body_q24,
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

/// Additive Phase 6 view of the already accepted four-substep vehicle update.
/// Holding one command for all four calls is required to equal Phase 5 step().
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase6FastObservation {
    pub substep: u8,
    pub accel_body_q28: [i32; 3],
    pub gyro_body_q24: [i32; 3],
    pub gimbal: GimbalSnapshotQ16,
    pub events: u16,
}

pub struct Phase6FastVehicle {
    committed: Phase5VehicleMachine,
    working: Phase5VehicleMachine,
    active: bool,
    substep: u8,
    events: u16,
    phase_during_step: Phase5StagePhase,
    mach: Mach,
    dynamic_pressure_q16: i32,
    angle_of_attack_sine_q16: i32,
    accel: [[i32; 3]; PHASE5_SUBSTEPS as usize],
    gyro: [[i32; 3]; PHASE5_SUBSTEPS as usize],
}
impl Phase6FastVehicle {
    pub fn new(machine: Phase5VehicleMachine) -> Self {
        Self {
            committed: machine,
            working: machine,
            active: false,
            substep: 0,
            events: 0,
            phase_during_step: machine.truth.phase,
            mach: Mach::from_raw(0),
            dynamic_pressure_q16: 0,
            angle_of_attack_sine_q16: 0,
            accel: [[0; 3]; PHASE5_SUBSTEPS as usize],
            gyro: [[0; 3]; PHASE5_SUBSTEPS as usize],
        }
    }
    pub const fn committed_machine(&self) -> Phase5VehicleMachine {
        self.committed
    }
    pub fn current_snapshot(&self) -> Result<Phase5VehicleSnapshot, Phase5VehicleError> {
        self.committed.current_snapshot()
    }
    pub fn begin(&mut self, boundary: Phase5VehicleCommand) -> Result<(), Phase5VehicleError> {
        if self.active {
            return Err(Phase5VehicleError::Configuration);
        }
        if self.committed.truth.step >= ksa64_core::phase5_contract::PHASE5_MISSION_STEPS {
            return Err(Phase5VehicleError::Complete);
        }
        self.working = self.committed;
        self.events = 0;
        self.substep = 0;
        self.accel = [[0; 3]; PHASE5_SUBSTEPS as usize];
        self.gyro = [[0; 3]; PHASE5_SUBSTEPS as usize];
        let mut status = NumericStatus::CLEAR;
        self.working
            .apply_boundary_command(boundary, &mut self.events, &mut status)?;
        if !status.is_clear() {
            return Err(Phase5VehicleError::NumericFault);
        }
        self.phase_during_step = self.working.truth.phase;
        self.active = true;
        Ok(())
    }
    pub fn advance(
        &mut self,
        command: Phase5VehicleCommand,
    ) -> Result<(Phase6FastObservation, Option<Phase5VehicleSnapshot>), Phase5VehicleError> {
        if !self.active || self.substep >= PHASE5_SUBSTEPS {
            return Err(Phase5VehicleError::Configuration);
        }
        let mut status = NumericStatus::CLEAR;
        let fast = self
            .working
            .fast_step(command, &mut self.events, &mut status)?;
        if !status.is_clear() {
            return Err(Phase5VehicleError::NumericFault);
        }
        let at = self.substep as usize;
        self.accel[at] = fast.accel_body_q28;
        self.gyro[at] = fast.gyro_body_q24;
        self.mach = fast.mach;
        self.dynamic_pressure_q16 = fast.dynamic_pressure_q16;
        self.angle_of_attack_sine_q16 = fast.angle_of_attack_sine_q16;
        self.substep += 1;
        let observation = Phase6FastObservation {
            substep: self.substep,
            accel_body_q28: fast.accel_body_q28,
            gyro_body_q24: fast.gyro_body_q24,
            gimbal: self.working.gimbal.snapshot(),
            events: self.events,
        };
        if self.substep < PHASE5_SUBSTEPS {
            return Ok((observation, None));
        }
        if self.working.truth.phase == self.phase_during_step
            && matches!(
                self.working.truth.phase,
                Phase5StagePhase::CoastBeforeIgnition | Phase5StagePhase::CoastBeforeSeparation
            )
        {
            self.working.phase_steps += 1
        }
        self.working.truth.step += 1;
        self.working.truth.time_q16 = add(
            self.working.truth.time_q16,
            ksa64_core::phase5_contract::PHASE5_MISSION_STEP_Q16,
            &mut status,
        );
        let stage = self.working.stage()?;
        let inertia = stage
            .inertia
            .interpolate(self.working.truth.active_propellant_q12, &mut status);
        if !status.is_clear() {
            return Err(Phase5VehicleError::NumericFault);
        }
        let snapshot = Phase5VehicleSnapshot {
            truth: self.working.truth,
            gimbal: self.working.gimbal.snapshot(),
            inertia,
            rcs_propellant_q12: self.working.rcs.propellant_q12(),
            events: self.events,
            mach: self.mach,
            dynamic_pressure_q16: self.dynamic_pressure_q16,
            angle_of_attack_sine_q16: self.angle_of_attack_sine_q16,
            imu_accel_body_q28: self.accel,
            imu_gyro_body_q24: self.gyro,
        };
        self.committed = self.working;
        self.active = false;
        Ok((observation, Some(snapshot)))
    }
}
