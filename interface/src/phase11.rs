//! Phase 11 mission-operations contracts.
//!
//! KLR10 remains the global flight wire ABI. These records describe which
//! implementation is allowed to consume it and add bounded operational
//! objects without giving ground software direct actuator authority.

use crate::{crc32_ieee, CodecError};

pub const KFS11_LENGTH: usize = 512;
pub const KFS11_MAGIC: [u8; 4] = *b"KFS1";
pub const KFS11_VERSION: u16 = 1;

pub const PACKAGE_CAP_MISSION_PLAN: u32 = 1 << 0;
pub const PACKAGE_CAP_GROUND_NAV_UPDATE: u32 = 1 << 1;
pub const PACKAGE_CAP_TARGET_UPDATE: u32 = 1 << 2;
pub const PACKAGE_CAP_BRANCH_SELECT: u32 = 1 << 3;
pub const PACKAGE_CAP_NAV_MODE: u32 = 1 << 4;
pub const PACKAGE_CAP_HIGH_LEVEL_MODE: u32 = 1 << 5;
pub const PACKAGE_CAP_PREDICTION: u32 = 1 << 6;
pub const PACKAGE_CAP_EVENT_JOURNAL: u32 = 1 << 7;
pub const PACKAGE_CAP_MASK: u32 = (1 << 8) - 1;

pub const PACKAGE_TARGET_HOST: u8 = 1 << 0;
pub const PACKAGE_TARGET_RUST_MOS: u8 = 1 << 1;
pub const PACKAGE_TARGET_EXTERNAL: u8 = 1 << 2;
pub const PACKAGE_TARGET_MASK: u8 = (1 << 3) - 1;

pub const PACKAGE_SEGMENT_LOCAL_LAUNCH: u16 = 1 << 0;
pub const PACKAGE_SEGMENT_ECEF_ASCENT: u16 = 1 << 1;
pub const PACKAGE_SEGMENT_ECI_COAST: u16 = 1 << 2;
pub const PACKAGE_SEGMENT_ECEF_ENTRY: u16 = 1 << 3;
pub const PACKAGE_SEGMENT_LOCAL_RECOVERY: u16 = 1 << 4;
pub const PACKAGE_SEGMENT_MASK: u16 = (1 << 5) - 1;

pub const PACKAGE_LOAD_GROUND_NAV: u16 = 1 << 0;
pub const PACKAGE_LOAD_EVENT_TARGET: u16 = 1 << 1;
pub const PACKAGE_LOAD_BRANCH: u16 = 1 << 2;
pub const PACKAGE_LOAD_NAV_MODE: u16 = 1 << 3;
pub const PACKAGE_LOAD_HIGH_LEVEL_MODE: u16 = 1 << 4;
pub const PACKAGE_LOAD_MASK: u16 = (1 << 5) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FlightSoftwarePackageId {
    KsaG10rReferenceOpsV1 = 0x11f5_0001,
    SafeholdRecoveryV1 = 0x11f5_0002,
}

impl FlightSoftwarePackageId {
    fn parse(value: u32) -> Result<Self, CodecError> {
        match value {
            0x11f5_0001 => Ok(Self::KsaG10rReferenceOpsV1),
            0x11f5_0002 => Ok(Self::SafeholdRecoveryV1),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FlightAbiId {
    GlobalKlr10V1 = 0x1052_0001,
}

impl FlightAbiId {
    fn parse(value: u32) -> Result<Self, CodecError> {
        match value {
            0x1052_0001 => Ok(Self::GlobalKlr10V1),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PackageSafeStateId {
    ReferenceGlobalSafeV1 = 1,
    EntryRecoverySafeholdV1 = 2,
}

impl PackageSafeStateId {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::ReferenceGlobalSafeV1),
            2 => Ok(Self::EntryRecoverySafeholdV1),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PackageCommandLossBehavior {
    FrozenKlr10HoldThenSafe = 1,
    ImmediateSafehold = 2,
}

impl PackageCommandLossBehavior {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::FrozenKlr10HoldThenSafe),
            2 => Ok(Self::ImmediateSafehold),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PackageTarget {
    Host = PACKAGE_TARGET_HOST,
    RustMos = PACKAGE_TARGET_RUST_MOS,
    ExternalEndpoint = PACKAGE_TARGET_EXTERNAL,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum PackageSegmentSupport {
    LocalLaunch = PACKAGE_SEGMENT_LOCAL_LAUNCH,
    EcefAscent = PACKAGE_SEGMENT_ECEF_ASCENT,
    EciCoast = PACKAGE_SEGMENT_ECI_COAST,
    EcefEntry = PACKAGE_SEGMENT_ECEF_ENTRY,
    LocalRecovery = PACKAGE_SEGMENT_LOCAL_RECOVERY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackageResourceClaim {
    pub persistent_bytes: u16,
    pub transient_bytes: u16,
    pub stack_bytes: u16,
    pub journal_records: u8,
    pub maximum_object_bytes: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightSoftwarePackageManifest {
    pub manifest_identity: u32,
    pub package: FlightSoftwarePackageId,
    pub implementation_identity: u32,
    pub abi: FlightAbiId,
    pub vehicle_profile_identity: u32,
    pub mission_compatibility_identity: u32,
    pub capabilities: u32,
    pub segment_support: u16,
    pub command_load_support: u16,
    pub targets: u8,
    pub safe_state: PackageSafeStateId,
    pub command_loss: PackageCommandLossBehavior,
    pub resource: PackageResourceClaim,
    pub fast_hz: u8,
    pub navigation_hz: u8,
    pub guidance_hz: u8,
    pub maximum_plan_events: u8,
    pub maximum_branches: u8,
    pub maximum_decisions: u8,
    pub code_identity: u32,
    pub configuration_identity: u32,
    pub resource_evidence_sha256: [u8; 32],
}

fn p16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn p32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn g16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn g32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn validate_manifest(value: &FlightSoftwarePackageManifest) -> Result<(), CodecError> {
    if value.manifest_identity == 0
        || value.implementation_identity == 0
        || value.vehicle_profile_identity == 0
        || value.mission_compatibility_identity == 0
        || value.code_identity == 0
        || value.configuration_identity == 0
        || value.capabilities == 0
        || value.capabilities & !PACKAGE_CAP_MASK != 0
        || value.segment_support == 0
        || value.segment_support & !PACKAGE_SEGMENT_MASK != 0
        || value.command_load_support & !PACKAGE_LOAD_MASK != 0
        || value.targets == 0
        || value.targets & !PACKAGE_TARGET_MASK != 0
        || value.fast_hz != 32
        || value.navigation_hz != 8
        || value.guidance_hz != 1
        || value.maximum_plan_events > 24
        || value.maximum_branches > 8
        || value.maximum_decisions > 8
        || value.resource.persistent_bytes == 0
        || value.resource.transient_bytes == 0
        || value.resource.stack_bytes == 0
        || value.resource.journal_records == 0
        || value.resource.maximum_object_bytes == 0
        || value.resource.maximum_object_bytes > 4_096
        || value.resource_evidence_sha256.iter().all(|byte| *byte == 0)
    {
        return Err(CodecError::Flags);
    }
    if value.command_load_support != 0
        && value.capabilities
            & (PACKAGE_CAP_GROUND_NAV_UPDATE
                | PACKAGE_CAP_TARGET_UPDATE
                | PACKAGE_CAP_BRANCH_SELECT
                | PACKAGE_CAP_NAV_MODE
                | PACKAGE_CAP_HIGH_LEVEL_MODE)
            == 0
    {
        return Err(CodecError::Flags);
    }
    Ok(())
}

pub fn write_kfs11(
    value: &FlightSoftwarePackageManifest,
    output: &mut [u8],
) -> Result<(), CodecError> {
    if output.len() != KFS11_LENGTH {
        return Err(CodecError::Length);
    }
    validate_manifest(value)?;
    output.fill(0);
    output[..4].copy_from_slice(&KFS11_MAGIC);
    p16(output, 4, KFS11_VERSION);
    p16(output, 6, KFS11_LENGTH as u16);
    p32(output, 8, value.manifest_identity);
    p32(output, 12, value.package as u32);
    p32(output, 16, value.implementation_identity);
    p32(output, 20, value.abi as u32);
    p32(output, 24, value.vehicle_profile_identity);
    p32(output, 28, value.mission_compatibility_identity);
    p32(output, 32, value.capabilities);
    p16(output, 36, value.segment_support);
    p16(output, 38, value.command_load_support);
    output[40] = value.targets;
    output[41] = value.safe_state as u8;
    output[42] = value.command_loss as u8;
    output[44] = value.fast_hz;
    output[45] = value.navigation_hz;
    output[46] = value.guidance_hz;
    output[47] = value.resource.journal_records;
    p16(output, 48, value.resource.persistent_bytes);
    p16(output, 50, value.resource.transient_bytes);
    p16(output, 52, value.resource.stack_bytes);
    p16(output, 54, value.resource.maximum_object_bytes);
    output[56] = value.maximum_plan_events;
    output[57] = value.maximum_branches;
    output[58] = value.maximum_decisions;
    p32(output, 60, value.code_identity);
    p32(output, 64, value.configuration_identity);
    output[68..100].copy_from_slice(&value.resource_evidence_sha256);
    p32(output, 508, crc32_ieee(&output[..508]));
    Ok(())
}

pub fn parse_kfs11(input: &[u8]) -> Result<FlightSoftwarePackageManifest, CodecError> {
    if input.len() != KFS11_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != KFS11_MAGIC
        || g16(input, 4) != KFS11_VERSION
        || g16(input, 6) as usize != KFS11_LENGTH
    {
        return Err(CodecError::Enum);
    }
    if crc32_ieee(&input[..508]) != g32(input, 508) {
        return Err(CodecError::Checksum);
    }
    if input[43] != 0 || input[59] != 0 || input[100..508].iter().any(|byte| *byte != 0) {
        return Err(CodecError::Reserved);
    }
    let mut resource_evidence_sha256 = [0; 32];
    resource_evidence_sha256.copy_from_slice(&input[68..100]);
    let value = FlightSoftwarePackageManifest {
        manifest_identity: g32(input, 8),
        package: FlightSoftwarePackageId::parse(g32(input, 12))?,
        implementation_identity: g32(input, 16),
        abi: FlightAbiId::parse(g32(input, 20))?,
        vehicle_profile_identity: g32(input, 24),
        mission_compatibility_identity: g32(input, 28),
        capabilities: g32(input, 32),
        segment_support: g16(input, 36),
        command_load_support: g16(input, 38),
        targets: input[40],
        safe_state: PackageSafeStateId::parse(input[41])?,
        command_loss: PackageCommandLossBehavior::parse(input[42])?,
        resource: PackageResourceClaim {
            journal_records: input[47],
            persistent_bytes: g16(input, 48),
            transient_bytes: g16(input, 50),
            stack_bytes: g16(input, 52),
            maximum_object_bytes: g16(input, 54),
        },
        fast_hz: input[44],
        navigation_hz: input[45],
        guidance_hz: input[46],
        maximum_plan_events: input[56],
        maximum_branches: input[57],
        maximum_decisions: input[58],
        code_identity: g32(input, 60),
        configuration_identity: g32(input, 64),
        resource_evidence_sha256,
    };
    validate_manifest(&value)?;
    Ok(value)
}

pub fn parse_kfs11_for(
    input: &[u8],
    expected_manifest_identity: u32,
) -> Result<FlightSoftwarePackageManifest, CodecError> {
    let manifest = parse_kfs11(input)?;
    if manifest.manifest_identity != expected_manifest_identity {
        return Err(CodecError::Sequence);
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_manifest() -> FlightSoftwarePackageManifest {
        FlightSoftwarePackageManifest {
            manifest_identity: 0x11f5_a001,
            package: FlightSoftwarePackageId::KsaG10rReferenceOpsV1,
            implementation_identity: 0x1053_0001,
            abi: FlightAbiId::GlobalKlr10V1,
            vehicle_profile_identity: 5,
            mission_compatibility_identity: 0x10a0_0001,
            capabilities: PACKAGE_CAP_MASK,
            segment_support: PACKAGE_SEGMENT_MASK,
            command_load_support: PACKAGE_LOAD_MASK,
            targets: PACKAGE_TARGET_HOST | PACKAGE_TARGET_RUST_MOS | PACKAGE_TARGET_EXTERNAL,
            safe_state: PackageSafeStateId::ReferenceGlobalSafeV1,
            command_loss: PackageCommandLossBehavior::FrozenKlr10HoldThenSafe,
            resource: PackageResourceClaim {
                persistent_bytes: 2_048,
                transient_bytes: 1_024,
                stack_bytes: 512,
                journal_records: 32,
                maximum_object_bytes: 4_096,
            },
            fast_hz: 32,
            navigation_hz: 8,
            guidance_hz: 1,
            maximum_plan_events: 24,
            maximum_branches: 8,
            maximum_decisions: 8,
            code_identity: 0x1053_0001,
            configuration_identity: 0x10a0_0002,
            resource_evidence_sha256: [0x5a; 32],
        }
    }

    #[test]
    fn kfs11_round_trips_and_binds_identity() {
        let manifest = reference_manifest();
        let mut bytes = [0; KFS11_LENGTH];
        write_kfs11(&manifest, &mut bytes).unwrap();
        assert_eq!(parse_kfs11(&bytes).unwrap(), manifest);
        assert_eq!(
            parse_kfs11_for(&bytes, manifest.manifest_identity).unwrap(),
            manifest
        );
        assert_eq!(
            parse_kfs11_for(&bytes, manifest.manifest_identity ^ 1),
            Err(CodecError::Sequence)
        );
    }

    #[test]
    fn kfs11_rejects_corruption_reserved_and_bad_resource_claims() {
        let manifest = reference_manifest();
        let mut bytes = [0; KFS11_LENGTH];
        write_kfs11(&manifest, &mut bytes).unwrap();
        bytes[70] ^= 1;
        assert_eq!(parse_kfs11(&bytes), Err(CodecError::Checksum));

        write_kfs11(&manifest, &mut bytes).unwrap();
        bytes[200] = 1;
        let crc = crc32_ieee(&bytes[..508]);
        p32(&mut bytes, 508, crc);
        assert_eq!(parse_kfs11(&bytes), Err(CodecError::Reserved));

        let mut invalid = manifest;
        invalid.resource.maximum_object_bytes = 4_097;
        assert_eq!(write_kfs11(&invalid, &mut bytes), Err(CodecError::Flags));
    }
}
