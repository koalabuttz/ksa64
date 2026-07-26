//! Strict Phase 10 global vehicle and mission packs.

use crate::evaluation::ModelProfileId;
use crate::phase10_contract::{ReferenceFrameId, PHASE10_CONTRACT_ID};
use crate::scenario::crc32_ieee;

pub const KGV10_LENGTH: usize = 2_048;
pub const KGM10_LENGTH: usize = 1_024;
pub const GLOBAL_AERO_MAX_KNOTS: usize = 16;
pub const GLOBAL_PITCH_MAX_KNOTS: usize = 16;
const HEADER: usize = 64;
const VERSION: u16 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalPackError {
    Length,
    Magic,
    Version,
    Kind,
    Contract,
    Identity,
    Reserved,
    Checksum,
    Profile,
    Range,
    Ordering,
    Reference,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct GlobalAeroKnot {
    pub mach_q24: i32,
    pub axial_cd_q24: i32,
    pub cp_from_nose_q28_m: i32,
    pub normal_force_slope_q24: i32,
    pub pitch_damping_q24: i32,
    pub yaw_damping_q24: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GlobalVehiclePack {
    pub identity: u32,
    pub source_identity: u32,
    pub dry_mass_q21_kg: i32,
    pub main_propellant_q21_kg: i32,
    pub rcs_propellant_q21_kg: i32,
    pub length_q13_m: i32,
    pub diameter_q13_m: i32,
    pub reference_area_q29_m2: i32,
    pub wet_cg_q28_m: i32,
    pub dry_cg_q28_m: i32,
    pub wet_inertia_q19: [i32; 3],
    pub dry_inertia_q19: [i32; 3],
    pub thrust_q13_n: i32,
    pub main_mass_flow_q21_kg_s: i32,
    pub burn_time_q16_s: u32,
    pub gimbal_limit_turn16: i16,
    pub gimbal_slew_turn16_per_release: i16,
    pub rcs_jet_count: u8,
    pub rcs_nominal_thrust_q13_n: i32,
    pub rcs_isp_q16_s: i32,
    pub rcs_reserve_q16: u16,
    pub drogue_cda_q24_m2: i32,
    pub main_cda_q24_m2: i32,
    pub aero_count: u8,
    pub aero: [GlobalAeroKnot; GLOBAL_AERO_MAX_KNOTS],
}

impl GlobalVehiclePack {
    pub fn validate(&self) -> Result<(), GlobalPackError> {
        if self.identity == 0
            || self.source_identity == 0
            || self.dry_mass_q21_kg <= 0
            || self.main_propellant_q21_kg <= 0
            || self.rcs_propellant_q21_kg <= 0
            || self.length_q13_m <= 0
            || self.diameter_q13_m <= 0
            || self.diameter_q13_m >= self.length_q13_m
            || self.reference_area_q29_m2 <= 0
            || self.wet_cg_q28_m <= 0
            || self.dry_cg_q28_m <= 0
            || self.thrust_q13_n <= 0
            || self.main_mass_flow_q21_kg_s <= 0
            || self.burn_time_q16_s == 0
            || self.gimbal_limit_turn16 <= 0
            || self.gimbal_slew_turn16_per_release <= 0
            || self.rcs_jet_count != 12
            || self.rcs_nominal_thrust_q13_n <= 0
            || self.rcs_isp_q16_s <= 0
            || self.rcs_reserve_q16 == 0
            || self.drogue_cda_q24_m2 <= 0
            || self.main_cda_q24_m2 <= self.drogue_cda_q24_m2
            || self.aero_count < 2
            || self.aero_count as usize > GLOBAL_AERO_MAX_KNOTS
            || self
                .wet_inertia_q19
                .iter()
                .chain(self.dry_inertia_q19.iter())
                .any(|value| *value <= 0)
        {
            return Err(GlobalPackError::Range);
        }
        let wet_mass = self.dry_mass_q21_kg as i64
            + self.main_propellant_q21_kg as i64
            + self.rcs_propellant_q21_kg as i64;
        if wet_mass > (800i64 << 21)
            || self.wet_cg_q28_m as i64 > (self.length_q13_m as i64) << 15
            || self.dry_cg_q28_m as i64 > (self.length_q13_m as i64) << 15
        {
            return Err(GlobalPackError::Range);
        }
        let mut previous = -1;
        for knot in &self.aero[..self.aero_count as usize] {
            if knot.mach_q24 <= previous
                || knot.axial_cd_q24 <= 0
                || knot.cp_from_nose_q28_m <= 0
                || knot.normal_force_slope_q24 <= 0
                || knot.pitch_damping_q24 <= 0
                || knot.yaw_damping_q24 <= 0
            {
                return Err(GlobalPackError::Ordering);
            }
            previous = knot.mach_q24;
        }
        if self.aero[self.aero_count as usize..]
            .iter()
            .any(|knot| *knot != GlobalAeroKnot::default())
        {
            return Err(GlobalPackError::Reserved);
        }
        Ok(())
    }

    pub fn wet_mass_q21(self) -> i32 {
        self.dry_mass_q21_kg
            .saturating_add(self.main_propellant_q21_kg)
            .saturating_add(self.rcs_propellant_q21_kg)
    }

    pub fn encode(&self, output: &mut [u8]) -> Result<(), GlobalPackError> {
        self.validate()?;
        write_header(output, Kind::Vehicle, self.identity)?;
        p32(output, 32, self.source_identity);
        output[36] = ModelProfileId::GlobalEcef6DofV1 as u8;
        pi32(output, 40, self.dry_mass_q21_kg);
        pi32(output, 44, self.main_propellant_q21_kg);
        pi32(output, 48, self.rcs_propellant_q21_kg);
        pi32(output, 52, self.length_q13_m);
        pi32(output, 56, self.diameter_q13_m);
        pi32(output, 64, self.reference_area_q29_m2);
        pi32(output, 68, self.wet_cg_q28_m);
        pi32(output, 72, self.dry_cg_q28_m);
        for axis in 0..3 {
            pi32(output, 76 + axis * 4, self.wet_inertia_q19[axis]);
            pi32(output, 88 + axis * 4, self.dry_inertia_q19[axis]);
        }
        pi32(output, 100, self.thrust_q13_n);
        pi32(output, 104, self.main_mass_flow_q21_kg_s);
        p32(output, 108, self.burn_time_q16_s);
        pi16(output, 112, self.gimbal_limit_turn16);
        pi16(output, 114, self.gimbal_slew_turn16_per_release);
        output[116] = self.rcs_jet_count;
        output[117] = self.aero_count;
        p16(output, 118, self.rcs_reserve_q16);
        pi32(output, 120, self.rcs_nominal_thrust_q13_n);
        pi32(output, 124, self.rcs_isp_q16_s);
        pi32(output, 128, self.drogue_cda_q24_m2);
        pi32(output, 132, self.main_cda_q24_m2);
        for (index, knot) in self.aero.iter().enumerate() {
            let at = 256 + index * 24;
            pi32(output, at, knot.mach_q24);
            pi32(output, at + 4, knot.axial_cd_q24);
            pi32(output, at + 8, knot.cp_from_nose_q28_m);
            pi32(output, at + 12, knot.normal_force_slope_q24);
            pi32(output, at + 16, knot.pitch_damping_q24);
            pi32(output, at + 20, knot.yaw_damping_q24);
        }
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalPackError> {
        let identity = validate_record(bytes, Kind::Vehicle)?;
        if bytes[36] != ModelProfileId::GlobalEcef6DofV1 as u8
            || bytes[37..40].iter().any(|byte| *byte != 0)
            || bytes[60..64].iter().any(|byte| *byte != 0)
            || bytes[136..256].iter().any(|byte| *byte != 0)
            || bytes[640..KGV10_LENGTH - 4].iter().any(|byte| *byte != 0)
        {
            return Err(GlobalPackError::Reserved);
        }
        let count = bytes[117];
        if count > GLOBAL_AERO_MAX_KNOTS as u8 {
            return Err(GlobalPackError::Range);
        }
        let mut aero = [GlobalAeroKnot::default(); GLOBAL_AERO_MAX_KNOTS];
        for (index, knot) in aero.iter_mut().enumerate() {
            let at = 256 + index * 24;
            *knot = GlobalAeroKnot {
                mach_q24: gi32(bytes, at),
                axial_cd_q24: gi32(bytes, at + 4),
                cp_from_nose_q28_m: gi32(bytes, at + 8),
                normal_force_slope_q24: gi32(bytes, at + 12),
                pitch_damping_q24: gi32(bytes, at + 16),
                yaw_damping_q24: gi32(bytes, at + 20),
            };
        }
        let mut wet_inertia = [0; 3];
        let mut dry_inertia = [0; 3];
        for axis in 0..3 {
            wet_inertia[axis] = gi32(bytes, 76 + axis * 4);
            dry_inertia[axis] = gi32(bytes, 88 + axis * 4);
        }
        let pack = Self {
            identity,
            source_identity: g32(bytes, 32),
            dry_mass_q21_kg: gi32(bytes, 40),
            main_propellant_q21_kg: gi32(bytes, 44),
            rcs_propellant_q21_kg: gi32(bytes, 48),
            length_q13_m: gi32(bytes, 52),
            diameter_q13_m: gi32(bytes, 56),
            reference_area_q29_m2: gi32(bytes, 64),
            wet_cg_q28_m: gi32(bytes, 68),
            dry_cg_q28_m: gi32(bytes, 72),
            wet_inertia_q19: wet_inertia,
            dry_inertia_q19: dry_inertia,
            thrust_q13_n: gi32(bytes, 100),
            main_mass_flow_q21_kg_s: gi32(bytes, 104),
            burn_time_q16_s: g32(bytes, 108),
            gimbal_limit_turn16: gi16(bytes, 112),
            gimbal_slew_turn16_per_release: gi16(bytes, 114),
            rcs_jet_count: bytes[116],
            rcs_nominal_thrust_q13_n: gi32(bytes, 120),
            rcs_isp_q16_s: gi32(bytes, 124),
            rcs_reserve_q16: g16(bytes, 118),
            drogue_cda_q24_m2: gi32(bytes, 128),
            main_cda_q24_m2: gi32(bytes, 132),
            aero_count: count,
            aero,
        };
        pack.validate()?;
        Ok(pack)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct PitchKnot {
    pub time_q16_s: u32,
    pub elevation_q28_rad: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct GlobalMissionPack {
    pub identity: u32,
    pub source_identity: u32,
    pub earth_identity: u32,
    pub transform_identity: u32,
    pub atmosphere_identity: u32,
    pub vehicle_identity: u32,
    pub launch_latitude_q28_rad: i32,
    pub launch_longitude_q28_rad: i32,
    pub launch_height_q12_km: i32,
    pub launch_azimuth_q28_rad: i32,
    pub launch_elevation_q28_rad: i32,
    pub rail_length_q13_m: i32,
    pub recovery_latitude_q28_rad: i32,
    pub recovery_longitude_q28_rad: i32,
    pub recovery_height_q12_km: i32,
    pub eci_transition_altitude_q12_km: i32,
    pub entry_transition_altitude_q12_km: i32,
    pub recovery_transition_altitude_q12_km: i32,
    pub recovery_radius_q12_km: i32,
    pub transition_dynamic_pressure_q14_pa: i32,
    pub recovery_transition_mach_q24: i32,
    pub main_deployment_altitude_q12_km: i32,
    pub entry_drag_area_scale_q16: i32,
    pub max_mission_time_q16_s: u32,
    pub pitch_count: u8,
    pub pitch: [PitchKnot; GLOBAL_PITCH_MAX_KNOTS],
}

impl GlobalMissionPack {
    pub fn validate(&self) -> Result<(), GlobalPackError> {
        if self.identity == 0
            || self.source_identity == 0
            || self.earth_identity == 0
            || self.transform_identity == 0
            || self.atmosphere_identity == 0
            || self.vehicle_identity == 0
            || self.launch_latitude_q28_rad.abs() > 421_657_428
            || self.recovery_latitude_q28_rad.abs() > 421_657_428
            || self.launch_longitude_q28_rad.abs() > 843_314_857
            || self.recovery_longitude_q28_rad.abs() > 843_314_857
            || self.launch_elevation_q28_rad <= 0
            || self.rail_length_q13_m <= 0
            || self.eci_transition_altitude_q12_km != 120 << 12
            || self.entry_transition_altitude_q12_km != 120 << 12
            || self.recovery_transition_altitude_q12_km != 20 << 12
            || self.recovery_radius_q12_km != 200 << 12
            || self.transition_dynamic_pressure_q14_pa != 1 << 14
            || self.recovery_transition_mach_q24 != 13_421_773
            || self.main_deployment_altitude_q12_km <= 0
            || self.entry_drag_area_scale_q16 < 1 << 16
            || self.entry_drag_area_scale_q16 > 8 << 16
            || self.max_mission_time_q16_s == 0
            || self.max_mission_time_q16_s > 2_700 << 16
            || self.pitch_count < 2
            || self.pitch_count as usize > GLOBAL_PITCH_MAX_KNOTS
        {
            return Err(GlobalPackError::Range);
        }
        let mut previous = 0;
        for (index, knot) in self.pitch[..self.pitch_count as usize].iter().enumerate() {
            if (index > 0 && knot.time_q16_s <= previous)
                || knot.elevation_q28_rad <= 0
                || knot.elevation_q28_rad > 421_657_428
            {
                return Err(GlobalPackError::Ordering);
            }
            previous = knot.time_q16_s;
        }
        if self.pitch[self.pitch_count as usize..]
            .iter()
            .any(|knot| *knot != PitchKnot::default())
        {
            return Err(GlobalPackError::Reserved);
        }
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> Result<(), GlobalPackError> {
        self.validate()?;
        write_header(output, Kind::Mission, self.identity)?;
        p32(output, 32, self.source_identity);
        output[36] = ModelProfileId::GlobalEcef6DofV1 as u8;
        output[37] = ReferenceFrameId::LocalEnuV1 as u8;
        output[38] = ReferenceFrameId::EarthFixedEcefV1 as u8;
        output[39] = ReferenceFrameId::EarthInertialEciV1 as u8;
        p32(output, 40, self.earth_identity);
        p32(output, 44, self.transform_identity);
        p32(output, 48, self.atmosphere_identity);
        p32(output, 52, self.vehicle_identity);
        pi32(output, 64, self.launch_latitude_q28_rad);
        pi32(output, 68, self.launch_longitude_q28_rad);
        pi32(output, 72, self.launch_height_q12_km);
        pi32(output, 76, self.launch_azimuth_q28_rad);
        pi32(output, 80, self.launch_elevation_q28_rad);
        pi32(output, 84, self.rail_length_q13_m);
        pi32(output, 88, self.recovery_latitude_q28_rad);
        pi32(output, 92, self.recovery_longitude_q28_rad);
        pi32(output, 96, self.recovery_height_q12_km);
        pi32(output, 100, self.eci_transition_altitude_q12_km);
        pi32(output, 104, self.entry_transition_altitude_q12_km);
        pi32(output, 108, self.recovery_transition_altitude_q12_km);
        pi32(output, 112, self.recovery_radius_q12_km);
        pi32(output, 116, self.transition_dynamic_pressure_q14_pa);
        pi32(output, 120, self.recovery_transition_mach_q24);
        pi32(output, 124, self.main_deployment_altitude_q12_km);
        p32(output, 128, self.max_mission_time_q16_s);
        output[132] = self.pitch_count;
        pi32(output, 136, self.entry_drag_area_scale_q16);
        for (index, knot) in self.pitch.iter().enumerate() {
            let at = 256 + index * 8;
            p32(output, at, knot.time_q16_s);
            pi32(output, at + 4, knot.elevation_q28_rad);
        }
        seal(output);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, GlobalPackError> {
        let identity = validate_record(bytes, Kind::Mission)?;
        if bytes[36] != ModelProfileId::GlobalEcef6DofV1 as u8
            || bytes[37] != ReferenceFrameId::LocalEnuV1 as u8
            || bytes[38] != ReferenceFrameId::EarthFixedEcefV1 as u8
            || bytes[39] != ReferenceFrameId::EarthInertialEciV1 as u8
            || bytes[56..64].iter().any(|byte| *byte != 0)
            || bytes[133..136].iter().any(|byte| *byte != 0)
            || bytes[140..256].iter().any(|byte| *byte != 0)
            || bytes[384..KGM10_LENGTH - 4].iter().any(|byte| *byte != 0)
        {
            return Err(GlobalPackError::Reserved);
        }
        let count = bytes[132];
        if count > GLOBAL_PITCH_MAX_KNOTS as u8 {
            return Err(GlobalPackError::Range);
        }
        let mut pitch = [PitchKnot::default(); GLOBAL_PITCH_MAX_KNOTS];
        for (index, knot) in pitch.iter_mut().enumerate() {
            let at = 256 + index * 8;
            *knot = PitchKnot {
                time_q16_s: g32(bytes, at),
                elevation_q28_rad: gi32(bytes, at + 4),
            };
        }
        let pack = Self {
            identity,
            source_identity: g32(bytes, 32),
            earth_identity: g32(bytes, 40),
            transform_identity: g32(bytes, 44),
            atmosphere_identity: g32(bytes, 48),
            vehicle_identity: g32(bytes, 52),
            launch_latitude_q28_rad: gi32(bytes, 64),
            launch_longitude_q28_rad: gi32(bytes, 68),
            launch_height_q12_km: gi32(bytes, 72),
            launch_azimuth_q28_rad: gi32(bytes, 76),
            launch_elevation_q28_rad: gi32(bytes, 80),
            rail_length_q13_m: gi32(bytes, 84),
            recovery_latitude_q28_rad: gi32(bytes, 88),
            recovery_longitude_q28_rad: gi32(bytes, 92),
            recovery_height_q12_km: gi32(bytes, 96),
            eci_transition_altitude_q12_km: gi32(bytes, 100),
            entry_transition_altitude_q12_km: gi32(bytes, 104),
            recovery_transition_altitude_q12_km: gi32(bytes, 108),
            recovery_radius_q12_km: gi32(bytes, 112),
            transition_dynamic_pressure_q14_pa: gi32(bytes, 116),
            recovery_transition_mach_q24: gi32(bytes, 120),
            main_deployment_altitude_q12_km: gi32(bytes, 124),
            entry_drag_area_scale_q16: gi32(bytes, 136),
            max_mission_time_q16_s: g32(bytes, 128),
            pitch_count: count,
            pitch,
        };
        pack.validate()?;
        Ok(pack)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
enum Kind {
    Vehicle = 4,
    Mission = 5,
}

impl Kind {
    const fn magic(self) -> [u8; 5] {
        match self {
            Self::Vehicle => *b"KGV10",
            Self::Mission => *b"KGM10",
        }
    }
    const fn length(self) -> usize {
        match self {
            Self::Vehicle => KGV10_LENGTH,
            Self::Mission => KGM10_LENGTH,
        }
    }
}

fn write_header(output: &mut [u8], kind: Kind, identity: u32) -> Result<(), GlobalPackError> {
    if output.len() != kind.length() {
        return Err(GlobalPackError::Length);
    }
    if identity == 0 {
        return Err(GlobalPackError::Identity);
    }
    output.fill(0);
    output[..5].copy_from_slice(&kind.magic());
    p16(output, 6, VERSION);
    p16(output, 8, HEADER as u16);
    p16(output, 10, kind as u16);
    p32(output, 12, output.len() as u32);
    p32(output, 16, PHASE10_CONTRACT_ID);
    p32(output, 20, identity);
    Ok(())
}

fn validate_record(bytes: &[u8], kind: Kind) -> Result<u32, GlobalPackError> {
    if bytes.len() != kind.length() {
        return Err(GlobalPackError::Length);
    }
    if bytes[..5] != kind.magic() || bytes[5] != 0 {
        return Err(GlobalPackError::Magic);
    }
    if g16(bytes, 6) != VERSION || g16(bytes, 8) as usize != HEADER {
        return Err(GlobalPackError::Version);
    }
    if g16(bytes, 10) != kind as u16 || g32(bytes, 12) as usize != kind.length() {
        return Err(GlobalPackError::Kind);
    }
    if g32(bytes, 16) != PHASE10_CONTRACT_ID {
        return Err(GlobalPackError::Contract);
    }
    if bytes[24..32].iter().any(|byte| *byte != 0) {
        return Err(GlobalPackError::Reserved);
    }
    let identity = g32(bytes, 20);
    if identity == 0 {
        return Err(GlobalPackError::Identity);
    }
    let at = bytes.len() - 4;
    if g32(bytes, at) != crc32_ieee(&bytes[..at]) {
        return Err(GlobalPackError::Checksum);
    }
    Ok(identity)
}

fn seal(output: &mut [u8]) {
    let at = output.len() - 4;
    p32(output, at, crc32_ieee(&output[..at]));
}

fn p16(output: &mut [u8], at: usize, value: u16) {
    output[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn pi16(output: &mut [u8], at: usize, value: i16) {
    p16(output, at, value as u16);
}
fn p32(output: &mut [u8], at: usize, value: u32) {
    output[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn pi32(output: &mut [u8], at: usize, value: i32) {
    p32(output, at, value as u32);
}
fn g16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn gi16(bytes: &[u8], at: usize) -> i16 {
    g16(bytes, at) as i16
}
fn g32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
fn gi32(bytes: &[u8], at: usize) -> i32 {
    g32(bytes, at) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_global_packs_are_strict_and_bound() {
        let vehicle =
            GlobalVehiclePack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgv10"))
                .unwrap();
        let mission =
            GlobalMissionPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kgm10"))
                .unwrap();
        assert_eq!(mission.vehicle_identity, vehicle.identity);
        assert_eq!(vehicle.wet_mass_q21(), 500 << 21);
        assert_eq!(vehicle.aero_count, 8);
        assert_eq!(mission.pitch_count, 8);
    }

    #[test]
    fn corruption_and_reserved_bytes_fail_closed() {
        let source = include_bytes!("../../phase10/generated/ksa-g10r.kgv10");
        let mut bytes = *source;
        bytes[1_000] = 1;
        seal(&mut bytes);
        assert_eq!(
            GlobalVehiclePack::decode(&bytes),
            Err(GlobalPackError::Reserved)
        );
    }
}
