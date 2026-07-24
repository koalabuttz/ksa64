use ksa64_flight::phase6_realtime::{
    RealtimeFlightComputer, VirtualScheduler, ALARM_DEADLINE, ALARM_SAFEING, PAL_TICK_BUDGET_CYCLES,
};
use ksa64_interface::phase6::{RealtimeAidCell, RealtimeInertialCell};

fn inertial(epoch: u16) -> RealtimeInertialCell {
    RealtimeInertialCell {
        session: 7,
        measurement_epoch: epoch,
        production_epoch: epoch,
        validity: 0xff,
        flags: 0,
        platform_angle: [100, -200, 300],
        angular_rate: [0; 3],
        delta_velocity: [1, 2, 3],
        gimbal_applied: [0; 2],
        stage_status: 1,
    }
}

fn aid(epoch: u16) -> RealtimeAidCell {
    RealtimeAidCell {
        session: 7,
        measurement_epoch: epoch,
        production_epoch: epoch,
        validity: 4,
        events: 0,
        onboard_time_q16: 0,
        barometer_q12: 0,
        gps_position_q12: [4000, 8000, 12000],
        gps_velocity_q24: [16000, 32000, 48000],
        star_angle: [0; 3],
        rcs_propellant_q12: 0,
        vehicle_status: 0,
    }
}

#[test]
fn scheduler_releases_exact_32_8_1_cadence() {
    let mut scheduler = VirtualScheduler::new();
    let mut navigation = 0;
    let mut guidance = 0;
    let mut status = 0;
    for expected in 0..64u16 {
        let release = scheduler.release();
        assert_eq!(release.epoch, expected);
        assert!(release.fast);
        navigation += release.navigation as u8;
        guidance += release.guidance as u8;
        status += release.status as u8;
    }
    assert_eq!((navigation, guidance, status), (16, 2, 16));
}

#[test]
fn valid_cells_produce_next_epoch_commands_and_deterministic_status() {
    let mut first = RealtimeFlightComputer::new(7, [0; 3], [0; 3]);
    let mut second = RealtimeFlightComputer::new(7, [0; 3], [0; 3]);
    first.set_guidance_target([500, 600, 700]);
    second.set_guidance_target([500, 600, 700]);
    for epoch in 0..32u16 {
        let a = if epoch & 3 == 0 {
            Some(aid(epoch))
        } else {
            None
        };
        let left = first.tick(Some(inertial(epoch)), a);
        let right = second.tick(Some(inertial(epoch)), a);
        assert_eq!(left, right);
        assert_eq!(left.command.source_epoch, epoch);
        assert_eq!(left.command.effective_epoch, epoch + 1);
        assert!(!left.safe);
        assert_eq!(left.status.is_some(), epoch & 3 == 0);
    }
    assert_eq!(first.navigation(), second.navigation());
}

#[test]
fn two_missing_cells_hold_then_third_missing_cell_safes() {
    let mut flight = RealtimeFlightComputer::new(7, [0; 3], [0; 3]);
    let first = flight.tick(None, None);
    let second = flight.tick(None, None);
    let third = flight.tick(None, None);
    assert!(!first.safe);
    assert!(!second.safe);
    assert!(third.safe);
    assert_eq!(third.command.flags & 1, 1);
    assert_eq!(third.command.discrete & 4, 4);
}

#[test]
fn deadline_overrun_latches_safe_mode_and_alarm() {
    let mut flight = RealtimeFlightComputer::new(7, [0; 3], [0; 3]);
    flight.record_cycles(PAL_TICK_BUDGET_CYCLES + 1);
    let evidence = flight.tick(Some(inertial(0)), Some(aid(0)));
    assert!(evidence.safe);
    assert_eq!(flight.deadline_misses(), 1);
    let status = evidence.status.unwrap();
    assert_ne!(status.alarms & ALARM_DEADLINE, 0);
    assert_ne!(status.alarms & ALARM_SAFEING, 0);
}
