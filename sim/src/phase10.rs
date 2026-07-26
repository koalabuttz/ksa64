//! Phase 10 deterministic global Earth world authority.

use ksa64_core::numeric::{add, magnitude3_floor, multiply_scaled, subtract, NumericStatus};
use ksa64_core::phase10_contract::{
    EarthModelPack, GlobalSegment, ReferenceFrameId, TransformPack,
};
use ksa64_core::phase10_environment::{
    central_j2_gravity, ecef_rotating_terms, ecef_to_geodetic, AtmosphereSample,
    CompiledAtmospherePack, EnvironmentError, GeodeticState,
};
use ksa64_core::phase10_frames::{
    ecef_to_gcrf, ecef_to_local, gcrf_to_ecef, interpolate_transform, local_to_ecef, FrameError,
    LocalAnchor, LocalKinematicState,
};
use ksa64_core::phase10_geodesy::{
    body_x_attitude, enu_to_ecef_rotation, geodetic_to_ecef, launch_direction_enu,
};
use ksa64_core::phase10_numeric::{
    interpolate_i32, GlobalAccelerationVec, GlobalAngularRateVec, GlobalKinematicState,
    GlobalPositionVec, GlobalVelocityVec, MissionTimeQ16, GLOBAL_AVIONICS_PERIOD_Q16,
    GLOBAL_COAST_STEP_Q16, GLOBAL_POWERED_STEP_Q16,
};
use ksa64_core::phase10_vehicle::{
    GlobalAeroKnot, GlobalMissionPack, GlobalPackError, GlobalVehiclePack, PitchKnot,
};
use ksa64_core::phase8_numeric::{EnuAcceleration, EnuPosition, EnuVelocity};
use ksa64_core::spatial_numeric::{FixedVec3, QuaternionQ30};
use ksa64_interface::phase10::{
    GlobalCommandCell, GlobalFrameId, GLOBAL_COMMAND_DROGUE, GLOBAL_COMMAND_MAIN,
};

pub const EVENT_RAIL_CLEAR: u16 = 0x0001;
pub const EVENT_ECEF_OWNER: u16 = 0x0002;
pub const EVENT_BURNOUT: u16 = 0x0004;
pub const EVENT_ECI_OWNER: u16 = 0x0008;
pub const EVENT_APOGEE: u16 = 0x0010;
pub const EVENT_ENTRY_OWNER: u16 = 0x0020;
pub const EVENT_DROGUE: u16 = 0x0040;
pub const EVENT_RECOVERY_OWNER: u16 = 0x0080;
pub const EVENT_MAIN: u16 = 0x0100;
pub const EVENT_LANDING: u16 = 0x0200;

const GRAVITY_Q19_M_S2: i32 = 5_141_504;
const MAX_DYNAMIC_PRESSURE_Q14: i32 = 100_000 << 14;
const MAX_MACH_Q24: i32 = 10 << 24;
const TRANSITION_CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalWorldError {
    Pack(GlobalPackError),
    Environment(EnvironmentError),
    Frame(FrameError),
    Identity,
    Numeric,
    Envelope,
    Transition,
    Complete,
    Timeout,
}

impl From<GlobalPackError> for GlobalWorldError {
    fn from(value: GlobalPackError) -> Self {
        Self::Pack(value)
    }
}
impl From<EnvironmentError> for GlobalWorldError {
    fn from(value: EnvironmentError) -> Self {
        Self::Environment(value)
    }
}
impl From<FrameError> for GlobalWorldError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct FrameTransitionRecord {
    pub from: ReferenceFrameId,
    pub to: ReferenceFrameId,
    pub time: MissionTimeQ16,
    pub position_delta_raw: i32,
    pub velocity_delta_raw: i32,
    pub attitude_delta_raw: i32,
    pub angular_rate_delta_raw: i32,
    pub checksum: u32,
}

impl FrameTransitionRecord {
    pub const ZERO: Self = Self {
        from: ReferenceFrameId::LocalEnuV1,
        to: ReferenceFrameId::LocalEnuV1,
        time: MissionTimeQ16::ZERO,
        position_delta_raw: 0,
        velocity_delta_raw: 0,
        attitude_delta_raw: 0,
        angular_rate_delta_raw: 0,
        checksum: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Coordinates {
    Local(LocalKinematicState),
    Global(GlobalKinematicState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalWorldSnapshot {
    pub segment: GlobalSegment,
    pub frame: ReferenceFrameId,
    pub state: GlobalKinematicState,
    pub altitude_q12_km: i32,
    pub mach_q24: i32,
    pub dynamic_pressure_q14_pa: i32,
    pub total_mass_q21_kg: i32,
    pub main_propellant_q21_kg: i32,
    pub rcs_propellant_q21_kg: i32,
    pub apogee_q12_km: i32,
    pub events: u16,
    pub transition_count: u8,
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionServiceRecord {
    pub from: GlobalFrameId,
    pub to: GlobalFrameId,
    pub time: MissionTimeQ16,
    pub transform_identity: u32,
    pub rotation_q30: QuaternionQ30,
    pub omega_q24: GlobalAngularRateVec,
    pub translation_q12: GlobalPositionVec,
    pub velocity_bias_q24: GlobalVelocityVec,
    pub before: GlobalKinematicState,
    pub after: GlobalKinematicState,
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameService {
    pub frame: GlobalFrameId,
    pub rotation_q30: QuaternionQ30,
    pub omega_q24: GlobalAngularRateVec,
    pub translation_q12: GlobalPositionVec,
    pub velocity_bias_q24: GlobalVelocityVec,
}

#[derive(Clone, Copy)]
struct ForceSample {
    acceleration: GlobalAccelerationVec,
    ecef_state: GlobalKinematicState,
    atmosphere: AtmosphereSample,
    mach_q24: i32,
    dynamic_pressure_q14_pa: i32,
}

pub struct GlobalWorldMachine<'a> {
    earth: &'a EarthModelPack,
    transforms: &'a TransformPack,
    atmosphere: &'a CompiledAtmospherePack,
    vehicle: &'a GlobalVehiclePack,
    mission: GlobalMissionPack,
    segment: GlobalSegment,
    coordinates: Coordinates,
    launch_anchor: LocalAnchor,
    recovery_anchor: LocalAnchor,
    launch_direction_enu_q30: FixedVec3<30>,
    main_propellant_q21: i32,
    rcs_propellant_q21: i32,
    descending: bool,
    drogue: bool,
    main: bool,
    complete: bool,
    last_altitude_q12: i32,
    apogee_q12: i32,
    mach_q24: i32,
    dynamic_pressure_q14: i32,
    events: u16,
    transitions: [FrameTransitionRecord; TRANSITION_CAPACITY],
    transition_count: u8,
    checksum: u32,
}

impl<'a> GlobalWorldMachine<'a> {
    pub fn new(
        earth: &'a EarthModelPack,
        transforms: &'a TransformPack,
        atmosphere: &'a CompiledAtmospherePack,
        vehicle: &'a GlobalVehiclePack,
        mission: GlobalMissionPack,
    ) -> Result<Self, GlobalWorldError> {
        earth.validate().map_err(|_| GlobalWorldError::Identity)?;
        transforms
            .validate()
            .map_err(|_| GlobalWorldError::Identity)?;
        atmosphere.validate()?;
        vehicle.validate()?;
        mission.validate()?;
        if mission.earth_identity != earth.identity
            || mission.transform_identity != transforms.identity
            || mission.atmosphere_identity != atmosphere.identity
            || mission.vehicle_identity != vehicle.identity
            || transforms.earth_identity != earth.identity
            || atmosphere.earth_identity != earth.identity
        {
            return Err(GlobalWorldError::Identity);
        }
        let launch_geodetic = GeodeticState {
            latitude_q28_rad: mission.launch_latitude_q28_rad,
            longitude_q28_rad: mission.launch_longitude_q28_rad,
            height_q12_km: mission.launch_height_q12_km,
        };
        let recovery_geodetic = GeodeticState {
            latitude_q28_rad: mission.recovery_latitude_q28_rad,
            longitude_q28_rad: mission.recovery_longitude_q28_rad,
            height_q12_km: mission.recovery_height_q12_km,
        };
        let launch_anchor = LocalAnchor {
            identity: mission.identity ^ 0x4c41_554e,
            origin_ecef: geodetic_to_ecef(launch_geodetic)?,
            enu_to_ecef: enu_to_ecef_rotation(
                launch_geodetic.latitude_q28_rad,
                launch_geodetic.longitude_q28_rad,
            )?,
            reference_meridian_q28_rad: mission.launch_longitude_q28_rad,
        }
        .validate()?;
        let recovery_anchor = LocalAnchor {
            identity: mission.identity ^ 0x5245_4356,
            origin_ecef: geodetic_to_ecef(recovery_geodetic)?,
            enu_to_ecef: enu_to_ecef_rotation(
                recovery_geodetic.latitude_q28_rad,
                recovery_geodetic.longitude_q28_rad,
            )?,
            reference_meridian_q28_rad: mission.recovery_longitude_q28_rad,
        }
        .validate()?;
        let direction = launch_direction_enu(
            mission.launch_azimuth_q28_rad,
            mission.launch_elevation_q28_rad,
        )?;
        let attitude = body_x_attitude(direction)?;
        let local = LocalKinematicState {
            position: EnuPosition::ZERO,
            velocity: EnuVelocity::ZERO,
            attitude,
            angular_rate: GlobalAngularRateVec::ZERO,
            time: MissionTimeQ16::ZERO,
        };
        Ok(Self {
            earth,
            transforms,
            atmosphere,
            vehicle,
            mission,
            segment: GlobalSegment::LocalLaunch,
            coordinates: Coordinates::Local(local),
            launch_anchor,
            recovery_anchor,
            launch_direction_enu_q30: direction,
            main_propellant_q21: vehicle.main_propellant_q21_kg,
            rcs_propellant_q21: vehicle.rcs_propellant_q21_kg,
            descending: false,
            drogue: false,
            main: false,
            complete: false,
            last_altitude_q12: mission.launch_height_q12_km,
            apogee_q12: mission.launch_height_q12_km,
            mach_q24: 0,
            dynamic_pressure_q14: 0,
            events: 0,
            transitions: [FrameTransitionRecord::ZERO; TRANSITION_CAPACITY],
            transition_count: 0,
            checksum: 0x811c_9dc5,
        })
    }

    pub const fn transitions(&self) -> &[FrameTransitionRecord; TRANSITION_CAPACITY] {
        &self.transitions
    }

    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn snapshot(&self) -> Result<GlobalWorldSnapshot, GlobalWorldError> {
        let state = self.canonical_state()?;
        Ok(GlobalWorldSnapshot {
            segment: self.segment,
            frame: self.segment.frame(),
            state,
            altitude_q12_km: self.last_altitude_q12,
            mach_q24: self.mach_q24,
            dynamic_pressure_q14_pa: self.dynamic_pressure_q14,
            total_mass_q21_kg: self.total_mass_q21(),
            main_propellant_q21_kg: self.main_propellant_q21,
            rcs_propellant_q21_kg: self.rcs_propellant_q21,
            apogee_q12_km: self.apogee_q12,
            events: self.events,
            transition_count: self.transition_count,
            checksum: self.checksum,
        })
    }

    pub fn step(&mut self) -> Result<GlobalWorldSnapshot, GlobalWorldError> {
        self.step_internal(None)
    }

    pub fn step_commanded(
        &mut self,
        command: &GlobalCommandCell,
    ) -> Result<GlobalWorldSnapshot, GlobalWorldError> {
        if command.frame != interface_frame(self.segment.frame()) {
            return Err(GlobalWorldError::Identity);
        }
        self.step_internal(Some(command))
    }

    fn step_internal(
        &mut self,
        command: Option<&GlobalCommandCell>,
    ) -> Result<GlobalWorldSnapshot, GlobalWorldError> {
        if self.complete {
            return Err(GlobalWorldError::Complete);
        }
        self.events = 0;
        match self.segment {
            GlobalSegment::LocalLaunch => self.step_local_launch()?,
            GlobalSegment::EcefAscent | GlobalSegment::EcefEntry => {
                self.step_global(GLOBAL_POWERED_STEP_Q16)?
            }
            GlobalSegment::EciCoast => self.step_global(GLOBAL_COAST_STEP_Q16)?,
            GlobalSegment::LocalRecovery => self.step_local_recovery()?,
        }
        self.process_events_and_transitions(command)?;
        self.update_checksum()?;
        self.snapshot()
    }

    fn total_mass_q21(&self) -> i32 {
        self.vehicle
            .dry_mass_q21_kg
            .saturating_add(self.main_propellant_q21)
            .saturating_add(self.rcs_propellant_q21)
    }

    fn current_time(&self) -> MissionTimeQ16 {
        match self.coordinates {
            Coordinates::Local(state) => state.time,
            Coordinates::Global(state) => state.time,
        }
    }

    pub fn active_state(&self) -> Result<GlobalKinematicState, GlobalWorldError> {
        match self.coordinates {
            Coordinates::Local(state) => local_active_state(state),
            Coordinates::Global(state) => Ok(state),
        }
    }

    pub fn ecef_state_public(&self) -> Result<GlobalKinematicState, GlobalWorldError> {
        self.ecef_state()
    }

    pub const fn deployment_feedback(&self) -> u16 {
        (self.drogue as u16) | ((self.main as u16) << 1)
    }

    pub fn frame_service(&self) -> Result<FrameService, GlobalWorldError> {
        let zero_position = GlobalPositionVec::ZERO;
        let zero_velocity = GlobalVelocityVec::ZERO;
        match self.segment {
            GlobalSegment::EcefAscent | GlobalSegment::EcefEntry => Ok(FrameService {
                frame: GlobalFrameId::EarthFixedEcefV1,
                rotation_q30: QuaternionQ30::IDENTITY,
                omega_q24: GlobalAngularRateVec::ZERO,
                translation_q12: zero_position,
                velocity_bias_q24: zero_velocity,
            }),
            GlobalSegment::EciCoast => {
                let transform = interpolate_transform(self.transforms, self.current_time())?;
                Ok(FrameService {
                    frame: GlobalFrameId::EarthInertialEciV1,
                    rotation_q30: transform.ecef_to_gcrf,
                    omega_q24: transform.angular_velocity_gcrf,
                    translation_q12: zero_position,
                    velocity_bias_q24: zero_velocity,
                })
            }
            GlobalSegment::LocalLaunch | GlobalSegment::LocalRecovery => {
                let anchor = if self.segment == GlobalSegment::LocalRecovery {
                    self.recovery_anchor
                } else {
                    self.launch_anchor
                };
                let rotation = anchor.enu_to_ecef.conjugate();
                let mut status = NumericStatus::CLEAR;
                let rotated_origin = rotation.rotate(anchor.origin_ecef, &mut status);
                let translation = GlobalPositionVec::ZERO.checked_sub(rotated_origin, &mut status);
                if !status.is_clear() {
                    return Err(GlobalWorldError::Numeric);
                }
                Ok(FrameService {
                    frame: GlobalFrameId::LocalEnuV1,
                    rotation_q30: rotation,
                    omega_q24: GlobalAngularRateVec::ZERO,
                    translation_q12: translation,
                    velocity_bias_q24: zero_velocity,
                })
            }
        }
    }

    pub fn transition_service(
        &self,
        index: usize,
    ) -> Result<TransitionServiceRecord, GlobalWorldError> {
        if index >= self.transition_count as usize || index + 1 != self.transition_count as usize {
            return Err(GlobalWorldError::Transition);
        }
        let record = self.transitions[index];
        if record.time != self.current_time() {
            return Err(GlobalWorldError::Transition);
        }
        let after = self.active_state()?;
        let (before, rotation, omega, identity) = match (record.from, record.to) {
            (ReferenceFrameId::LocalEnuV1, ReferenceFrameId::EarthFixedEcefV1) => {
                let local = ecef_to_local(self.launch_anchor, after)?;
                (
                    local_active_state(local)?,
                    self.launch_anchor.enu_to_ecef,
                    GlobalAngularRateVec::ZERO,
                    self.launch_anchor.identity,
                )
            }
            (ReferenceFrameId::EarthFixedEcefV1, ReferenceFrameId::EarthInertialEciV1) => {
                let transform = interpolate_transform(self.transforms, record.time)?;
                (
                    gcrf_to_ecef(transform, after)?,
                    transform.ecef_to_gcrf,
                    transform.angular_velocity_gcrf,
                    self.transforms.identity,
                )
            }
            (ReferenceFrameId::EarthInertialEciV1, ReferenceFrameId::EarthFixedEcefV1) => {
                let transform = interpolate_transform(self.transforms, record.time)?;
                (
                    ecef_to_gcrf(transform, after)?,
                    transform.ecef_to_gcrf.conjugate(),
                    transform.angular_velocity_gcrf,
                    self.transforms.identity,
                )
            }
            (ReferenceFrameId::EarthFixedEcefV1, ReferenceFrameId::LocalEnuV1) => {
                let local = match self.coordinates {
                    Coordinates::Local(state) => state,
                    _ => return Err(GlobalWorldError::Transition),
                };
                (
                    local_to_ecef(self.recovery_anchor, local)?,
                    self.recovery_anchor.enu_to_ecef.conjugate(),
                    GlobalAngularRateVec::ZERO,
                    self.recovery_anchor.identity,
                )
            }
            _ => return Err(GlobalWorldError::Transition),
        };
        let mut status = NumericStatus::CLEAR;
        let rotated_position = rotation.rotate(before.position, &mut status);
        let translation = after.position.checked_sub(rotated_position, &mut status);
        let velocity_bias = if omega == GlobalAngularRateVec::ZERO {
            let rotated_velocity = rotation.rotate(before.velocity, &mut status);
            after.velocity.checked_sub(rotated_velocity, &mut status)
        } else {
            GlobalVelocityVec::ZERO
        };
        if !status.is_clear() {
            return Err(GlobalWorldError::Numeric);
        }
        Ok(TransitionServiceRecord {
            from: interface_frame(record.from),
            to: interface_frame(record.to),
            time: record.time,
            transform_identity: identity,
            rotation_q30: rotation,
            omega_q24: omega,
            translation_q12: translation,
            velocity_bias_q24: velocity_bias,
            before,
            after,
            checksum: record.checksum,
        })
    }

    fn canonical_state(&self) -> Result<GlobalKinematicState, GlobalWorldError> {
        match self.coordinates {
            Coordinates::Local(state) => {
                let anchor = if self.segment == GlobalSegment::LocalRecovery {
                    self.recovery_anchor
                } else {
                    self.launch_anchor
                };
                Ok(local_to_ecef(anchor, state)?)
            }
            Coordinates::Global(state) => Ok(state),
        }
    }

    fn ecef_state(&self) -> Result<GlobalKinematicState, GlobalWorldError> {
        match self.coordinates {
            Coordinates::Local(_) => self.canonical_state(),
            Coordinates::Global(state)
                if self.segment == GlobalSegment::EcefAscent
                    || self.segment == GlobalSegment::EcefEntry =>
            {
                Ok(state)
            }
            Coordinates::Global(state) => {
                let transform = interpolate_transform(self.transforms, state.time)?;
                Ok(gcrf_to_ecef(transform, state)?)
            }
        }
    }

    fn step_local_launch(&mut self) -> Result<(), GlobalWorldError> {
        let current = match self.coordinates {
            Coordinates::Local(state) => state,
            _ => return Err(GlobalWorldError::Transition),
        };
        let dt = GLOBAL_POWERED_STEP_Q16;
        let altitude_q12 =
            self.mission.launch_height_q12_km + rounded_i64(current.position.z() as i64, 2_000)?;
        let atmosphere = self.atmosphere.sample(altitude_q12)?;
        let (speed_q4, mach_q24, q_q14) = local_air_data(current.velocity, atmosphere)?;
        let mass_q13 = (self.total_mass_q21() + 128) >> 8;
        let thrust_accel_q19 = force_to_acceleration_q19(self.vehicle.thrust_q13_n, mass_q13)?;
        let drag_force_q13 = drag_force_q13(
            q_q14,
            self.vehicle.reference_area_q29_m2,
            29,
            self.aero_knot(mach_q24)?.axial_cd_q24,
        )?;
        let drag_accel_q19 = force_to_acceleration_q19(drag_force_q13, mass_q13)?;
        let mut status = NumericStatus::CLEAR;
        let thrust =
            scale_direction::<19>(self.launch_direction_enu_q30, thrust_accel_q19, &mut status);
        let drag = unit_against_local_velocity(current.velocity, speed_q4, drag_accel_q19)?;
        let gravity = EnuAcceleration::new(0, 0, -GRAVITY_Q19_M_S2);
        let unconstrained = thrust
            .checked_add(drag, &mut status)
            .checked_add(gravity, &mut status);
        let projected =
            dot_mixed_q19_q30(unconstrained, self.launch_direction_enu_q30, &mut status).max(0);
        let acceleration =
            scale_direction::<19>(self.launch_direction_enu_q30, projected, &mut status);
        let delta_v = acceleration.scale::<16>(dt as i32, &mut status);
        let successor_velocity = current.velocity.checked_add(delta_v, &mut status);
        let average_velocity = EnuVelocity::new(
            (current.velocity.x() / 2) + (successor_velocity.x() / 2),
            (current.velocity.y() / 2) + (successor_velocity.y() / 2),
            (current.velocity.z() / 2) + (successor_velocity.z() / 2),
        );
        let position_delta = FixedVec3::<13>::new(
            multiply_scaled(average_velocity.x(), dt as i32, 22, &mut status),
            multiply_scaled(average_velocity.y(), dt as i32, 22, &mut status),
            multiply_scaled(average_velocity.z(), dt as i32, 22, &mut status),
        );
        let time = current.time.checked_add(dt, &mut status);
        if !status.is_clear() {
            return Err(GlobalWorldError::Numeric);
        }
        self.coordinates = Coordinates::Local(LocalKinematicState {
            position: current.position.checked_add(position_delta, &mut status),
            velocity: successor_velocity,
            attitude: current.attitude,
            angular_rate: current.angular_rate,
            time,
        });
        self.consume_main_propellant(dt)?;
        self.mach_q24 = mach_q24;
        self.dynamic_pressure_q14 = q_q14;
        Ok(())
    }

    fn step_global(&mut self, dt: u32) -> Result<(), GlobalWorldError> {
        let current = match self.coordinates {
            Coordinates::Global(state) => state,
            _ => return Err(GlobalWorldError::Transition),
        };
        let first = self.force_sample(current)?;
        let mut status = NumericStatus::CLEAR;
        let half_dt = (dt / 2) as i32;
        let midpoint_velocity = current.velocity.checked_add(
            FixedVec3::<24>::new(
                multiply_scaled(first.acceleration.x(), half_dt, 20, &mut status),
                multiply_scaled(first.acceleration.y(), half_dt, 20, &mut status),
                multiply_scaled(first.acceleration.z(), half_dt, 20, &mut status),
            ),
            &mut status,
        );
        let midpoint_position = current.position.checked_add(
            FixedVec3::<12>::new(
                multiply_scaled(current.velocity.x(), half_dt, 28, &mut status),
                multiply_scaled(current.velocity.y(), half_dt, 28, &mut status),
                multiply_scaled(current.velocity.z(), half_dt, 28, &mut status),
            ),
            &mut status,
        );
        let midpoint_time = current.time.checked_add(dt / 2, &mut status);
        let midpoint_mass = self.midpoint_mass(dt)?;
        let midpoint = GlobalKinematicState::new(
            midpoint_position,
            midpoint_velocity,
            current.attitude,
            current.angular_rate,
            midpoint_time,
        );
        let second = self.force_sample_with_mass(midpoint, midpoint_mass)?;
        let successor_velocity = current.velocity.checked_add(
            FixedVec3::<24>::new(
                multiply_scaled(second.acceleration.x(), dt as i32, 20, &mut status),
                multiply_scaled(second.acceleration.y(), dt as i32, 20, &mut status),
                multiply_scaled(second.acceleration.z(), dt as i32, 20, &mut status),
            ),
            &mut status,
        );
        let successor_position = current.position.checked_add(
            FixedVec3::<12>::new(
                multiply_scaled(midpoint_velocity.x(), dt as i32, 28, &mut status),
                multiply_scaled(midpoint_velocity.y(), dt as i32, 28, &mut status),
                multiply_scaled(midpoint_velocity.z(), dt as i32, 28, &mut status),
            ),
            &mut status,
        );
        let successor_time = current.time.checked_add(dt, &mut status);
        if !status.is_clear() {
            return Err(GlobalWorldError::Numeric);
        }
        let attitude = self.commanded_attitude(successor_time, successor_position)?;
        self.coordinates = Coordinates::Global(GlobalKinematicState::new(
            successor_position,
            successor_velocity,
            attitude,
            current.angular_rate,
            successor_time,
        ));
        self.consume_main_propellant(dt)?;
        self.mach_q24 = second.mach_q24;
        self.dynamic_pressure_q14 = second.dynamic_pressure_q14_pa;
        let _ = first.ecef_state;
        let _ = first.atmosphere;
        Ok(())
    }

    fn step_local_recovery(&mut self) -> Result<(), GlobalWorldError> {
        let current = match self.coordinates {
            Coordinates::Local(state) => state,
            _ => return Err(GlobalWorldError::Transition),
        };
        let dt = GLOBAL_COAST_STEP_Q16;
        let altitude_q12 =
            self.mission.recovery_height_q12_km + rounded_i64(current.position.z() as i64, 2_000)?;
        let atmosphere = self.atmosphere.sample(altitude_q12.max(-1 << 12))?;
        let (speed_q4, mach, dynamic_q) = local_air_data(current.velocity, atmosphere)?;
        let cda_q24 = if self.main {
            self.vehicle.main_cda_q24_m2
        } else {
            self.vehicle.drogue_cda_q24_m2
        };
        let drag_force = drag_force_q13(dynamic_q, cda_q24, 24, 1 << 24)?;
        let mass_q13 = (self.total_mass_q21() + 128) >> 8;
        let drag_accel = force_to_acceleration_q19(drag_force, mass_q13)?;
        let drag = unit_against_local_velocity(current.velocity, speed_q4, drag_accel)?;
        let mut status = NumericStatus::CLEAR;
        let acceleration =
            drag.checked_add(EnuAcceleration::new(0, 0, -GRAVITY_Q19_M_S2), &mut status);
        let delta_v = acceleration.scale::<16>(dt as i32, &mut status);
        let successor_velocity = current.velocity.checked_add(delta_v, &mut status);
        let average_velocity = EnuVelocity::new(
            (current.velocity.x() / 2) + (successor_velocity.x() / 2),
            (current.velocity.y() / 2) + (successor_velocity.y() / 2),
            (current.velocity.z() / 2) + (successor_velocity.z() / 2),
        );
        let position_delta = EnuPosition::new(
            multiply_scaled(average_velocity.x(), dt as i32, 22, &mut status),
            multiply_scaled(average_velocity.y(), dt as i32, 22, &mut status),
            multiply_scaled(average_velocity.z(), dt as i32, 22, &mut status),
        );
        let successor = LocalKinematicState {
            position: current.position.checked_add(position_delta, &mut status),
            velocity: successor_velocity,
            attitude: current.attitude,
            angular_rate: current.angular_rate,
            time: current.time.checked_add(dt, &mut status),
        };
        if !status.is_clear() {
            return Err(GlobalWorldError::Numeric);
        }
        self.coordinates = Coordinates::Local(successor);
        self.mach_q24 = mach;
        self.dynamic_pressure_q14 = dynamic_q;
        Ok(())
    }

    fn force_sample(&self, state: GlobalKinematicState) -> Result<ForceSample, GlobalWorldError> {
        self.force_sample_with_mass(state, self.total_mass_q21())
    }

    fn force_sample_with_mass(
        &self,
        state: GlobalKinematicState,
        mass_q21: i32,
    ) -> Result<ForceSample, GlobalWorldError> {
        let transform = interpolate_transform(self.transforms, state.time)?;
        let ecef_state = if self.segment == GlobalSegment::EciCoast {
            gcrf_to_ecef(transform, state)?
        } else {
            state
        };
        let geodetic = ecef_to_geodetic(ecef_state.position)?;
        let atmosphere = self.atmosphere.sample(geodetic.height_q12_km)?;
        let wind = self.wind_ecef(geodetic, atmosphere)?;
        let mut status = NumericStatus::CLEAR;
        let air_velocity = ecef_state.velocity.checked_sub(wind, &mut status);
        if !status.is_clear() {
            return Err(GlobalWorldError::Numeric);
        }
        let (speed_q4, mach_q24, dynamic_q14) = global_air_data(air_velocity, atmosphere)?;
        if dynamic_q14 > MAX_DYNAMIC_PRESSURE_Q14 || mach_q24 > MAX_MACH_Q24 {
            return Err(GlobalWorldError::Envelope);
        }
        let cda_and_bits = if self.drogue {
            if self.main {
                (self.vehicle.main_cda_q24_m2, 24)
            } else {
                (self.vehicle.drogue_cda_q24_m2, 24)
            }
        } else {
            let area = if self.segment == GlobalSegment::EcefEntry && self.descending {
                multiply_scaled(
                    self.vehicle.reference_area_q29_m2,
                    self.mission.entry_drag_area_scale_q16,
                    16,
                    &mut status,
                )
            } else {
                self.vehicle.reference_area_q29_m2
            };
            (area, 29)
        };
        let cd = if self.drogue {
            1 << 24
        } else {
            self.aero_knot(mach_q24)?.axial_cd_q24
        };
        let drag_force = drag_force_q13(dynamic_q14, cda_and_bits.0, cda_and_bits.1, cd)?;
        let drag_acceleration =
            force_to_global_acceleration(drag_force, mass_q21, air_velocity, speed_q4)?;
        let gravity_ecef = central_j2_gravity(self.earth, ecef_state.position)?;
        let mut acceleration_ecef = gravity_ecef.checked_add(drag_acceleration, &mut status);
        if self.segment != GlobalSegment::EciCoast {
            acceleration_ecef = acceleration_ecef.checked_add(
                ecef_rotating_terms(
                    ecef_state.position,
                    ecef_state.velocity,
                    GlobalAccelerationVec::ZERO,
                )?,
                &mut status,
            );
        }
        if state.time.raw() < self.vehicle.burn_time_q16_s && self.main_propellant_q21 > 0 {
            let direction = self.commanded_direction_ecef(state.time)?;
            let thrust_accel = force_magnitude_to_global_q28(self.vehicle.thrust_q13_n, mass_q21)?;
            let thrust = scale_direction::<28>(direction, thrust_accel, &mut status);
            acceleration_ecef = acceleration_ecef.checked_add(thrust, &mut status);
        }
        let acceleration = if self.segment == GlobalSegment::EciCoast {
            transform
                .ecef_to_gcrf
                .rotate(acceleration_ecef, &mut status)
        } else {
            acceleration_ecef
        };
        if !status.is_clear() {
            return Err(GlobalWorldError::Numeric);
        }
        Ok(ForceSample {
            acceleration,
            ecef_state,
            atmosphere,
            mach_q24,
            dynamic_pressure_q14_pa: dynamic_q14,
        })
    }

    fn wind_ecef(
        &self,
        geodetic: GeodeticState,
        atmosphere: AtmosphereSample,
    ) -> Result<GlobalVelocityVec, GlobalWorldError> {
        let mut status = NumericStatus::CLEAR;
        let wind_global = GlobalVelocityVec::new(
            rounded_i64(atmosphere.wind_enu_q19_m_s.x() as i64 * 4, 125)?,
            rounded_i64(atmosphere.wind_enu_q19_m_s.y() as i64 * 4, 125)?,
            rounded_i64(atmosphere.wind_enu_q19_m_s.z() as i64 * 4, 125)?,
        );
        let rotation = enu_to_ecef_rotation(geodetic.latitude_q28_rad, geodetic.longitude_q28_rad)?;
        let result = rotation.rotate(wind_global, &mut status);
        if status.is_clear() {
            Ok(result)
        } else {
            Err(GlobalWorldError::Numeric)
        }
    }

    fn aero_knot(&self, mach_q24: i32) -> Result<GlobalAeroKnot, GlobalWorldError> {
        let active = &self.vehicle.aero[..self.vehicle.aero_count as usize];
        if mach_q24 <= active[0].mach_q24 {
            return Ok(active[0]);
        }
        if mach_q24 > active[active.len() - 1].mach_q24 {
            return Err(GlobalWorldError::Envelope);
        }
        let mut upper = 1;
        while mach_q24 > active[upper].mach_q24 {
            upper += 1;
        }
        let lo = active[upper - 1];
        let hi = active[upper];
        let numerator = (mach_q24 - lo.mach_q24) as u32;
        let denominator = (hi.mach_q24 - lo.mach_q24) as u32;
        let mut status = NumericStatus::CLEAR;
        let knot = GlobalAeroKnot {
            mach_q24,
            axial_cd_q24: interpolate_i32(
                lo.axial_cd_q24,
                hi.axial_cd_q24,
                numerator,
                denominator,
                &mut status,
            ),
            cp_from_nose_q28_m: interpolate_i32(
                lo.cp_from_nose_q28_m,
                hi.cp_from_nose_q28_m,
                numerator,
                denominator,
                &mut status,
            ),
            normal_force_slope_q24: interpolate_i32(
                lo.normal_force_slope_q24,
                hi.normal_force_slope_q24,
                numerator,
                denominator,
                &mut status,
            ),
            pitch_damping_q24: interpolate_i32(
                lo.pitch_damping_q24,
                hi.pitch_damping_q24,
                numerator,
                denominator,
                &mut status,
            ),
            yaw_damping_q24: interpolate_i32(
                lo.yaw_damping_q24,
                hi.yaw_damping_q24,
                numerator,
                denominator,
                &mut status,
            ),
        };
        if status.is_clear() {
            Ok(knot)
        } else {
            Err(GlobalWorldError::Numeric)
        }
    }

    fn pitch_at(&self, time: MissionTimeQ16) -> Result<i32, GlobalWorldError> {
        let active = &self.mission.pitch[..self.mission.pitch_count as usize];
        if time.raw() <= active[0].time_q16_s {
            return Ok(active[0].elevation_q28_rad);
        }
        if time.raw() >= active[active.len() - 1].time_q16_s {
            return Ok(active[active.len() - 1].elevation_q28_rad);
        }
        let mut upper = 1;
        while time.raw() > active[upper].time_q16_s {
            upper += 1;
        }
        interpolate_pitch(active[upper - 1], active[upper], time)
    }

    fn commanded_direction_ecef(
        &self,
        time: MissionTimeQ16,
    ) -> Result<FixedVec3<30>, GlobalWorldError> {
        let local =
            launch_direction_enu(self.mission.launch_azimuth_q28_rad, self.pitch_at(time)?)?;
        let mut status = NumericStatus::CLEAR;
        let direction = self.launch_anchor.enu_to_ecef.rotate(local, &mut status);
        if status.is_clear() {
            Ok(direction)
        } else {
            Err(GlobalWorldError::Numeric)
        }
    }

    fn commanded_attitude(
        &self,
        time: MissionTimeQ16,
        _position: GlobalPositionVec,
    ) -> Result<QuaternionQ30, GlobalWorldError> {
        let direction_ecef = self.commanded_direction_ecef(time)?;
        if self.segment == GlobalSegment::EciCoast {
            let transform = interpolate_transform(self.transforms, time)?;
            let mut status = NumericStatus::CLEAR;
            let inertial_direction = transform.ecef_to_gcrf.rotate(direction_ecef, &mut status);
            if !status.is_clear() {
                return Err(GlobalWorldError::Numeric);
            }
            Ok(body_x_attitude(inertial_direction)?)
        } else {
            Ok(body_x_attitude(direction_ecef)?)
        }
    }

    fn midpoint_mass(&self, dt: u32) -> Result<i32, GlobalWorldError> {
        if self.current_time().raw() >= self.vehicle.burn_time_q16_s
            || self.main_propellant_q21 == 0
        {
            return Ok(self.total_mass_q21());
        }
        let mut status = NumericStatus::CLEAR;
        let consumed = multiply_scaled(
            self.vehicle.main_mass_flow_q21_kg_s,
            (dt / 2) as i32,
            16,
            &mut status,
        )
        .min(self.main_propellant_q21);
        if status.is_clear() {
            Ok(self.total_mass_q21() - consumed)
        } else {
            Err(GlobalWorldError::Numeric)
        }
    }

    fn consume_main_propellant(&mut self, dt: u32) -> Result<(), GlobalWorldError> {
        let time = self.current_time();
        let previous = time.raw().saturating_sub(dt);
        if previous >= self.vehicle.burn_time_q16_s || self.main_propellant_q21 == 0 {
            return Ok(());
        }
        let active_dt = dt.min(self.vehicle.burn_time_q16_s - previous);
        let mut status = NumericStatus::CLEAR;
        let consumed = multiply_scaled(
            self.vehicle.main_mass_flow_q21_kg_s,
            active_dt as i32,
            16,
            &mut status,
        )
        .min(self.main_propellant_q21);
        self.main_propellant_q21 = subtract(self.main_propellant_q21, consumed, &mut status);
        if previous < self.vehicle.burn_time_q16_s && time.raw() >= self.vehicle.burn_time_q16_s {
            self.events |= EVENT_BURNOUT;
        }
        if status.is_clear() {
            Ok(())
        } else {
            Err(GlobalWorldError::Numeric)
        }
    }

    fn process_events_and_transitions(
        &mut self,
        command: Option<&GlobalCommandCell>,
    ) -> Result<(), GlobalWorldError> {
        let time = self.current_time();
        let ecef = self.ecef_state()?;
        let geodetic = ecef_to_geodetic(ecef.position)?;
        let altitude = geodetic.height_q12_km;
        if altitude > self.apogee_q12 {
            self.apogee_q12 = altitude;
        } else if !self.descending
            && time.raw() > self.vehicle.burn_time_q16_s
            && altitude < self.last_altitude_q12
        {
            self.descending = true;
            self.events |= EVENT_APOGEE;
            if command.is_none() {
                self.drogue = true;
                self.events |= EVENT_DROGUE;
            }
        }
        if let Some(value) = command {
            if !self.drogue && value.discrete & GLOBAL_COMMAND_DROGUE != 0 {
                self.drogue = true;
                self.events |= EVENT_DROGUE;
            }
            if self.drogue && !self.main && value.discrete & GLOBAL_COMMAND_MAIN != 0 {
                self.main = true;
                self.events |= EVENT_MAIN;
            }
        } else if self.descending
            && !self.main
            && altitude <= self.mission.main_deployment_altitude_q12_km
        {
            self.main = true;
            self.events |= EVENT_MAIN;
        }
        self.last_altitude_q12 = altitude;
        let release = time.raw() & (GLOBAL_AVIONICS_PERIOD_Q16 - 1) == 0;
        if release {
            match self.segment {
                GlobalSegment::LocalLaunch => {
                    let local = match self.coordinates {
                        Coordinates::Local(state) => state,
                        _ => return Err(GlobalWorldError::Transition),
                    };
                    let mut status = NumericStatus::CLEAR;
                    let distance_q13 = dot_mixed_q13_q30(
                        local.position,
                        self.launch_direction_enu_q30,
                        &mut status,
                    );
                    if !status.is_clear() {
                        return Err(GlobalWorldError::Numeric);
                    }
                    if distance_q13 >= self.mission.rail_length_q13_m {
                        self.events |= EVENT_RAIL_CLEAR | EVENT_ECEF_OWNER;
                        let global = local_to_ecef(self.launch_anchor, local)?;
                        self.record_transition(
                            ReferenceFrameId::LocalEnuV1,
                            ReferenceFrameId::EarthFixedEcefV1,
                            global,
                            global,
                        )?;
                        self.segment = GlobalSegment::EcefAscent;
                        self.coordinates = Coordinates::Global(global);
                    }
                }
                GlobalSegment::EcefAscent
                    if altitude > self.mission.eci_transition_altitude_q12_km
                        && self.dynamic_pressure_q14
                            < self.mission.transition_dynamic_pressure_q14_pa =>
                {
                    let current = match self.coordinates {
                        Coordinates::Global(state) => state,
                        _ => return Err(GlobalWorldError::Transition),
                    };
                    let transform = interpolate_transform(self.transforms, time)?;
                    let inertial = ecef_to_gcrf(transform, current)?;
                    let round_trip = gcrf_to_ecef(transform, inertial)?;
                    self.record_transition(
                        ReferenceFrameId::EarthFixedEcefV1,
                        ReferenceFrameId::EarthInertialEciV1,
                        current,
                        round_trip,
                    )?;
                    self.segment = GlobalSegment::EciCoast;
                    self.coordinates = Coordinates::Global(inertial);
                    self.events |= EVENT_ECI_OWNER;
                }
                GlobalSegment::EciCoast
                    if self.descending
                        && altitude <= self.mission.entry_transition_altitude_q12_km =>
                {
                    let inertial = match self.coordinates {
                        Coordinates::Global(state) => state,
                        _ => return Err(GlobalWorldError::Transition),
                    };
                    let transform = interpolate_transform(self.transforms, time)?;
                    let fixed = gcrf_to_ecef(transform, inertial)?;
                    let round_trip = ecef_to_gcrf(transform, fixed)?;
                    self.record_transition(
                        ReferenceFrameId::EarthInertialEciV1,
                        ReferenceFrameId::EarthFixedEcefV1,
                        inertial,
                        round_trip,
                    )?;
                    self.segment = GlobalSegment::EcefEntry;
                    self.coordinates = Coordinates::Global(fixed);
                    self.events |= EVENT_ENTRY_OWNER;
                }
                GlobalSegment::EcefEntry
                    if altitude <= self.mission.recovery_transition_altitude_q12_km
                        && self.mach_q24 <= self.mission.recovery_transition_mach_q24
                        && distance_q12(ecef.position, self.recovery_anchor.origin_ecef)?
                            <= self.mission.recovery_radius_q12_km =>
                {
                    let local = ecef_to_local(self.recovery_anchor, ecef)?;
                    let round_trip = local_to_ecef(self.recovery_anchor, local)?;
                    self.record_transition(
                        ReferenceFrameId::EarthFixedEcefV1,
                        ReferenceFrameId::LocalEnuV1,
                        ecef,
                        round_trip,
                    )?;
                    self.segment = GlobalSegment::LocalRecovery;
                    self.coordinates = Coordinates::Local(local);
                    self.events |= EVENT_RECOVERY_OWNER;
                }
                _ => {}
            }
        }
        if self.segment == GlobalSegment::LocalRecovery {
            let local = match self.coordinates {
                Coordinates::Local(state) => state,
                _ => return Err(GlobalWorldError::Transition),
            };
            if local.position.z() <= 0 && local.velocity.z() < 0 {
                self.complete = true;
                self.events |= EVENT_LANDING;
            }
        }
        if time.raw() >= self.mission.max_mission_time_q16_s && !self.complete {
            return Err(GlobalWorldError::Timeout);
        }
        Ok(())
    }

    fn record_transition(
        &mut self,
        from: ReferenceFrameId,
        to: ReferenceFrameId,
        before: GlobalKinematicState,
        round_trip: GlobalKinematicState,
    ) -> Result<(), GlobalWorldError> {
        if self.transition_count as usize >= TRANSITION_CAPACITY || before.time != round_trip.time {
            return Err(GlobalWorldError::Transition);
        }
        let position_delta = max_vec_delta(before.position, round_trip.position);
        let velocity_delta = max_vec_delta(before.velocity, round_trip.velocity);
        let attitude_delta = max_quaternion_delta(before.attitude, round_trip.attitude);
        let angular_rate_delta = max_vec_delta(before.angular_rate, round_trip.angular_rate);
        let mut checksum: u32 = 0x811c_9dc5;
        for value in [
            from as u32,
            to as u32,
            before.time.raw(),
            position_delta as u32,
            velocity_delta as u32,
            attitude_delta as u32,
            angular_rate_delta as u32,
        ] {
            checksum = checksum.rotate_left(5) ^ value.wrapping_mul(0x9e37_79b1);
        }
        self.transitions[self.transition_count as usize] = FrameTransitionRecord {
            from,
            to,
            time: before.time,
            position_delta_raw: position_delta,
            velocity_delta_raw: velocity_delta,
            attitude_delta_raw: attitude_delta,
            angular_rate_delta_raw: angular_rate_delta,
            checksum,
        };
        self.transition_count += 1;
        Ok(())
    }

    fn update_checksum(&mut self) -> Result<(), GlobalWorldError> {
        let snapshot = self.canonical_state()?;
        for value in [
            self.segment as u32,
            snapshot.time.raw(),
            snapshot.position.x() as u32,
            snapshot.position.y() as u32,
            snapshot.position.z() as u32,
            snapshot.velocity.x() as u32,
            snapshot.velocity.y() as u32,
            snapshot.velocity.z() as u32,
            self.main_propellant_q21 as u32,
            self.events as u32,
        ] {
            self.checksum = self.checksum.rotate_left(5) ^ value.wrapping_mul(0x0100_0193);
        }
        Ok(())
    }
}

fn interface_frame(frame: ReferenceFrameId) -> GlobalFrameId {
    match frame {
        ReferenceFrameId::LocalEnuV1 => GlobalFrameId::LocalEnuV1,
        ReferenceFrameId::EarthFixedEcefV1 => GlobalFrameId::EarthFixedEcefV1,
        ReferenceFrameId::EarthInertialEciV1 => GlobalFrameId::EarthInertialEciV1,
    }
}

fn local_active_state(
    state: LocalKinematicState,
) -> Result<GlobalKinematicState, GlobalWorldError> {
    let position = GlobalPositionVec::new(
        rounded_i64(state.position.x() as i64, 2_000)?,
        rounded_i64(state.position.y() as i64, 2_000)?,
        rounded_i64(state.position.z() as i64, 2_000)?,
    );
    let velocity = GlobalVelocityVec::new(
        rounded_i64(state.velocity.x() as i64 * 4, 125)?,
        rounded_i64(state.velocity.y() as i64 * 4, 125)?,
        rounded_i64(state.velocity.z() as i64 * 4, 125)?,
    );
    Ok(GlobalKinematicState::new(
        position,
        velocity,
        state.attitude,
        state.angular_rate,
        state.time,
    ))
}

fn interpolate_pitch(
    lo: PitchKnot,
    hi: PitchKnot,
    time: MissionTimeQ16,
) -> Result<i32, GlobalWorldError> {
    let mut status = NumericStatus::CLEAR;
    let value = interpolate_i32(
        lo.elevation_q28_rad,
        hi.elevation_q28_rad,
        time.raw() - lo.time_q16_s,
        hi.time_q16_s - lo.time_q16_s,
        &mut status,
    );
    if status.is_clear() {
        Ok(value)
    } else {
        Err(GlobalWorldError::Numeric)
    }
}

fn rounded_i64(value: i64, denominator: i64) -> Result<i32, GlobalWorldError> {
    if denominator <= 0 {
        return Err(GlobalWorldError::Numeric);
    }
    let half = denominator / 2;
    let value = if value >= 0 {
        (value + half) / denominator
    } else {
        (value - half) / denominator
    };
    if value < i32::MIN as i64 || value > i32::MAX as i64 {
        Err(GlobalWorldError::Numeric)
    } else {
        Ok(value as i32)
    }
}

fn global_air_data(
    velocity: GlobalVelocityVec,
    atmosphere: AtmosphereSample,
) -> Result<(i32, i32, i32), GlobalWorldError> {
    let mut status = NumericStatus::CLEAR;
    let magnitude = magnitude3_floor(velocity.x(), velocity.y(), velocity.z(), &mut status);
    if magnitude > i32::MAX as u32 || !status.is_clear() {
        return Err(GlobalWorldError::Numeric);
    }
    let speed_q4 = rounded_i64(magnitude as i64 * 125, 131_072)?;
    air_data_from_speed(speed_q4, atmosphere)
}

fn local_air_data(
    velocity: EnuVelocity,
    atmosphere: AtmosphereSample,
) -> Result<(i32, i32, i32), GlobalWorldError> {
    let mut status = NumericStatus::CLEAR;
    let magnitude = magnitude3_floor(velocity.x(), velocity.y(), velocity.z(), &mut status);
    if magnitude > i32::MAX as u32 || !status.is_clear() {
        return Err(GlobalWorldError::Numeric);
    }
    let speed_q4 = rounded_i64(magnitude as i64, 1 << 15)?;
    air_data_from_speed(speed_q4, atmosphere)
}

fn air_data_from_speed(
    speed_q4: i32,
    atmosphere: AtmosphereSample,
) -> Result<(i32, i32, i32), GlobalWorldError> {
    if atmosphere.density_q28_kg_m3 == 0 {
        return Ok((speed_q4, 0, 0));
    }
    let sound_q4 = ((atmosphere.speed_of_sound_q16_m_s + (1 << 11)) >> 12).max(1);
    let mut status = NumericStatus::CLEAR;
    let mach_q24 = ksa64_core::numeric::divide_scaled(speed_q4, sound_q4, 24, &mut status);
    if !status.is_clear() {
        return Err(GlobalWorldError::Numeric);
    }
    if mach_q24 > MAX_MACH_Q24 {
        return Err(GlobalWorldError::Envelope);
    }
    let density_q20 = (atmosphere.density_q28_kg_m3 + 128) >> 8;
    let q_raw = density_q20 as i64 * speed_q4 as i64 * speed_q4 as i64;
    let dynamic_q14 = rounded_i64(q_raw, 1 << 15)?;
    if dynamic_q14 > MAX_DYNAMIC_PRESSURE_Q14 {
        Err(GlobalWorldError::Envelope)
    } else {
        Ok((speed_q4, mach_q24, dynamic_q14))
    }
}

fn drag_force_q13(
    dynamic_q14: i32,
    cda_raw: i32,
    cda_fractional_bits: u8,
    cd_q24: i32,
) -> Result<i32, GlobalWorldError> {
    let shift = 14 + cda_fractional_bits - 13;
    if shift > 31 {
        return Err(GlobalWorldError::Numeric);
    }
    let mut status = NumericStatus::CLEAR;
    let area_force = multiply_scaled(dynamic_q14, cda_raw, shift, &mut status);
    let force = multiply_scaled(area_force, cd_q24, 24, &mut status);
    if status.is_clear() {
        Ok(force.max(0))
    } else {
        Err(GlobalWorldError::Numeric)
    }
}

fn force_to_acceleration_q19(force_q13: i32, mass_q13: i32) -> Result<i32, GlobalWorldError> {
    rounded_i64(force_q13 as i64 * (1 << 19), mass_q13 as i64)
}

fn force_magnitude_to_global_q28(force_q13: i32, mass_q21: i32) -> Result<i32, GlobalWorldError> {
    let mass_q13 = ((mass_q21 as i64 + 128) >> 8) as i32;
    rounded_i64(force_q13 as i64 * (1i64 << 28), mass_q13 as i64 * 1_000)
}

fn force_to_global_acceleration(
    force_q13: i32,
    mass_q21: i32,
    velocity: GlobalVelocityVec,
    speed_q4: i32,
) -> Result<GlobalAccelerationVec, GlobalWorldError> {
    if speed_q4 == 0 || force_q13 == 0 {
        return Ok(GlobalAccelerationVec::ZERO);
    }
    let mut status = NumericStatus::CLEAR;
    let magnitude = magnitude3_floor(velocity.x(), velocity.y(), velocity.z(), &mut status);
    if !status.is_clear() || magnitude == 0 || magnitude > i32::MAX as u32 {
        return Err(GlobalWorldError::Numeric);
    }
    let acceleration = force_magnitude_to_global_q28(force_q13, mass_q21)?;
    let ratio = |component: i32| rounded_i64(-(component as i64) * (1i64 << 30), magnitude as i64);
    let direction = FixedVec3::<30>::new(
        ratio(velocity.x())?,
        ratio(velocity.y())?,
        ratio(velocity.z())?,
    );
    status = NumericStatus::CLEAR;
    let result = scale_direction::<28>(direction, acceleration, &mut status);
    if status.is_clear() {
        Ok(result)
    } else {
        Err(GlobalWorldError::Numeric)
    }
}

fn unit_against_local_velocity(
    velocity: EnuVelocity,
    speed_q4: i32,
    acceleration_q19: i32,
) -> Result<EnuAcceleration, GlobalWorldError> {
    if speed_q4 == 0 || acceleration_q19 == 0 {
        return Ok(EnuAcceleration::ZERO);
    }
    let mut status = NumericStatus::CLEAR;
    let magnitude = magnitude3_floor(velocity.x(), velocity.y(), velocity.z(), &mut status);
    if !status.is_clear() || magnitude == 0 || magnitude > i32::MAX as u32 {
        return Err(GlobalWorldError::Numeric);
    }
    let ratio = |component: i32| rounded_i64(-(component as i64) * (1i64 << 30), magnitude as i64);
    let direction = FixedVec3::<30>::new(
        ratio(velocity.x())?,
        ratio(velocity.y())?,
        ratio(velocity.z())?,
    );
    status = NumericStatus::CLEAR;
    let result = scale_direction::<19>(direction, acceleration_q19, &mut status);
    if status.is_clear() {
        Ok(result)
    } else {
        Err(GlobalWorldError::Numeric)
    }
}

fn dot_mixed_q19_q30(left: FixedVec3<19>, right: FixedVec3<30>, status: &mut NumericStatus) -> i32 {
    add(
        add(
            multiply_scaled(left.x(), right.x(), 30, status),
            multiply_scaled(left.y(), right.y(), 30, status),
            status,
        ),
        multiply_scaled(left.z(), right.z(), 30, status),
        status,
    )
}

fn distance_q12(a: GlobalPositionVec, b: GlobalPositionVec) -> Result<i32, GlobalWorldError> {
    let mut status = NumericStatus::CLEAR;

    let difference = a.checked_sub(b, &mut status);
    let magnitude = magnitude3_floor(difference.x(), difference.y(), difference.z(), &mut status);
    if !status.is_clear() || magnitude > i32::MAX as u32 {
        Err(GlobalWorldError::Numeric)
    } else {
        Ok(magnitude as i32)
    }
}

fn dot_mixed_q13_q30(left: FixedVec3<13>, right: FixedVec3<30>, status: &mut NumericStatus) -> i32 {
    add(
        add(
            multiply_scaled(left.x(), right.x(), 30, status),
            multiply_scaled(left.y(), right.y(), 30, status),
            status,
        ),
        multiply_scaled(left.z(), right.z(), 30, status),
        status,
    )
}

fn scale_direction<const OUTPUT: u8>(
    direction: FixedVec3<30>,
    magnitude: i32,
    status: &mut NumericStatus,
) -> FixedVec3<OUTPUT> {
    FixedVec3::new(
        multiply_scaled(direction.x(), magnitude, 30, status),
        multiply_scaled(direction.y(), magnitude, 30, status),
        multiply_scaled(direction.z(), magnitude, 30, status),
    )
}
fn max_vec_delta<const F: u8>(a: FixedVec3<F>, b: FixedVec3<F>) -> i32 {
    (a.x() - b.x())
        .abs()
        .max((a.y() - b.y()).abs())
        .max((a.z() - b.z()).abs())
}

fn max_quaternion_delta(a: QuaternionQ30, b: QuaternionQ30) -> i32 {
    (a.w() - b.w())
        .abs()
        .max((a.x() - b.x()).abs())
        .max((a.y() - b.y()).abs())
        .max((a.z() - b.z()).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (
        EarthModelPack,
        TransformPack,
        CompiledAtmospherePack,
        GlobalVehiclePack,
        GlobalMissionPack,
    ) {
        (
            EarthModelPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kem10"))
                .unwrap(),
            TransformPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kft10"))
                .unwrap(),
            CompiledAtmospherePack::decode(include_bytes!(
                "../../phase10/generated/ksa-g10r.kat10"
            ))
            .unwrap(),
            GlobalVehiclePack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgv10"))
                .unwrap(),
            GlobalMissionPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgm10"))
                .unwrap(),
        )
    }

    #[test]
    fn nominal_global_world_crosses_every_owner_and_lands() {
        let (earth, transforms, atmosphere, vehicle, mission) = fixture();
        let mut world =
            GlobalWorldMachine::new(&earth, &transforms, &atmosphere, &vehicle, mission).unwrap();
        let mut steps = 0u32;
        while !world.is_complete() && steps < 150_000 {
            world.step().unwrap();
            steps += 1;
        }
        let snapshot = world.snapshot().unwrap();
        assert!(world.is_complete(), "{snapshot:?}");
        assert_eq!(snapshot.segment, GlobalSegment::LocalRecovery);
        assert_eq!(snapshot.transition_count, 4);
        assert!((200 << 12..=300 << 12).contains(&snapshot.apogee_q12_km));
        assert_ne!(snapshot.checksum, 0);
        for transition in world.transitions().iter().take(4) {
            assert!(transition.position_delta_raw <= 4);
            assert!(transition.velocity_delta_raw <= 4);
            assert!(transition.attitude_delta_raw <= 4);
            assert_eq!(transition.angular_rate_delta_raw, 0);
        }
    }

    #[test]
    fn air_data_and_drag_envelopes_fail_closed() {
        let atmosphere = AtmosphereSample {
            density_q28_kg_m3: 328_833_433,
            pressure_q14_pa: 101_325 << 14,
            temperature_q16_k: 288 << 16,
            speed_of_sound_q16_m_s: 340 << 16,
            wind_enu_q19_m_s: FixedVec3::ZERO,
        };
        assert!(air_data_from_speed(0, atmosphere).is_ok());
        assert_eq!(
            air_data_from_speed(12_000 << 4, atmosphere),
            Err(GlobalWorldError::Envelope)
        );
    }
}
