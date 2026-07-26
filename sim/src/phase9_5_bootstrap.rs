//! Strict KFB9 bootstrap for selected-finalist flight endpoints.
//!
//! This additive transport payload carries only the bounded flight and allocator
//! configuration needed by an externally paced C64 flight computer. It is not a
//! vehicle/world authority and does not replace KPE9/KPA9 evidence.

#![allow(clippy::needless_range_loop)]

use ksa64_flight::phase8_5::{LocalControlCapability, LocalFlightConfig};
use ksa64_flight::phase9_5::AdvancedFlightConfig;
use ksa64_flight::phase9_5_allocator::AdvancedAllocatorConfig;
use ksa64_interface::crc32_ieee;

pub const KFB9_CONTRACT_ID: u32 = 0x095f_b001;
pub const KFB9_LENGTH: usize = 352;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapError {
    Length,
    Magic,
    Version,
    Identity,
    Reserved,
    Checksum,
    Configuration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvancedFlightBootstrap {
    pub manifest_identity: u32,
    pub study_identity: u32,
    pub candidate_identity: u32,
    pub vehicle_identity: u32,
    pub effector_identity: u32,
    pub allocator_identity: u32,
    pub flight: AdvancedFlightConfig,
    pub allocator: AdvancedAllocatorConfig,
    pub initial_position_q13: [i32; 3],
    pub attitude_target: [i16; 3],
}

impl AdvancedFlightBootstrap {
    pub fn is_valid(&self) -> bool {
        self.manifest_identity != 0
            && self.study_identity != 0
            && self.candidate_identity != 0
            && self.vehicle_identity != 0
            && self.effector_identity != 0
            && self.allocator_identity != 0
            && self.flight.is_valid()
            && self.allocator.is_valid()
    }
}

fn p16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn pi16(out: &mut [u8], offset: usize, value: i16) {
    p16(out, offset, value as u16);
}
fn p32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn pi32(out: &mut [u8], offset: usize, value: i32) {
    p32(out, offset, value as u32);
}
fn g16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn gi16(input: &[u8], offset: usize) -> i16 {
    g16(input, offset) as i16
}
fn g32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}
fn gi32(input: &[u8], offset: usize) -> i32 {
    g32(input, offset) as i32
}

pub fn write_flight_bootstrap(
    value: &AdvancedFlightBootstrap,
    output: &mut [u8],
) -> Result<(), BootstrapError> {
    if output.len() != KFB9_LENGTH {
        return Err(BootstrapError::Length);
    }
    if !value.is_valid() {
        return Err(BootstrapError::Configuration);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"KFB9");
    p16(output, 4, 1);
    p16(output, 6, KFB9_LENGTH as u16);
    p32(output, 8, value.manifest_identity);
    p32(output, 12, value.study_identity);
    p32(output, 16, value.candidate_identity);
    p32(output, 20, value.vehicle_identity);
    p32(output, 24, value.effector_identity);
    p32(output, 28, value.allocator_identity);

    let local = value.flight.local;
    p16(output, 32, local.session);
    output[34] = match local.capability {
        LocalControlCapability::MonitorOnly => 1,
        LocalControlCapability::TwoAxisGimbal => 2,
    };
    pi32(output, 36, local.minimum_arming_time_q18);
    pi32(output, 40, local.minimum_arming_altitude_q13);
    pi32(output, 44, local.burnout_qualification_time_q18);
    pi32(output, 48, local.drogue_backup_time_q18);
    pi32(output, 52, local.main_backup_time_q18);
    pi32(output, 56, local.main_altitude_q13);
    pi32(output, 60, local.minimum_deployment_separation_q18);
    pi16(output, 64, local.proportional_gain_q15);
    pi16(output, 66, local.derivative_gain_q15);
    pi16(output, 68, local.gimbal_limit_q15);
    pi16(output, 72, value.flight.roll_proportional_gain_q15);
    pi16(output, 74, value.flight.roll_derivative_gain_q15);
    for axis in 0..3 {
        pi32(output, 76 + axis * 4, value.flight.torque_limit_q12[axis]);
    }
    pi32(output, 88, value.flight.fallback_density_upper_q10);
    pi32(output, 92, value.flight.maximum_wind_q19);
    p16(output, 96, value.flight.minimum_sound_speed_mps);
    p16(output, 98, value.flight.maximum_navigation_speed_mps);
    pi32(output, 100, value.flight.propellant_wet_q21);
    p16(output, 104, value.flight.reserve_q15);

    let allocator = &value.allocator;
    output[108..111].copy_from_slice(&allocator.priorities);
    output[111] = u8::from(allocator.has_gimbal)
        | (u8::from(allocator.has_canards) << 1)
        | (u8::from(allocator.has_rcs) << 2);
    pi32(output, 112, allocator.canard_enable_q10);
    pi32(output, 116, allocator.canard_full_q10);
    pi32(output, 120, allocator.canard_disable_q10);
    p16(output, 124, allocator.reserve_q15);
    pi32(output, 128, allocator.propellant_wet_q21);
    for axis in 0..3 {
        for group in 0..3 {
            pi32(
                output,
                132 + (axis * 3 + group) * 4,
                allocator.group_authority_q12[axis][group],
            );
        }
    }
    for axis in 0..3 {
        for channel in 0..2 {
            pi16(
                output,
                168 + (axis * 2 + channel) * 2,
                allocator.gimbal_mix_q15[axis][channel],
            );
        }
    }
    for axis in 0..3 {
        for channel in 0..4 {
            pi16(
                output,
                180 + (axis * 4 + channel) * 2,
                allocator.canard_mix_q15[axis][channel],
            );
        }
    }
    for axis in 0..3 {
        for channel in 0..12 {
            pi16(
                output,
                204 + (axis * 12 + channel) * 2,
                allocator.rcs_mix_q15[axis][channel],
            );
        }
    }
    for channel in 0..2 {
        pi16(
            output,
            276 + channel * 2,
            allocator.gimbal_limit_turn16[channel],
        );
    }
    for channel in 0..4 {
        pi16(
            output,
            280 + channel * 2,
            allocator.canard_limit_turn16[channel],
        );
    }
    output[288..300].copy_from_slice(&allocator.rcs_max_quanta);
    for axis in 0..3 {
        pi32(output, 300 + axis * 4, value.initial_position_q13[axis]);
        pi16(output, 312 + axis * 2, value.attitude_target[axis]);
    }
    p32(output, 348, crc32_ieee(&output[..348]));
    Ok(())
}

pub fn parse_flight_bootstrap(input: &[u8]) -> Result<AdvancedFlightBootstrap, BootstrapError> {
    if input.len() != KFB9_LENGTH {
        return Err(BootstrapError::Length);
    }
    if &input[..4] != b"KFB9" {
        return Err(BootstrapError::Magic);
    }
    if g16(input, 4) != 1 || g16(input, 6) as usize != KFB9_LENGTH {
        return Err(BootstrapError::Version);
    }
    if input[35] != 0
        || input[70..72].iter().any(|byte| *byte != 0)
        || input[106..108].iter().any(|byte| *byte != 0)
        || input[318..348].iter().any(|byte| *byte != 0)
    {
        return Err(BootstrapError::Reserved);
    }
    if g32(input, 348) != crc32_ieee(&input[..348]) {
        return Err(BootstrapError::Checksum);
    }
    let capability = match input[34] {
        1 => LocalControlCapability::MonitorOnly,
        2 => LocalControlCapability::TwoAxisGimbal,
        _ => return Err(BootstrapError::Configuration),
    };
    let flags = input[111];
    if flags & !7 != 0 {
        return Err(BootstrapError::Configuration);
    }
    let local = LocalFlightConfig {
        session: g16(input, 32),
        capability,
        minimum_arming_time_q18: gi32(input, 36),
        minimum_arming_altitude_q13: gi32(input, 40),
        burnout_qualification_time_q18: gi32(input, 44),
        drogue_backup_time_q18: gi32(input, 48),
        main_backup_time_q18: gi32(input, 52),
        main_altitude_q13: gi32(input, 56),
        minimum_deployment_separation_q18: gi32(input, 60),
        proportional_gain_q15: gi16(input, 64),
        derivative_gain_q15: gi16(input, 66),
        gimbal_limit_q15: gi16(input, 68),
    };
    let mut torque_limit_q12 = [0; 3];
    for axis in 0..3 {
        torque_limit_q12[axis] = gi32(input, 76 + axis * 4);
    }
    let flight = AdvancedFlightConfig {
        local,
        roll_proportional_gain_q15: gi16(input, 72),
        roll_derivative_gain_q15: gi16(input, 74),
        torque_limit_q12,
        fallback_density_upper_q10: gi32(input, 88),
        maximum_wind_q19: gi32(input, 92),
        minimum_sound_speed_mps: g16(input, 96),
        maximum_navigation_speed_mps: g16(input, 98),
        propellant_wet_q21: gi32(input, 100),
        reserve_q15: g16(input, 104),
    };
    let mut group_authority_q12 = [[0; 3]; 3];
    let mut gimbal_mix_q15 = [[0; 2]; 3];
    let mut canard_mix_q15 = [[0; 4]; 3];
    let mut rcs_mix_q15 = [[0; 12]; 3];
    for axis in 0..3 {
        for group in 0..3 {
            group_authority_q12[axis][group] = gi32(input, 132 + (axis * 3 + group) * 4);
        }
        for channel in 0..2 {
            gimbal_mix_q15[axis][channel] = gi16(input, 168 + (axis * 2 + channel) * 2);
        }
        for channel in 0..4 {
            canard_mix_q15[axis][channel] = gi16(input, 180 + (axis * 4 + channel) * 2);
        }
        for channel in 0..12 {
            rcs_mix_q15[axis][channel] = gi16(input, 204 + (axis * 12 + channel) * 2);
        }
    }
    let mut gimbal_limit_turn16 = [0; 2];
    let mut canard_limit_turn16 = [0; 4];
    let mut rcs_max_quanta = [0; 12];
    for channel in 0..2 {
        gimbal_limit_turn16[channel] = gi16(input, 276 + channel * 2);
    }
    for channel in 0..4 {
        canard_limit_turn16[channel] = gi16(input, 280 + channel * 2);
    }
    rcs_max_quanta.copy_from_slice(&input[288..300]);
    let allocator = AdvancedAllocatorConfig {
        priorities: [input[108], input[109], input[110]],
        canard_enable_q10: gi32(input, 112),
        canard_full_q10: gi32(input, 116),
        canard_disable_q10: gi32(input, 120),
        reserve_q15: g16(input, 124),
        propellant_wet_q21: gi32(input, 128),
        group_authority_q12,
        gimbal_mix_q15,
        canard_mix_q15,
        rcs_mix_q15,
        gimbal_limit_turn16,
        canard_limit_turn16,
        rcs_max_quanta,
        has_gimbal: flags & 1 != 0,
        has_canards: flags & 2 != 0,
        has_rcs: flags & 4 != 0,
    };
    let mut initial_position_q13 = [0; 3];
    let mut attitude_target = [0; 3];
    for axis in 0..3 {
        initial_position_q13[axis] = gi32(input, 300 + axis * 4);
        attitude_target[axis] = gi16(input, 312 + axis * 2);
    }
    let value = AdvancedFlightBootstrap {
        manifest_identity: g32(input, 8),
        study_identity: g32(input, 12),
        candidate_identity: g32(input, 16),
        vehicle_identity: g32(input, 20),
        effector_identity: g32(input, 24),
        allocator_identity: g32(input, 28),
        flight,
        allocator,
        initial_position_q13,
        attitude_target,
    };
    if !value.is_valid() {
        return Err(BootstrapError::Configuration);
    }
    Ok(value)
}

#[cfg(all(test, feature = "fixtures"))]
mod tests {
    use super::*;
    use crate::phase9_5::{reference_mixed_allocator_config, reference_mixed_flight_config};

    fn fixture() -> AdvancedFlightBootstrap {
        AdvancedFlightBootstrap {
            manifest_identity: 1,
            study_identity: 2,
            candidate_identity: 3,
            vehicle_identity: 4,
            effector_identity: 5,
            allocator_identity: 6,
            flight: reference_mixed_flight_config().unwrap(),
            allocator: reference_mixed_allocator_config(),
            initial_position_q13: [7, -8, 9],
            attitude_target: [10, -11, 12],
        }
    }

    #[test]
    fn strict_bootstrap_roundtrip_and_corruption() {
        let value = fixture();
        let mut bytes = [0; KFB9_LENGTH];
        write_flight_bootstrap(&value, &mut bytes).unwrap();
        assert_eq!(parse_flight_bootstrap(&bytes).unwrap(), value);
        let mut bad = bytes;
        bad[200] ^= 1;
        assert_eq!(parse_flight_bootstrap(&bad), Err(BootstrapError::Checksum));
        let mut reserved = bytes;
        reserved[330] = 1;
        let crc = crc32_ieee(&reserved[..348]);
        p32(&mut reserved, 348, crc);
        assert_eq!(
            parse_flight_bootstrap(&reserved),
            Err(BootstrapError::Reserved)
        );
    }
}
