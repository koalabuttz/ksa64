//! Deterministic one-dimensional hobby-rocket mission execution.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericStatus};
use crate::phase7_environment::sample_hobby_environment;
use crate::phase7_numeric::*;
use crate::phase7_pack::{packs_are_compatible, HobbyMissionPack, MotorPack, VerticalVehiclePack};

pub const HOBBY_EVENT_IGNITION: u32 = 1 << 0;
pub const HOBBY_EVENT_LIFTOFF: u32 = 1 << 1;
pub const HOBBY_EVENT_RAIL_EXIT: u32 = 1 << 2;
pub const HOBBY_EVENT_BURNOUT: u32 = 1 << 3;
pub const HOBBY_EVENT_APOGEE: u32 = 1 << 4;
pub const HOBBY_EVENT_DROGUE: u32 = 1 << 5;
pub const HOBBY_EVENT_MAIN: u32 = 1 << 6;
pub const HOBBY_EVENT_GROUND: u32 = 1 << 7;
pub const HOBBY_EVENT_END: u32 = 1 << 8;

const MAX_HOBBY_STEPS: u32 = 200_000;
const CHECKSUM_OFFSET: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HobbyFlightPhase {
    ConstrainedPowered = 0,
    Powered = 1,
    Coast = 2,
    DrogueInflating = 3,
    DrogueDescent = 4,
    MainInflating = 5,
    MainDescent = 6,
    Complete = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HobbyMissionOutcome {
    Landed,
    NoLiftoff,
    RecoveryIncomplete,
    NumericFault,
    StepLimit,
    ConfigurationFault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyVerticalState {
    pub step: u32,
    pub time: HobbyTime,
    pub altitude: HobbyAltitude,
    pub velocity: HobbyVelocity,
    pub acceleration: HobbyAcceleration,
    pub mass: HobbyMass,
    pub propellant: HobbyMass,
    pub impulse_consumed_q16: i32,
    pub phase: HobbyFlightPhase,
    pub phase_start_time: HobbyTime,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HobbyMilestone {
    pub valid: bool,
    pub time_raw: i32,
    pub altitude_raw: i32,
    pub velocity_raw: i32,
}

impl HobbyMilestone {
    const fn from_state(state: HobbyVerticalState) -> Self {
        Self {
            valid: true,
            time_raw: state.time.raw(),
            altitude_raw: state.altitude.raw(),
            velocity_raw: state.velocity.raw(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyMissionObservation {
    pub state: HobbyVerticalState,
    pub thrust_raw_q13: i32,
    pub dynamic_pressure: HobbyDynamicPressure,
    pub mach: Option<HobbyMach>,
    pub events: u32,
    pub checksum: u32,
}

pub trait HobbyMissionObserver {
    type Error;
    fn observe(&mut self, observation: HobbyMissionObservation) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HobbyMissionExecutionError<E> {
    Configuration,
    Observer(E),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HobbyMissionResult {
    pub outcome: HobbyMissionOutcome,
    pub terminal: HobbyVerticalState,
    pub numeric_faults: u8,
    pub event_history: u32,
    pub max_altitude: HobbyAltitude,
    pub max_speed: HobbyVelocity,
    pub max_acceleration: HobbyAcceleration,
    pub max_dynamic_pressure: HobbyDynamicPressure,
    pub max_mach: HobbyMach,
    pub max_opening_deceleration: HobbyAcceleration,
    pub rail_exit: HobbyMilestone,
    pub burnout: HobbyMilestone,
    pub apogee: HobbyMilestone,
    pub drogue: HobbyMilestone,
    pub main: HobbyMilestone,
    pub ground: HobbyMilestone,
    pub state_checksum: u32,
}

struct NullObserver;
impl HobbyMissionObserver for NullObserver {
    type Error = core::convert::Infallible;
    fn observe(&mut self, _observation: HobbyMissionObservation) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn abs_i32(value: i32) -> i32 {
    if value == i32::MIN {
        i32::MAX
    } else {
        value.abs()
    }
}

fn hash_word(mut checksum: u32, word: u32) -> u32 {
    let mut shift = 0;
    while shift < 32 {
        checksum ^= (word >> shift) & 0xff;
        checksum = checksum.wrapping_mul(FNV_PRIME);
        shift += 8;
    }
    checksum
}

pub fn hash_hobby_state(mut checksum: u32, state: HobbyVerticalState) -> u32 {
    for word in [
        state.step,
        state.time.raw() as u32,
        state.altitude.raw() as u32,
        state.velocity.raw() as u32,
        state.acceleration.raw() as u32,
        state.mass.raw() as u32,
        state.propellant.raw() as u32,
        state.impulse_consumed_q16 as u32,
        state.phase as u32,
        state.phase_start_time.raw() as u32,
    ] {
        checksum = hash_word(checksum, word);
    }
    checksum
}

pub fn sample_motor_thrust(motor: &MotorPack, time: HobbyTime) -> i32 {
    if time.raw() < 0 || time > motor.burn_time {
        return 0;
    }
    let count = motor.knot_count as usize;
    let mut index = 1usize;
    while index < count && time > motor.knots[index].time {
        index += 1;
    }
    if index >= count {
        return 0;
    }
    let left = motor.knots[index - 1];
    let right = motor.knots[index];
    let span = right.time.raw() - left.time.raw();
    if span <= 0 {
        return 0;
    }
    let mut status = NumericStatus::CLEAR;
    let fraction = divide_scaled(time.raw() - left.time.raw(), span, 16, &mut status);
    if !status.is_clear() {
        return 0;
    }
    left.thrust_raw_q13
        + multiply_scaled(
            right.thrust_raw_q13 - left.thrust_raw_q13,
            fraction,
            16,
            &mut status,
        )
}

fn body_cda_q23(vehicle: VerticalVehiclePack, status: &mut NumericStatus) -> i32 {
    multiply_scaled(
        vehicle.reference_area.raw(),
        vehicle.body_cd_q16,
        21,
        status,
    )
}

fn recovery_fraction_q16(state: HobbyVerticalState, duration: HobbyTime) -> i32 {
    let elapsed = state.time.raw() - state.phase_start_time.raw();
    if elapsed <= 0 {
        0
    } else if elapsed >= duration.raw() {
        65_535
    } else {
        let mut status = NumericStatus::CLEAR;
        divide_scaled(elapsed, duration.raw(), 16, &mut status).clamp(0, 65_535)
    }
}

fn active_cda_q23(
    vehicle: VerticalVehiclePack,
    mission: HobbyMissionPack,
    state: HobbyVerticalState,
    status: &mut NumericStatus,
) -> i32 {
    let body = body_cda_q23(vehicle, status);
    match state.phase {
        HobbyFlightPhase::ConstrainedPowered
        | HobbyFlightPhase::Powered
        | HobbyFlightPhase::Coast => body,
        HobbyFlightPhase::DrogueInflating => {
            let fraction = recovery_fraction_q16(state, mission.drogue_inflation_time);
            add(
                body,
                multiply_scaled(vehicle.drogue_cda.raw() - body, fraction, 16, status),
                status,
            )
        }
        HobbyFlightPhase::DrogueDescent => vehicle.drogue_cda.raw(),
        HobbyFlightPhase::MainInflating => {
            let fraction = recovery_fraction_q16(state, mission.main_inflation_time);
            add(
                vehicle.drogue_cda.raw(),
                multiply_scaled(
                    vehicle.main_cda.raw() - vehicle.drogue_cda.raw(),
                    fraction,
                    16,
                    status,
                ),
                status,
            )
        }
        HobbyFlightPhase::MainDescent | HobbyFlightPhase::Complete => vehicle.main_cda.raw(),
    }
}

fn cadence(phase: HobbyFlightPhase) -> HobbyTime {
    match phase {
        HobbyFlightPhase::ConstrainedPowered | HobbyFlightPhase::Powered => HOBBY_POWERED_STEP,
        HobbyFlightPhase::Coast => HOBBY_COAST_STEP,
        HobbyFlightPhase::DrogueInflating
        | HobbyFlightPhase::DrogueDescent
        | HobbyFlightPhase::MainInflating
        | HobbyFlightPhase::MainDescent
        | HobbyFlightPhase::Complete => HOBBY_RECOVERY_STEP,
    }
}

fn initial_state(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
) -> HobbyVerticalState {
    HobbyVerticalState {
        step: 0,
        time: HobbyTime::ZERO,
        altitude: mission.launch_altitude,
        velocity: HobbyVelocity::ZERO,
        acceleration: HobbyAcceleration::ZERO,
        mass: HobbyMass::from_raw(vehicle.dry_mass.raw() + motor.loaded_mass.raw()),
        propellant: motor.propellant_mass,
        impulse_consumed_q16: 0,
        phase: HobbyFlightPhase::ConstrainedPowered,
        phase_start_time: HobbyTime::ZERO,
    }
}

fn evaluate_motion(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
    state: HobbyVerticalState,
    status: &mut NumericStatus,
) -> (i32, i32, i32, Option<i32>) {
    let environment = sample_hobby_environment(state.altitude, status);
    let thrust = if matches!(
        state.phase,
        HobbyFlightPhase::ConstrainedPowered | HobbyFlightPhase::Powered
    ) {
        sample_motor_thrust(motor, state.time)
    } else {
        0
    };
    let speed_squared_q8 = multiply_scaled(state.velocity.raw(), state.velocity.raw(), 30, status);
    let dynamic_pressure_q7 =
        multiply_scaled(environment.density.raw(), speed_squared_q8, 30, status) / 2;
    let drag_magnitude_q13 = multiply_scaled(
        dynamic_pressure_q7,
        active_cda_q23(vehicle, mission, state, status),
        17,
        status,
    );
    let gravity_force_q13 =
        multiply_scaled(state.mass.raw(), environment.gravity.raw(), 27, status);
    let mut net_force_q13 = subtract(thrust, gravity_force_q13, status);
    net_force_q13 = if state.velocity.raw() >= 0 {
        subtract(net_force_q13, drag_magnitude_q13, status)
    } else {
        add(net_force_q13, drag_magnitude_q13, status)
    };
    let acceleration_q19 = divide_scaled(net_force_q13, state.mass.raw(), 27, status);
    let mach = environment.sound_speed.map(|sound_speed| {
        divide_scaled(
            abs_i32(state.velocity.raw()),
            sound_speed.raw(),
            HOBBY_MACH_FRACTIONAL_BITS,
            status,
        )
    });
    (thrust, dynamic_pressure_q7, acceleration_q19, mach)
}

fn update_propellant(
    motor: &MotorPack,
    state: HobbyVerticalState,
    thrust_q13: i32,
    dt: HobbyTime,
    status: &mut NumericStatus,
) -> (i32, i32) {
    if thrust_q13 <= 0 || state.impulse_consumed_q16 >= motor.total_impulse_raw_q16 {
        return (state.impulse_consumed_q16, state.propellant.raw());
    }
    let impulse_delta_q16 = multiply_scaled(thrust_q13, dt.raw(), 15, status);
    let consumed =
        add(state.impulse_consumed_q16, impulse_delta_q16, status).min(motor.total_impulse_raw_q16);
    let propellant_per_impulse_q21 = divide_scaled(
        motor.propellant_mass.raw(),
        motor.total_impulse_raw_q16,
        16,
        status,
    );
    let used_q21 = multiply_scaled(propellant_per_impulse_q21, consumed, 16, status);
    (
        consumed,
        subtract(motor.propellant_mass.raw(), used_q21, status).max(0),
    )
}

fn run_internal<O: HobbyMissionObserver>(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
    observer: &mut O,
) -> Result<HobbyMissionResult, HobbyMissionExecutionError<O::Error>> {
    if !vehicle.is_valid()
        || !motor.is_valid()
        || !mission.is_valid()
        || !packs_are_compatible(vehicle, motor, mission)
    {
        return Err(HobbyMissionExecutionError::Configuration);
    }
    let mut state = initial_state(vehicle, motor, mission);
    let mut status = NumericStatus::CLEAR;
    let mut checksum = hash_hobby_state(CHECKSUM_OFFSET, state);
    let mut event_history = HOBBY_EVENT_IGNITION;
    let mut pending_events = HOBBY_EVENT_IGNITION;
    let mut liftoff = false;
    let mut max_altitude = state.altitude.raw();
    let mut max_speed = 0;
    let mut max_acceleration = 0;
    let mut max_q = 0;
    let mut max_mach = 0;
    let mut max_opening_deceleration = 0;
    let mut rail_exit = HobbyMilestone::default();
    let mut burnout = HobbyMilestone::default();
    let mut apogee = HobbyMilestone::default();
    let mut drogue = HobbyMilestone::default();
    let mut main = HobbyMilestone::default();
    let mut ground = HobbyMilestone::default();

    loop {
        let (thrust, q, acceleration, mach) =
            evaluate_motion(vehicle, motor, mission, state, &mut status);
        observer
            .observe(HobbyMissionObservation {
                state,
                thrust_raw_q13: thrust,
                dynamic_pressure: HobbyDynamicPressure::from_raw(q),
                mach: mach.map(HobbyMach::from_raw),
                events: pending_events,
                checksum,
            })
            .map_err(HobbyMissionExecutionError::Observer)?;
        pending_events = 0;
        if !status.is_clear() || state.phase == HobbyFlightPhase::Complete {
            break;
        }
        if state.step >= MAX_HOBBY_STEPS || state.time >= mission.max_mission_time {
            break;
        }
        let dt = cadence(state.phase);
        let (impulse, propellant) = update_propellant(motor, state, thrust, dt, &mut status);
        let dry_plus_case =
            vehicle.dry_mass.raw() + motor.loaded_mass.raw() - motor.propellant_mass.raw();
        let mass = HobbyMass::from_raw(add(dry_plus_case, propellant, &mut status));
        let acceleration_raw =
            if state.phase == HobbyFlightPhase::ConstrainedPowered && !liftoff && acceleration <= 0
            {
                0
            } else {
                acceleration
            };
        let mut velocity = add(
            state.velocity.raw(),
            multiply_scaled(acceleration_raw, dt.raw(), 18, &mut status),
            &mut status,
        );
        if state.phase == HobbyFlightPhase::ConstrainedPowered && !liftoff && velocity <= 0 {
            velocity = 0;
        }
        let mut altitude = add(
            state.altitude.raw(),
            multiply_scaled(velocity, dt.raw(), 24, &mut status),
            &mut status,
        );
        let time = HobbyTime::from_raw(add(state.time.raw(), dt.raw(), &mut status));
        let mut successor = HobbyVerticalState {
            step: state.step + 1,
            time,
            altitude: HobbyAltitude::from_raw(altitude),
            velocity: HobbyVelocity::from_raw(velocity),
            acceleration: HobbyAcceleration::from_raw(acceleration_raw),
            mass,
            propellant: HobbyMass::from_raw(propellant),
            impulse_consumed_q16: impulse,
            phase: state.phase,
            phase_start_time: state.phase_start_time,
        };
        if !liftoff && velocity > 0 {
            liftoff = true;
            pending_events |= HOBBY_EVENT_LIFTOFF;
        }
        let rail_top = mission.launch_altitude.raw() + mission.rail_length.raw();
        if successor.phase == HobbyFlightPhase::ConstrainedPowered && altitude >= rail_top {
            successor.phase = HobbyFlightPhase::Powered;
            successor.phase_start_time = time;
            pending_events |= HOBBY_EVENT_RAIL_EXIT;
            rail_exit = HobbyMilestone::from_state(successor);
        }
        if matches!(
            successor.phase,
            HobbyFlightPhase::ConstrainedPowered | HobbyFlightPhase::Powered
        ) && time >= motor.burn_time
        {
            successor.phase = HobbyFlightPhase::Coast;
            successor.phase_start_time = time;
            pending_events |= HOBBY_EVENT_BURNOUT;
            burnout = HobbyMilestone::from_state(successor);
        }
        if successor.phase == HobbyFlightPhase::Coast && state.velocity.raw() > 0 && velocity <= 0 {
            successor.phase = HobbyFlightPhase::DrogueInflating;
            successor.phase_start_time = time;
            pending_events |= HOBBY_EVENT_APOGEE | HOBBY_EVENT_DROGUE;
            apogee = HobbyMilestone::from_state(successor);
            drogue = apogee;
        }
        if successor.phase == HobbyFlightPhase::DrogueInflating
            && time.raw() - successor.phase_start_time.raw() >= mission.drogue_inflation_time.raw()
        {
            successor.phase = HobbyFlightPhase::DrogueDescent;
            successor.phase_start_time = time;
        }
        let main_threshold = mission.launch_altitude.raw() + mission.main_deployment_altitude.raw();
        if matches!(
            successor.phase,
            HobbyFlightPhase::DrogueInflating | HobbyFlightPhase::DrogueDescent
        ) && velocity < 0
            && altitude <= main_threshold
        {
            successor.phase = HobbyFlightPhase::MainInflating;
            successor.phase_start_time = time;
            pending_events |= HOBBY_EVENT_MAIN;
            main = HobbyMilestone::from_state(successor);
        }
        if successor.phase == HobbyFlightPhase::MainInflating
            && time.raw() - successor.phase_start_time.raw() >= mission.main_inflation_time.raw()
        {
            successor.phase = HobbyFlightPhase::MainDescent;
            successor.phase_start_time = time;
        }
        if liftoff && velocity < 0 && altitude <= mission.launch_altitude.raw() {
            altitude = mission.launch_altitude.raw();
            successor.altitude = mission.launch_altitude;
            successor.phase = HobbyFlightPhase::Complete;
            successor.phase_start_time = time;
            pending_events |= HOBBY_EVENT_GROUND | HOBBY_EVENT_END;
            ground = HobbyMilestone::from_state(successor);
        }
        event_history |= pending_events;
        max_altitude = max_altitude.max(altitude);
        max_speed = max_speed.max(abs_i32(velocity));
        max_acceleration = max_acceleration.max(abs_i32(acceleration_raw));
        max_q = max_q.max(q);
        max_mach = max_mach.max(mach.unwrap_or(0));
        if matches!(
            successor.phase,
            HobbyFlightPhase::DrogueInflating | HobbyFlightPhase::MainInflating
        ) {
            max_opening_deceleration = max_opening_deceleration.max(abs_i32(acceleration_raw));
        }
        checksum = hash_hobby_state(checksum, successor);
        state = successor;
    }
    let outcome = if !status.is_clear() {
        HobbyMissionOutcome::NumericFault
    } else if state.phase == HobbyFlightPhase::Complete {
        HobbyMissionOutcome::Landed
    } else if !liftoff {
        HobbyMissionOutcome::NoLiftoff
    } else if state.step >= MAX_HOBBY_STEPS {
        HobbyMissionOutcome::StepLimit
    } else {
        HobbyMissionOutcome::RecoveryIncomplete
    };
    Ok(HobbyMissionResult {
        outcome,
        terminal: state,
        numeric_faults: status.bits(),
        event_history,
        max_altitude: HobbyAltitude::from_raw(max_altitude),
        max_speed: HobbyVelocity::from_raw(max_speed),
        max_acceleration: HobbyAcceleration::from_raw(max_acceleration),
        max_dynamic_pressure: HobbyDynamicPressure::from_raw(max_q),
        max_mach: HobbyMach::from_raw(max_mach),
        max_opening_deceleration: HobbyAcceleration::from_raw(max_opening_deceleration),
        rail_exit,
        burnout,
        apogee,
        drogue,
        main,
        ground,
        state_checksum: checksum,
    })
}

pub fn execute_hobby_mission(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
) -> Result<HobbyMissionResult, HobbyMissionExecutionError<core::convert::Infallible>> {
    run_internal(vehicle, motor, mission, &mut NullObserver)
}

pub fn execute_hobby_mission_observed<O: HobbyMissionObserver>(
    vehicle: VerticalVehiclePack,
    motor: &MotorPack,
    mission: HobbyMissionPack,
    observer: &mut O,
) -> Result<HobbyMissionResult, HobbyMissionExecutionError<O::Error>> {
    run_internal(vehicle, motor, mission, observer)
}
