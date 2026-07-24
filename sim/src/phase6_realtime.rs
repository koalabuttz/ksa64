//! Native composition of the KSA-6R 32/8/1 Hz realtime profile.
use crate::phase5_vehicle::{
    GimbalCommandQ16, Phase5StagePhase, Phase5VehicleCommand, Phase5VehicleError,
    Phase5VehicleMachine, Phase6FastVehicle, PHASE5_SUBSTEPS,
};
use ksa64_core::spatial_numeric::FixedVec3;
use ksa64_flight::phase5_guidance::reference_guidance_target;
use ksa64_flight::phase6_realtime::RealtimeFlightComputer;
use ksa64_interface::phase6::{
    RealtimeAidCell, RealtimeCommandCell, RealtimeInertialCell, REALTIME_AID_GPS, REALTIME_AID_STAR,
};
use ksa64_interface::EngineAction;

pub const REALTIME_SESSION: u16 = 0x6a52;
pub const REALTIME_MAX_FAST_EPOCHS: u32 =
    ksa64_core::phase5_contract::PHASE5_MISSION_STEPS * PHASE5_SUBSTEPS as u32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RealtimeRunError {
    VehicleAt {
        epoch: u32,
        error: Phase5VehicleError,
        gimbal_q16: [i32; 2],
        rcs_q15: [i32; 3],
    },
    Epoch,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeRunEvidence {
    pub fast_epochs: u32,
    pub mission_steps: u32,
    pub terminal_phase: Phase5StagePhase,
    pub terminal_position_q12: [i32; 3],
    pub terminal_velocity_q24: [i32; 3],
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
    pub status_checksum: u32,
    pub safe: bool,
}

pub fn run_realtime_nominal() -> Result<RealtimeRunEvidence, RealtimeRunError> {
    let machine = Phase5VehicleMachine::new_ksa5a().map_err(|e| RealtimeRunError::VehicleAt {
        epoch: 0,
        error: e,
        gimbal_q16: [0; 2],
        rcs_q15: [0; 3],
    })?;
    let initial = machine.truth();
    let p0 = initial.spatial().position();
    let v0 = initial.spatial().velocity();
    let mut flight = RealtimeFlightComputer::new(
        REALTIME_SESSION,
        [p0.x(), p0.y(), p0.z()],
        [v0.x(), v0.y(), v0.z()],
    );
    let mut world = Phase6FastVehicle::new(machine);
    let mut command = Phase5VehicleCommand::HOLD;
    let mut snapshot = world
        .current_snapshot()
        .map_err(|e| RealtimeRunError::VehicleAt {
            epoch: 0,
            error: e,
            gimbal_q16: [0; 2],
            rcs_q15: [0; 3],
        })?;
    let mut epoch = 0u32;
    let mut status_checksum = 2_166_136_261u32;
    while epoch < REALTIME_MAX_FAST_EPOCHS {
        if epoch % PHASE5_SUBSTEPS as u32 == 0 {
            world
                .begin(command)
                .map_err(|error| RealtimeRunError::VehicleAt {
                    epoch,
                    error,
                    gimbal_q16: [command.gimbal.pitch, command.gimbal.yaw],
                    rcs_q15: [
                        command.rcs_q15.x(),
                        command.rcs_q15.y(),
                        command.rcs_q15.z(),
                    ],
                })?;
        }
        let (observation, committed) =
            world
                .advance(command)
                .map_err(|error| RealtimeRunError::VehicleAt {
                    epoch,
                    error,
                    gimbal_q16: [command.gimbal.pitch, command.gimbal.yaw],
                    rcs_q15: [
                        command.rcs_q15.x(),
                        command.rcs_q15.y(),
                        command.rcs_q15.z(),
                    ],
                })?;
        let truth = world.working_truth();
        let q = truth.rigid().attitude();
        let platform = [q.x() >> 15, q.y() >> 15, q.z() >> 15];
        let inertial = RealtimeInertialCell {
            session: REALTIME_SESSION,
            measurement_epoch: epoch as u16,
            production_epoch: epoch as u16,
            validity: 0xff,
            flags: 0,
            platform_angle: [
                clamp_i16(platform[0]),
                clamp_i16(platform[1]),
                clamp_i16(platform[2]),
            ],
            angular_rate: [
                clamp_i16(observation.gyro_body_q24[0] >> 12),
                clamp_i16(observation.gyro_body_q24[1] >> 12),
                clamp_i16(observation.gyro_body_q24[2] >> 12),
            ],
            delta_velocity: [
                clamp_i16(observation.accel_body_q28[0] >> 21),
                clamp_i16(observation.accel_body_q28[1] >> 21),
                clamp_i16(observation.accel_body_q28[2] >> 21),
            ],
            gimbal_applied: [
                clamp_i16(observation.gimbal.applied.pitch),
                clamp_i16(observation.gimbal.applied.yaw),
            ],
            stage_status: truth.phase() as u16,
        };
        if epoch & 31 == 0 {
            let mission_step = epoch / PHASE5_SUBSTEPS as u32;
            let target = reference_guidance_target(mission_step);
            let target_end = reference_guidance_target(mission_step.saturating_add(8));
            flight.set_guidance_segment(
                [
                    clamp_i16(target.attitude_q30[1] >> 15),
                    clamp_i16(target.attitude_q30[2] >> 15),
                    clamp_i16(target.attitude_q30[3] >> 15),
                ],
                [
                    clamp_i16(target_end.attitude_q30[1] >> 15),
                    clamp_i16(target_end.attitude_q30[2] >> 15),
                    clamp_i16(target_end.attitude_q30[3] >> 15),
                ],
                [
                    clamp_i16(target.angular_rate_q24[0] >> 12),
                    clamp_i16(target.angular_rate_q24[1] >> 12),
                    clamp_i16(target.angular_rate_q24[2] >> 12),
                ],
            );
        }
        let aid = if epoch & 3 == 0 {
            let p = truth.spatial().position();
            let v = truth.spatial().velocity();
            Some(RealtimeAidCell {
                session: REALTIME_SESSION,
                measurement_epoch: epoch as u16,
                production_epoch: epoch as u16,
                validity: REALTIME_AID_GPS | REALTIME_AID_STAR,
                events: observation.events,
                onboard_time_q16: truth.time_q16(),
                barometer_q12: 0,
                gps_position_q12: [p.x(), p.y(), p.z()],
                gps_velocity_q24: [v.x(), v.y(), v.z()],
                star_angle: inertial.platform_angle,
                rcs_propellant_q12: world.committed_machine().rcs_propellant_q12(),
                vehicle_status: truth.phase() as u32,
            })
        } else {
            None
        };
        let output = flight.tick(Some(inertial), aid);
        command = map_command(output.command, output.safe);
        if let Some(status) = output.status {
            status_checksum = hash_status(
                status_checksum,
                status.flight_checksum,
                status.alarms as u32,
            );
        }
        epoch += 1;
        if let Some(value) = committed {
            snapshot = value;
            if snapshot.truth.phase() == Phase5StagePhase::Complete {
                break;
            }
        }
    }
    if epoch > u16::MAX as u32 {
        return Err(RealtimeRunError::Epoch);
    }
    let truth = snapshot.truth;
    let p = truth.spatial().position();
    let v = truth.spatial().velocity();
    let nav = flight.navigation();
    Ok(RealtimeRunEvidence {
        fast_epochs: epoch,
        mission_steps: truth.step(),
        terminal_phase: truth.phase(),
        terminal_position_q12: [p.x(), p.y(), p.z()],
        terminal_velocity_q24: [v.x(), v.y(), v.z()],
        navigation_position_q12: nav.position_q12,
        navigation_velocity_q24: nav.velocity_q24,
        navigation_checksum: nav.checksum,
        flight_checksum: flight.flight_checksum(),
        status_checksum,
        safe: flight.is_safe(),
    })
}
fn map_command(cell: RealtimeCommandCell, safe: bool) -> Phase5VehicleCommand {
    let cutoff = safe || cell.discrete & 4 != 0;
    Phase5VehicleCommand {
        gimbal: GimbalCommandQ16 {
            pitch: cell.gimbal[0] as i32,
            yaw: cell.gimbal[1] as i32,
        },
        rcs_q15: FixedVec3::new(
            (cell.rcs[0] as i32) << 8,
            (cell.rcs[1] as i32) << 8,
            (cell.rcs[2] as i32) << 8,
        ),
        engine_action: if cutoff {
            EngineAction::Cutoff
        } else if cell.discrete & 1 != 0 {
            EngineAction::Ignite
        } else {
            EngineAction::Hold
        },
        separate: cell.discrete & 2 != 0,
        abort_safeing: safe,
    }
}
const fn clamp_i16(v: i32) -> i16 {
    if v > i16::MAX as i32 {
        i16::MAX
    } else if v < i16::MIN as i32 {
        i16::MIN
    } else {
        v as i16
    }
}
fn hash_status(mut h: u32, a: u32, b: u32) -> u32 {
    for v in [a, b] {
        for byte in v.to_le_bytes() {
            h ^= byte as u32;
            h = h.wrapping_mul(16_777_619)
        }
    }
    h
}
