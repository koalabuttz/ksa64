//! Shared native/MOS acceptance fixture for `SafeholdRecoveryV1`.

use ksa64_flight::phase10::GlobalFlightMode;
use ksa64_flight::phase11::{GlobalKlr10FlightPackage, EVENT_JOURNAL_CAPACITY};
use ksa64_flight::phase11_safehold::{
    SafeholdRecoveryConfig, SafeholdRecoveryV1, SafeholdSessionSegment,
};
use ksa64_interface::phase10::{
    GlobalAidFrameCell, GlobalFastSensorCell, GlobalFrameId, GlobalTransitionCell,
    GLOBAL_AID_VALID_MASK, GLOBAL_COMMAND_DROGUE, GLOBAL_COMMAND_MAIN, GLOBAL_FAST_VALID_MASK,
};
use ksa64_interface::phase11::EventJournalRecord;

const Q30_ONE: i32 = 1 << 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeholdProbeResult {
    pub releases: u16,
    pub failures: u16,
    pub flight_checksum: u32,
    pub navigation_checksum: u32,
    pub command_checksum: u32,
    pub journal_chain: u32,
    pub drogue_epoch: u16,
    pub main_epoch: u16,
    pub transition_count: u8,
    pub final_frame: GlobalFrameId,
    pub safe: bool,
}

pub fn run_safehold_probe() -> SafeholdProbeResult {
    let mut package = SafeholdRecoveryV1::new(config()).unwrap();
    let mut drogue_epoch = u16::MAX;
    let mut main_epoch = u16::MAX;
    let mut failures = 0u16;
    let mut final_evidence = None;
    for epoch in 0..16u16 {
        let (frame, change) = match epoch {
            8 => (
                GlobalFrameId::EarthFixedEcefV1,
                Some(transition(
                    epoch,
                    GlobalFrameId::EarthInertialEciV1,
                    GlobalFrameId::EarthFixedEcefV1,
                )),
            ),
            12 => (
                GlobalFrameId::LocalEnuV1,
                Some(transition(
                    epoch,
                    GlobalFrameId::EarthFixedEcefV1,
                    GlobalFrameId::LocalEnuV1,
                )),
            ),
            0..=7 => (GlobalFrameId::EarthInertialEciV1, None),
            9..=11 => (GlobalFrameId::EarthFixedEcefV1, None),
            _ => (GlobalFrameId::LocalEnuV1, None),
        };
        let feedback = u16::from(epoch >= 14);
        let evidence = package.process_release(
            Some(fast(epoch, frame)),
            Some(aid(epoch, frame, feedback)),
            change,
        );
        if evidence.command.discrete & GLOBAL_COMMAND_DROGUE != 0 {
            drogue_epoch = drogue_epoch.min(epoch);
        }
        if evidence.command.discrete & GLOBAL_COMMAND_MAIN != 0 {
            main_epoch = main_epoch.min(epoch);
        }
        if evidence.command.source_epoch != epoch
            || evidence.command.effective_epoch != epoch.wrapping_add(1)
            || evidence.navigation.frame != frame
        {
            failures = failures.saturating_add(1);
        }
        final_evidence = Some(evidence);
    }
    let final_evidence = final_evidence.unwrap();
    let mut journal = [EventJournalRecord::EMPTY; EVENT_JOURNAL_CAPACITY];
    let journal_count = package.recover_journal_after(0, &mut journal);
    let journal_chain = if journal_count == 0 {
        0
    } else {
        journal[journal_count - 1].chain
    };
    if final_evidence.safe
        || final_evidence.mode != GlobalFlightMode::Recovery
        || drogue_epoch == u16::MAX
        || main_epoch == u16::MAX
        || package.segment() != SafeholdSessionSegment::LocalRecovery
    {
        failures = failures.saturating_add(1);
    }
    SafeholdProbeResult {
        releases: 16,
        failures,
        flight_checksum: final_evidence.flight_checksum,
        navigation_checksum: final_evidence.navigation.checksum,
        command_checksum: final_evidence.command.command_checksum,
        journal_chain,
        drogue_epoch,
        main_epoch,
        transition_count: package.transition_count(),
        final_frame: final_evidence.navigation.frame,
        safe: final_evidence.safe,
    }
}

pub fn phase11_safehold_probe_signature() -> u32 {
    let value = run_safehold_probe();
    let mut hash = 0x811c_9dc5u32;
    for word in [
        u32::from(value.releases),
        u32::from(value.failures),
        value.flight_checksum,
        value.navigation_checksum,
        value.command_checksum,
        value.journal_chain,
        u32::from(value.drogue_epoch),
        u32::from(value.main_epoch),
        u32::from(value.transition_count),
        value.final_frame as u32,
        u32::from(value.safe),
    ] {
        for byte in word.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    hash
}

fn config() -> SafeholdRecoveryConfig {
    SafeholdRecoveryConfig {
        session: 42,
        initial_segment: SafeholdSessionSegment::EciCoast,
        initial_position_q12: [0, 0, 4_096],
        initial_velocity_q24: [0, 0, -(1 << 24)],
        initial_attitude_q30: [Q30_ONE, 0, 0, 0],
        attitude_target_q30: [Q30_ONE, 0, 0, 0],
        proportional_gain_q15: [6_144; 3],
        derivative_gain_q15: [24_576; 3],
        torque_limit_q12: [12_288; 3],
        drogue_backup_time_q16: 2_000_000,
        main_backup_time_q16: 3_000_000,
        main_altitude_q12_km: 8_192,
        minimum_deployment_separation_q16: 2_048,
    }
}

fn fast(epoch: u16, frame: GlobalFrameId) -> GlobalFastSensorCell {
    GlobalFastSensorCell {
        session: 42,
        measurement_epoch: epoch,
        production_epoch: epoch,
        frame,
        validity: GLOBAL_FAST_VALID_MASK,
        mission_time_q16: u32::from(epoch) * 2_048,
        delta_velocity_q24: [0; 3],
        delta_angle_q24: [0; 3],
        attitude_vector_q15: [0; 3],
        angular_rate_q15: [0; 3],
        dynamic_pressure_q10: 0,
        mach_q12: 0,
        gimbal_applied_q15: [0; 2],
        rcs_propellant_q21: 5 << 21,
        actuator_feedback: 0,
        vehicle_status: 0,
        sensor_checksum: epoch,
    }
}

fn aid(epoch: u16, frame: GlobalFrameId, feedback: u16) -> GlobalAidFrameCell {
    GlobalAidFrameCell {
        session: 42,
        measurement_epoch: epoch,
        production_epoch: epoch,
        frame,
        validity: GLOBAL_AID_VALID_MASK,
        mission_time_q16: u32::from(epoch) * 2_048,
        barometer_q12_km: 4_096,
        gnss_position_q12_km: [0, 0, 4_096],
        gnss_velocity_q24_km_s: [0, 0, -(1 << 24)],
        attitude_q30: [Q30_ONE, 0, 0, 0],
        frame_rotation_q30: [Q30_ONE, 0, 0, 0],
        frame_omega_q24: [0; 3],
        events: 0,
        continuity: 1,
        deployment_feedback: feedback,
    }
}

fn transition(epoch: u16, from: GlobalFrameId, to: GlobalFrameId) -> GlobalTransitionCell {
    GlobalTransitionCell {
        session: 42,
        source_epoch: epoch,
        effective_epoch: epoch,
        from,
        to,
        flags: 0,
        mission_time_q16: u32::from(epoch) * 2_048,
        transform_identity: 0x11f5_7000 + u32::from(epoch),
        rotation_q30: [Q30_ONE, 0, 0, 0],
        omega_q24: [0; 3],
        pre_position_q12: [0; 3],
        post_position_q12: [0; 3],
        pre_velocity_q24: [0; 3],
        post_velocity_q24: [0; 3],
        pre_attitude_q30: [Q30_ONE, 0, 0, 0],
        post_attitude_q30: [Q30_ONE, 0, 0, 0],
        pre_rate_q24: [0; 3],
        post_rate_q24: [0; 3],
        translation_q12: [0; 3],
        velocity_bias_q24: [0; 3],
        transition_checksum: epoch.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_probe_is_deterministic_and_complete() {
        let first = run_safehold_probe();
        let second = run_safehold_probe();
        assert_eq!(first, second);
        assert_eq!(first.failures, 0);
        assert_eq!(first.releases, 16);
        assert_eq!(first.transition_count, 2);
        assert_eq!(first.final_frame, GlobalFrameId::LocalEnuV1);
        assert!(!first.safe);
        assert_ne!(phase11_safehold_probe_signature(), 0);
    }
}
