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

pub const KMP11_LENGTH: usize = 2_048;
pub const KMP11_MAGIC: [u8; 4] = *b"KMP1";
pub const KMP11_VERSION: u16 = 11;
pub const KMP11_MAX_EVENTS: usize = 24;
pub const KMP11_MAX_BRANCHES: usize = 8;
pub const KMP11_MAX_DECISIONS: usize = 8;
const KMP11_EVENT_OFFSET: usize = 128;
const KMP11_EVENT_LENGTH: usize = 48;
const KMP11_BRANCH_OFFSET: usize = 1_280;
const KMP11_BRANCH_LENGTH: usize = 32;
const KMP11_DECISION_OFFSET: usize = 1_536;
const KMP11_DECISION_LENGTH: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MissionEventKind {
    AttitudeOrGuidanceTarget = 1,
    NavigationSource = 2,
    HoldOrContinue = 3,
    PlannedCorrection = 4,
    RecoveryMode = 5,
    AbortOrSafeState = 6,
}
impl MissionEventKind {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::AttitudeOrGuidanceTarget),
            2 => Ok(Self::NavigationSource),
            3 => Ok(Self::HoldOrContinue),
            4 => Ok(Self::PlannedCorrection),
            5 => Ok(Self::RecoveryMode),
            6 => Ok(Self::AbortOrSafeState),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MissionEventTrigger {
    ExactRelease = 1,
    PublicOnboardGuard = 2,
}
impl MissionEventTrigger {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::ExactRelease),
            2 => Ok(Self::PublicOnboardGuard),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MissionFailurePolicy {
    FailClosed = 1,
    Skip = 2,
    SelectContingency = 3,
}
impl MissionFailurePolicy {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::FailClosed),
            2 => Ok(Self::Skip),
            3 => Ok(Self::SelectContingency),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionPlanEvent {
    pub event_id: u16,
    pub kind: MissionEventKind,
    pub trigger: MissionEventTrigger,
    pub flags: u16,
    pub failure_policy: MissionFailurePolicy,
    pub earliest_epoch: u32,
    pub latest_epoch: u32,
    pub exact_epoch: u32,
    pub prerequisite_events: u32,
    pub required_capabilities: u32,
    pub arguments: [i32; 4],
    pub guard_metric: u16,
    pub guard_comparison: u8,
}
impl MissionPlanEvent {
    pub const EMPTY: Self = Self {
        event_id: 0,
        kind: MissionEventKind::AttitudeOrGuidanceTarget,
        trigger: MissionEventTrigger::ExactRelease,
        flags: 0,
        failure_policy: MissionFailurePolicy::FailClosed,
        earliest_epoch: 0,
        latest_epoch: 0,
        exact_epoch: 0,
        prerequisite_events: 0,
        required_capabilities: 0,
        arguments: [0; 4],
        guard_metric: 0,
        guard_comparison: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContingencyBranch {
    pub branch_id: u8,
    pub source_decision_id: u8,
    pub first_event_id: u16,
    pub flags: u16,
    pub earliest_epoch: u32,
    pub latest_epoch: u32,
    pub prerequisite_events: u32,
    pub required_capabilities: u32,
    pub arguments: [i32; 2],
}
impl ContingencyBranch {
    pub const EMPTY: Self = Self {
        branch_id: 0,
        source_decision_id: 0,
        first_event_id: 0,
        flags: 0,
        earliest_epoch: 0,
        latest_epoch: 0,
        prerequisite_events: 0,
        required_capabilities: 0,
        arguments: [0; 2],
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperatorDecisionPoint {
    pub decision_id: u8,
    pub default_branch_id: u8,
    pub flags: u16,
    pub earliest_epoch: u32,
    pub latest_epoch: u32,
    pub required_event_mask: u32,
    pub timeout_branch_id: u8,
}
impl OperatorDecisionPoint {
    pub const EMPTY: Self = Self {
        decision_id: 0,
        default_branch_id: 0,
        flags: 0,
        earliest_epoch: 0,
        latest_epoch: 0,
        required_event_mask: 0,
        timeout_branch_id: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionPlan {
    pub plan_identity: u32,
    pub package_manifest_identity: u32,
    pub package: FlightSoftwarePackageId,
    pub abi: FlightAbiId,
    pub vehicle_profile_identity: u32,
    pub mission_identity: u32,
    pub earth_identity: u32,
    pub environment_identity: u32,
    pub onboard_prediction_model: u32,
    pub ground_prediction_model: u32,
    pub required_capabilities: u32,
    pub event_count: u8,
    pub branch_count: u8,
    pub decision_count: u8,
    pub events: [MissionPlanEvent; KMP11_MAX_EVENTS],
    pub branches: [ContingencyBranch; KMP11_MAX_BRANCHES],
    pub decisions: [OperatorDecisionPoint; KMP11_MAX_DECISIONS],
}

fn validate_plan(plan: &MissionPlan) -> Result<(), CodecError> {
    if plan.plan_identity == 0
        || plan.package_manifest_identity == 0
        || plan.vehicle_profile_identity == 0
        || plan.mission_identity == 0
        || plan.earth_identity == 0
        || plan.environment_identity == 0
        || plan.onboard_prediction_model == 0
        || plan.ground_prediction_model == 0
        || plan.required_capabilities & !PACKAGE_CAP_MASK != 0
        || plan.event_count as usize > KMP11_MAX_EVENTS
        || plan.branch_count as usize > KMP11_MAX_BRANCHES
        || plan.decision_count as usize > KMP11_MAX_DECISIONS
    {
        return Err(CodecError::Flags);
    }
    for index in 0..KMP11_MAX_EVENTS {
        let event = plan.events[index];
        if index >= plan.event_count as usize {
            if event != MissionPlanEvent::EMPTY {
                return Err(CodecError::Reserved);
            }
            continue;
        }
        if event.event_id == 0
            || event.flags != 0
            || event.earliest_epoch > event.latest_epoch
            || event.required_capabilities & !PACKAGE_CAP_MASK != 0
            || event.guard_comparison > 5
            || (event.trigger == MissionEventTrigger::ExactRelease
                && (event.exact_epoch < event.earliest_epoch
                    || event.exact_epoch > event.latest_epoch))
        {
            return Err(CodecError::Flags);
        }
        for prior in 0..index {
            if plan.events[prior].event_id == event.event_id {
                return Err(CodecError::Sequence);
            }
        }
    }
    for index in 0..KMP11_MAX_BRANCHES {
        let branch = plan.branches[index];
        if index >= plan.branch_count as usize {
            if branch != ContingencyBranch::EMPTY {
                return Err(CodecError::Reserved);
            }
            continue;
        }
        if branch.branch_id == 0
            || branch.first_event_id == 0
            || branch.flags != 0
            || branch.earliest_epoch > branch.latest_epoch
            || branch.required_capabilities & !PACKAGE_CAP_MASK != 0
        {
            return Err(CodecError::Flags);
        }
        for prior in 0..index {
            if plan.branches[prior].branch_id == branch.branch_id {
                return Err(CodecError::Sequence);
            }
        }
    }
    for index in 0..KMP11_MAX_DECISIONS {
        let decision = plan.decisions[index];
        if index >= plan.decision_count as usize {
            if decision != OperatorDecisionPoint::EMPTY {
                return Err(CodecError::Reserved);
            }
            continue;
        }
        if decision.decision_id == 0
            || decision.flags != 0
            || decision.earliest_epoch > decision.latest_epoch
        {
            return Err(CodecError::Flags);
        }
        for prior in 0..index {
            if plan.decisions[prior].decision_id == decision.decision_id {
                return Err(CodecError::Sequence);
            }
        }
    }
    Ok(())
}

pub fn write_kmp11(plan: &MissionPlan, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KMP11_LENGTH {
        return Err(CodecError::Length);
    }
    validate_plan(plan)?;
    output.fill(0);
    output[..4].copy_from_slice(&KMP11_MAGIC);
    p16(output, 4, KMP11_VERSION);
    p16(output, 6, KMP11_LENGTH as u16);
    p32(output, 8, plan.plan_identity);
    p32(output, 12, plan.package_manifest_identity);
    p32(output, 16, plan.package as u32);
    p32(output, 20, plan.abi as u32);
    p32(output, 24, plan.vehicle_profile_identity);
    p32(output, 28, plan.mission_identity);
    p32(output, 32, plan.earth_identity);
    p32(output, 36, plan.environment_identity);
    p32(output, 40, plan.onboard_prediction_model);
    p32(output, 44, plan.ground_prediction_model);
    p32(output, 48, plan.required_capabilities);
    output[52] = plan.event_count;
    output[53] = plan.branch_count;
    output[54] = plan.decision_count;
    for index in 0..plan.event_count as usize {
        let value = plan.events[index];
        let offset = KMP11_EVENT_OFFSET + index * KMP11_EVENT_LENGTH;
        p16(output, offset, value.event_id);
        output[offset + 2] = value.kind as u8;
        output[offset + 3] = value.trigger as u8;
        p16(output, offset + 4, value.flags);
        output[offset + 6] = value.failure_policy as u8;
        p32(output, offset + 8, value.earliest_epoch);
        p32(output, offset + 12, value.latest_epoch);
        p32(output, offset + 16, value.exact_epoch);
        p32(output, offset + 20, value.prerequisite_events);
        p32(output, offset + 24, value.required_capabilities);
        for argument in 0..4 {
            p32(
                output,
                offset + 28 + argument * 4,
                value.arguments[argument] as u32,
            );
        }
        p16(output, offset + 44, value.guard_metric);
        output[offset + 46] = value.guard_comparison;
    }
    for index in 0..plan.branch_count as usize {
        let value = plan.branches[index];
        let offset = KMP11_BRANCH_OFFSET + index * KMP11_BRANCH_LENGTH;
        output[offset] = value.branch_id;
        output[offset + 1] = value.source_decision_id;
        p16(output, offset + 2, value.first_event_id);
        p16(output, offset + 4, value.flags);
        p32(output, offset + 8, value.earliest_epoch);
        p32(output, offset + 12, value.latest_epoch);
        p32(output, offset + 16, value.prerequisite_events);
        p32(output, offset + 20, value.required_capabilities);
        p32(output, offset + 24, value.arguments[0] as u32);
        p32(output, offset + 28, value.arguments[1] as u32);
    }
    for index in 0..plan.decision_count as usize {
        let value = plan.decisions[index];
        let offset = KMP11_DECISION_OFFSET + index * KMP11_DECISION_LENGTH;
        output[offset] = value.decision_id;
        output[offset + 1] = value.default_branch_id;
        p16(output, offset + 2, value.flags);
        p32(output, offset + 4, value.earliest_epoch);
        p32(output, offset + 8, value.latest_epoch);
        p32(output, offset + 12, value.required_event_mask);
        output[offset + 16] = value.timeout_branch_id;
    }
    p32(
        output,
        KMP11_LENGTH - 4,
        crc32_ieee(&output[..KMP11_LENGTH - 4]),
    );
    Ok(())
}

pub fn parse_kmp11(input: &[u8]) -> Result<MissionPlan, CodecError> {
    if input.len() != KMP11_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != KMP11_MAGIC
        || g16(input, 4) != KMP11_VERSION
        || g16(input, 6) as usize != KMP11_LENGTH
    {
        return Err(CodecError::Enum);
    }
    if crc32_ieee(&input[..KMP11_LENGTH - 4]) != g32(input, KMP11_LENGTH - 4) {
        return Err(CodecError::Checksum);
    }
    if input[55..128].iter().any(|byte| *byte != 0)
        || input[1_728..KMP11_LENGTH - 4].iter().any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let event_count = input[52];
    let branch_count = input[53];
    let decision_count = input[54];
    if event_count as usize > KMP11_MAX_EVENTS
        || branch_count as usize > KMP11_MAX_BRANCHES
        || decision_count as usize > KMP11_MAX_DECISIONS
    {
        return Err(CodecError::Flags);
    }
    let mut events = [MissionPlanEvent::EMPTY; KMP11_MAX_EVENTS];
    for (index, event) in events.iter_mut().enumerate().take(event_count as usize) {
        let offset = KMP11_EVENT_OFFSET + index * KMP11_EVENT_LENGTH;
        if input[offset + 7] != 0 || input[offset + 47] != 0 {
            return Err(CodecError::Reserved);
        }
        let mut arguments = [0; 4];
        for (argument, value) in arguments.iter_mut().enumerate() {
            *value = g32(input, offset + 28 + argument * 4) as i32;
        }
        *event = MissionPlanEvent {
            event_id: g16(input, offset),
            kind: MissionEventKind::parse(input[offset + 2])?,
            trigger: MissionEventTrigger::parse(input[offset + 3])?,
            flags: g16(input, offset + 4),
            failure_policy: MissionFailurePolicy::parse(input[offset + 6])?,
            earliest_epoch: g32(input, offset + 8),
            latest_epoch: g32(input, offset + 12),
            exact_epoch: g32(input, offset + 16),
            prerequisite_events: g32(input, offset + 20),
            required_capabilities: g32(input, offset + 24),
            arguments,
            guard_metric: g16(input, offset + 44),
            guard_comparison: input[offset + 46],
        };
    }
    if input[KMP11_EVENT_OFFSET + event_count as usize * KMP11_EVENT_LENGTH..KMP11_BRANCH_OFFSET]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let mut branches = [ContingencyBranch::EMPTY; KMP11_MAX_BRANCHES];
    for (index, branch) in branches.iter_mut().enumerate().take(branch_count as usize) {
        let offset = KMP11_BRANCH_OFFSET + index * KMP11_BRANCH_LENGTH;
        if input[offset + 6] != 0 || input[offset + 7] != 0 {
            return Err(CodecError::Reserved);
        }
        *branch = ContingencyBranch {
            branch_id: input[offset],
            source_decision_id: input[offset + 1],
            first_event_id: g16(input, offset + 2),
            flags: g16(input, offset + 4),
            earliest_epoch: g32(input, offset + 8),
            latest_epoch: g32(input, offset + 12),
            prerequisite_events: g32(input, offset + 16),
            required_capabilities: g32(input, offset + 20),
            arguments: [
                g32(input, offset + 24) as i32,
                g32(input, offset + 28) as i32,
            ],
        };
    }
    if input
        [KMP11_BRANCH_OFFSET + branch_count as usize * KMP11_BRANCH_LENGTH..KMP11_DECISION_OFFSET]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let mut decisions = [OperatorDecisionPoint::EMPTY; KMP11_MAX_DECISIONS];
    for (index, decision) in decisions
        .iter_mut()
        .enumerate()
        .take(decision_count as usize)
    {
        let offset = KMP11_DECISION_OFFSET + index * KMP11_DECISION_LENGTH;
        if input[offset + 17..offset + KMP11_DECISION_LENGTH]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(CodecError::Reserved);
        }
        *decision = OperatorDecisionPoint {
            decision_id: input[offset],
            default_branch_id: input[offset + 1],
            flags: g16(input, offset + 2),
            earliest_epoch: g32(input, offset + 4),
            latest_epoch: g32(input, offset + 8),
            required_event_mask: g32(input, offset + 12),
            timeout_branch_id: input[offset + 16],
        };
    }
    if input[KMP11_DECISION_OFFSET + decision_count as usize * KMP11_DECISION_LENGTH..1_728]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let plan = MissionPlan {
        plan_identity: g32(input, 8),
        package_manifest_identity: g32(input, 12),
        package: FlightSoftwarePackageId::parse(g32(input, 16))?,
        abi: FlightAbiId::parse(g32(input, 20))?,
        vehicle_profile_identity: g32(input, 24),
        mission_identity: g32(input, 28),
        earth_identity: g32(input, 32),
        environment_identity: g32(input, 36),
        onboard_prediction_model: g32(input, 40),
        ground_prediction_model: g32(input, 44),
        required_capabilities: g32(input, 48),
        event_count,
        branch_count,
        decision_count,
        events,
        branches,
        decisions,
    };
    validate_plan(&plan)?;
    Ok(plan)
}

pub const KPX11_LENGTH: usize = 512;
pub const KPX11_PAYLOAD_MAX: usize = 480;
pub const KPX11_MAGIC: [u8; 4] = *b"KPX1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PackageObjectType {
    PackageManifest = 1,
    MissionPlan = 2,
    ProcedurePack = 3,
    PackageConfiguration = 4,
}
impl PackageObjectType {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::PackageManifest),
            2 => Ok(Self::MissionPlan),
            3 => Ok(Self::ProcedurePack),
            4 => Ok(Self::PackageConfiguration),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackageObjectSegment<'a> {
    pub object_type: PackageObjectType,
    pub object_identity: u32,
    pub total_length: u32,
    pub total_crc32: u32,
    pub segment_index: u16,
    pub segment_count: u16,
    pub logical_offset: u16,
    pub payload: &'a [u8],
}

pub fn write_kpx11(value: &PackageObjectSegment<'_>, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KPX11_LENGTH {
        return Err(CodecError::Length);
    }
    let expected_count = value.total_length.div_ceil(KPX11_PAYLOAD_MAX as u32);
    let expected_offset = u32::from(value.segment_index) * KPX11_PAYLOAD_MAX as u32;
    let remaining = value.total_length.saturating_sub(expected_offset);
    let expected_payload = remaining.min(KPX11_PAYLOAD_MAX as u32) as usize;
    if value.object_identity == 0
        || value.total_length == 0
        || value.total_length > 4_096
        || value.total_crc32 == 0
        || value.segment_count == 0
        || u32::from(value.segment_count) != expected_count
        || value.segment_index >= value.segment_count
        || u32::from(value.logical_offset) != expected_offset
        || value.payload.len() != expected_payload
    {
        return Err(CodecError::Sequence);
    }
    output.fill(0);
    output[..4].copy_from_slice(&KPX11_MAGIC);
    output[4] = 11;
    output[5] = value.object_type as u8;
    p16(output, 6, 28);
    p32(output, 8, value.object_identity);
    p32(output, 12, value.total_length);
    p32(output, 16, value.total_crc32);
    p16(output, 20, value.segment_index);
    p16(output, 22, value.segment_count);
    p16(output, 24, value.logical_offset);
    p16(output, 26, value.payload.len() as u16);
    output[28..28 + value.payload.len()].copy_from_slice(value.payload);
    p32(output, 508, crc32_ieee(&output[..508]));
    Ok(())
}

pub fn parse_kpx11(input: &[u8]) -> Result<PackageObjectSegment<'_>, CodecError> {
    if input.len() != KPX11_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != KPX11_MAGIC || input[4] != 11 || g16(input, 6) != 28 {
        return Err(CodecError::Enum);
    }
    if crc32_ieee(&input[..508]) != g32(input, 508) {
        return Err(CodecError::Checksum);
    }
    let payload_length = g16(input, 26) as usize;
    if payload_length > KPX11_PAYLOAD_MAX
        || input[28 + payload_length..508]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let value = PackageObjectSegment {
        object_type: PackageObjectType::parse(input[5])?,
        object_identity: g32(input, 8),
        total_length: g32(input, 12),
        total_crc32: g32(input, 16),
        segment_index: g16(input, 20),
        segment_count: g16(input, 22),
        logical_offset: g16(input, 24),
        payload: &input[28..28 + payload_length],
    };
    let mut canonical = [0; KPX11_LENGTH];
    write_kpx11(&value, &mut canonical)?;
    Ok(value)
}

#[cfg(test)]
mod mission_plan_tests {
    use super::*;

    fn plan() -> MissionPlan {
        let mut events = [MissionPlanEvent::EMPTY; KMP11_MAX_EVENTS];
        events[0] = MissionPlanEvent {
            event_id: 1,
            kind: MissionEventKind::PlannedCorrection,
            trigger: MissionEventTrigger::ExactRelease,
            flags: 0,
            failure_policy: MissionFailurePolicy::FailClosed,
            earliest_epoch: 100,
            latest_epoch: 120,
            exact_epoch: 110,
            prerequisite_events: 0,
            required_capabilities: PACKAGE_CAP_TARGET_UPDATE,
            arguments: [1, 2, 3, 4],
            guard_metric: 0,
            guard_comparison: 0,
        };
        MissionPlan {
            plan_identity: 0x11a0_0001,
            package_manifest_identity: 0x11f5_a001,
            package: FlightSoftwarePackageId::KsaG10rReferenceOpsV1,
            abi: FlightAbiId::GlobalKlr10V1,
            vehicle_profile_identity: 5,
            mission_identity: 0x10a0_0001,
            earth_identity: 0x10e0_0001,
            environment_identity: 0x10a7_0001,
            onboard_prediction_model: 0x11d0_0001,
            ground_prediction_model: 0x11d0_0002,
            required_capabilities: PACKAGE_CAP_MISSION_PLAN | PACKAGE_CAP_TARGET_UPDATE,
            event_count: 1,
            branch_count: 0,
            decision_count: 0,
            events,
            branches: [ContingencyBranch::EMPTY; KMP11_MAX_BRANCHES],
            decisions: [OperatorDecisionPoint::EMPTY; KMP11_MAX_DECISIONS],
        }
    }

    #[test]
    fn mission_plan_round_trips_and_reserved_data_fails_closed() {
        let plan = plan();
        let mut bytes = [0; KMP11_LENGTH];
        write_kmp11(&plan, &mut bytes).unwrap();
        assert_eq!(parse_kmp11(&bytes).unwrap(), plan);
        bytes[1_900] = 1;
        let crc = crc32_ieee(&bytes[..KMP11_LENGTH - 4]);
        p32(&mut bytes, KMP11_LENGTH - 4, crc);
        assert_eq!(parse_kmp11(&bytes), Err(CodecError::Reserved));
    }

    #[test]
    fn object_segments_are_canonical_and_reconstruct_the_plan() {
        let mut object = [0; KMP11_LENGTH];
        write_kmp11(&plan(), &mut object).unwrap();
        let total_crc32 = crc32_ieee(&object);
        let count = object.len().div_ceil(KPX11_PAYLOAD_MAX) as u16;
        let mut rebuilt = [0; KMP11_LENGTH];
        for index in 0..count {
            let offset = usize::from(index) * KPX11_PAYLOAD_MAX;
            let end = (offset + KPX11_PAYLOAD_MAX).min(object.len());
            let segment = PackageObjectSegment {
                object_type: PackageObjectType::MissionPlan,
                object_identity: 0x11a0_0001,
                total_length: object.len() as u32,
                total_crc32,
                segment_index: index,
                segment_count: count,
                logical_offset: offset as u16,
                payload: &object[offset..end],
            };
            let mut bytes = [0; KPX11_LENGTH];
            write_kpx11(&segment, &mut bytes).unwrap();
            let decoded = parse_kpx11(&bytes).unwrap();
            rebuilt[offset..end].copy_from_slice(decoded.payload);
        }
        assert_eq!(rebuilt, object);
        assert_eq!(crc32_ieee(&rebuilt), total_crc32);
    }
}

pub const KUL11_LENGTH: usize = 512;
pub const KUA11_LENGTH: usize = 128;
pub const KAL11_HEADER_LENGTH: usize = 128;
pub const KAL11_RECORD_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum UplinkLoadType {
    GroundNavigationUpdate = 1,
    MissionEventTarget = 2,
    ContingencyBranch = 3,
    NavigationMode = 4,
    HighLevelMode = 5,
}
impl UplinkLoadType {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::GroundNavigationUpdate),
            2 => Ok(Self::MissionEventTarget),
            3 => Ok(Self::ContingencyBranch),
            4 => Ok(Self::NavigationMode),
            5 => Ok(Self::HighLevelMode),
            _ => Err(CodecError::Enum),
        }
    }

    pub const fn capability(self) -> u32 {
        match self {
            Self::GroundNavigationUpdate => PACKAGE_CAP_GROUND_NAV_UPDATE,
            Self::MissionEventTarget => PACKAGE_CAP_TARGET_UPDATE,
            Self::ContingencyBranch => PACKAGE_CAP_BRANCH_SELECT,
            Self::NavigationMode => PACKAGE_CAP_NAV_MODE,
            Self::HighLevelMode => PACKAGE_CAP_HIGH_LEVEL_MODE,
        }
    }

    pub const fn support_bit(self) -> u16 {
        match self {
            Self::GroundNavigationUpdate => PACKAGE_LOAD_GROUND_NAV,
            Self::MissionEventTarget => PACKAGE_LOAD_EVENT_TARGET,
            Self::ContingencyBranch => PACKAGE_LOAD_BRANCH,
            Self::NavigationMode => PACKAGE_LOAD_NAV_MODE,
            Self::HighLevelMode => PACKAGE_LOAD_HIGH_LEVEL_MODE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UplinkCommandLoad {
    pub load_identity: u32,
    pub package_manifest_identity: u32,
    pub plan_identity: u32,
    pub abi: FlightAbiId,
    pub source_estimator_identity: u32,
    pub source_estimator_checksum: u32,
    pub stage_epoch: u32,
    pub not_before_epoch: u32,
    pub expires_epoch: u32,
    pub requested_effective_epoch: u32,
    pub required_capabilities: u32,
    pub prerequisite_event_mask: u32,
    pub position_residual_limit_q12: i32,
    pub velocity_residual_limit_q24: i32,
    pub frame: crate::phase10::GlobalFrameId,
    pub load_type: UplinkLoadType,
    pub arguments: [i32; 16],
}

fn validate_uplink_load(value: &UplinkCommandLoad) -> Result<(), CodecError> {
    if value.load_identity == 0
        || value.package_manifest_identity == 0
        || value.plan_identity == 0
        || value.source_estimator_identity == 0
        || value.expires_epoch < value.not_before_epoch
        || value.requested_effective_epoch < value.not_before_epoch
        || value.requested_effective_epoch > value.expires_epoch
        || value.required_capabilities & !PACKAGE_CAP_MASK != 0
        || value.required_capabilities & value.load_type.capability() == 0
        || value.position_residual_limit_q12 < 0
        || value.velocity_residual_limit_q24 < 0
    {
        return Err(CodecError::Flags);
    }
    Ok(())
}

pub fn write_kul11(value: &UplinkCommandLoad, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KUL11_LENGTH {
        return Err(CodecError::Length);
    }
    validate_uplink_load(value)?;
    output.fill(0);
    output[..4].copy_from_slice(b"KUL1");
    p16(output, 4, 11);
    p16(output, 6, KUL11_LENGTH as u16);
    p32(output, 8, value.load_identity);
    p32(output, 12, value.package_manifest_identity);
    p32(output, 16, value.plan_identity);
    p32(output, 20, value.abi as u32);
    p32(output, 24, value.source_estimator_identity);
    p32(output, 28, value.source_estimator_checksum);
    p32(output, 32, value.stage_epoch);
    p32(output, 36, value.not_before_epoch);
    p32(output, 40, value.expires_epoch);
    p32(output, 44, value.requested_effective_epoch);
    p32(output, 48, value.required_capabilities);
    p32(output, 52, value.prerequisite_event_mask);
    p32(output, 56, value.position_residual_limit_q12 as u32);
    p32(output, 60, value.velocity_residual_limit_q24 as u32);
    output[64] = value.frame as u8;
    output[65] = value.load_type as u8;
    for (index, argument) in value.arguments.iter().enumerate() {
        p32(output, 68 + index * 4, *argument as u32);
    }
    p32(output, 508, crc32_ieee(&output[..508]));
    Ok(())
}

fn parse_global_frame(value: u8) -> Result<crate::phase10::GlobalFrameId, CodecError> {
    match value {
        1 => Ok(crate::phase10::GlobalFrameId::LocalEnuV1),
        2 => Ok(crate::phase10::GlobalFrameId::EarthFixedEcefV1),
        3 => Ok(crate::phase10::GlobalFrameId::EarthInertialEciV1),
        _ => Err(CodecError::Enum),
    }
}

pub fn parse_kul11(input: &[u8]) -> Result<UplinkCommandLoad, CodecError> {
    if input.len() != KUL11_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != *b"KUL1" || g16(input, 4) != 11 || g16(input, 6) != KUL11_LENGTH as u16 {
        return Err(CodecError::Enum);
    }
    if crc32_ieee(&input[..508]) != g32(input, 508) {
        return Err(CodecError::Checksum);
    }
    if input[66] != 0 || input[67] != 0 || input[132..508].iter().any(|byte| *byte != 0) {
        return Err(CodecError::Reserved);
    }
    let mut arguments = [0; 16];
    for (index, argument) in arguments.iter_mut().enumerate() {
        *argument = g32(input, 68 + index * 4) as i32;
    }
    let value = UplinkCommandLoad {
        load_identity: g32(input, 8),
        package_manifest_identity: g32(input, 12),
        plan_identity: g32(input, 16),
        abi: FlightAbiId::parse(g32(input, 20))?,
        source_estimator_identity: g32(input, 24),
        source_estimator_checksum: g32(input, 28),
        stage_epoch: g32(input, 32),
        not_before_epoch: g32(input, 36),
        expires_epoch: g32(input, 40),
        requested_effective_epoch: g32(input, 44),
        required_capabilities: g32(input, 48),
        prerequisite_event_mask: g32(input, 52),
        position_residual_limit_q12: g32(input, 56) as i32,
        velocity_residual_limit_q24: g32(input, 60) as i32,
        frame: parse_global_frame(input[64])?,
        load_type: UplinkLoadType::parse(input[65])?,
        arguments,
    };
    validate_uplink_load(&value)?;
    Ok(value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum UplinkControlKind {
    StageReceipt = 1,
    CommitRequest = 2,
    CommitReceipt = 3,
    Cancellation = 4,
    ExecutionAcknowledgement = 5,
}
impl UplinkControlKind {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::StageReceipt),
            2 => Ok(Self::CommitRequest),
            3 => Ok(Self::CommitReceipt),
            4 => Ok(Self::Cancellation),
            5 => Ok(Self::ExecutionAcknowledgement),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum UplinkState {
    Empty = 0,
    Staged = 1,
    Committed = 2,
    Executed = 3,
    Cancelled = 4,
    Rejected = 5,
    Expired = 6,
}
impl UplinkState {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Empty),
            1 => Ok(Self::Staged),
            2 => Ok(Self::Committed),
            3 => Ok(Self::Executed),
            4 => Ok(Self::Cancelled),
            5 => Ok(Self::Rejected),
            6 => Ok(Self::Expired),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum UplinkReasonCode {
    Accepted = 0,
    Corrupt = 1,
    Identity = 2,
    Frame = 3,
    Stale = 4,
    Late = 5,
    Unsupported = 6,
    Capability = 7,
    Prerequisite = 8,
    Bounds = 9,
    Residual = 10,
    Occupied = 11,
    Conflict = 12,
    AlreadyApplied = 13,
}
impl UplinkReasonCode {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Accepted),
            1 => Ok(Self::Corrupt),
            2 => Ok(Self::Identity),
            3 => Ok(Self::Frame),
            4 => Ok(Self::Stale),
            5 => Ok(Self::Late),
            6 => Ok(Self::Unsupported),
            7 => Ok(Self::Capability),
            8 => Ok(Self::Prerequisite),
            9 => Ok(Self::Bounds),
            10 => Ok(Self::Residual),
            11 => Ok(Self::Occupied),
            12 => Ok(Self::Conflict),
            13 => Ok(Self::AlreadyApplied),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UplinkControlRecord {
    pub kind: UplinkControlKind,
    pub control_identity: u32,
    pub load_identity: u32,
    pub package_manifest_identity: u32,
    pub plan_identity: u32,
    pub request_epoch: u32,
    pub effective_epoch: u32,
    pub state: UplinkState,
    pub reason: UplinkReasonCode,
    pub receipt_checksum: u32,
}

pub fn write_kua11(value: &UplinkControlRecord, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KUA11_LENGTH {
        return Err(CodecError::Length);
    }
    if value.control_identity == 0
        || value.load_identity == 0
        || value.package_manifest_identity == 0
        || value.plan_identity == 0
    {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"KUA1");
    output[4] = 11;
    output[5] = value.kind as u8;
    p16(output, 6, KUA11_LENGTH as u16);
    p32(output, 8, value.control_identity);
    p32(output, 12, value.load_identity);
    p32(output, 16, value.package_manifest_identity);
    p32(output, 20, value.plan_identity);
    p32(output, 24, value.request_epoch);
    p32(output, 28, value.effective_epoch);
    output[32] = value.state as u8;
    output[33] = value.reason as u8;
    p32(output, 36, value.receipt_checksum);
    p32(output, 124, crc32_ieee(&output[..124]));
    Ok(())
}

pub fn parse_kua11(input: &[u8]) -> Result<UplinkControlRecord, CodecError> {
    if input.len() != KUA11_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != *b"KUA1" || input[4] != 11 || g16(input, 6) != KUA11_LENGTH as u16 {
        return Err(CodecError::Enum);
    }
    if crc32_ieee(&input[..124]) != g32(input, 124) {
        return Err(CodecError::Checksum);
    }
    if input[34] != 0 || input[35] != 0 || input[40..124].iter().any(|byte| *byte != 0) {
        return Err(CodecError::Reserved);
    }
    let value = UplinkControlRecord {
        kind: UplinkControlKind::parse(input[5])?,
        control_identity: g32(input, 8),
        load_identity: g32(input, 12),
        package_manifest_identity: g32(input, 16),
        plan_identity: g32(input, 20),
        request_epoch: g32(input, 24),
        effective_epoch: g32(input, 28),
        state: UplinkState::parse(input[32])?,
        reason: UplinkReasonCode::parse(input[33])?,
        receipt_checksum: g32(input, 36),
    };
    if value.control_identity == 0
        || value.load_identity == 0
        || value.package_manifest_identity == 0
        || value.plan_identity == 0
    {
        return Err(CodecError::Flags);
    }
    Ok(value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OperationalRole {
    Observer = 1,
    GuidedOperator = 2,
    FlightController = 3,
    FlightSoftwareEngineer = 4,
    SimDirector = 5,
    ScriptedOperator = 6,
}
impl OperationalRole {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Observer),
            2 => Ok(Self::GuidedOperator),
            3 => Ok(Self::FlightController),
            4 => Ok(Self::FlightSoftwareEngineer),
            5 => Ok(Self::SimDirector),
            6 => Ok(Self::ScriptedOperator),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionLogHeader {
    pub session_definition_identity: u32,
    pub transcript_identity: u32,
    pub package_manifest_identity: u32,
    pub plan_identity: u32,
    pub action_count: u32,
    pub final_chain: u32,
    pub complete: bool,
}

pub fn write_kal11_header(value: &ActionLogHeader, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KAL11_HEADER_LENGTH
        || value.session_definition_identity == 0
        || value.transcript_identity == 0
        || value.package_manifest_identity == 0
        || value.plan_identity == 0
    {
        return Err(CodecError::Length);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"KAL1");
    p16(output, 4, 11);
    p16(output, 6, KAL11_HEADER_LENGTH as u16);
    p32(output, 8, value.session_definition_identity);
    p32(output, 12, value.transcript_identity);
    p32(output, 16, value.package_manifest_identity);
    p32(output, 20, value.plan_identity);
    p32(output, 24, value.action_count);
    p32(output, 28, value.final_chain);
    output[32] = u8::from(value.complete);
    p32(output, 124, crc32_ieee(&output[..124]));
    Ok(())
}

pub fn parse_kal11_header(input: &[u8]) -> Result<ActionLogHeader, CodecError> {
    if input.len() != KAL11_HEADER_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != *b"KAL1" || g16(input, 4) != 11 || g16(input, 6) != 128 {
        return Err(CodecError::Enum);
    }
    if crc32_ieee(&input[..124]) != g32(input, 124) {
        return Err(CodecError::Checksum);
    }
    if input[32] > 1 || input[33..124].iter().any(|byte| *byte != 0) {
        return Err(CodecError::Reserved);
    }
    Ok(ActionLogHeader {
        session_definition_identity: g32(input, 8),
        transcript_identity: g32(input, 12),
        package_manifest_identity: g32(input, 16),
        plan_identity: g32(input, 20),
        action_count: g32(input, 24),
        final_chain: g32(input, 28),
        complete: input[32] != 0,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionLogRecord {
    pub sequence: u32,
    pub epoch: u32,
    pub role: OperationalRole,
    pub action_kind: UplinkControlKind,
    pub state: UplinkState,
    pub reason: UplinkReasonCode,
    pub load_identity: u32,
    pub detail_identity: u32,
    pub procedure_step: u16,
    pub arguments: [i32; 4],
    pub prior_chain: u32,
    pub chain: u32,
}

pub fn write_kal11_record(value: &ActionLogRecord, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KAL11_RECORD_LENGTH || value.sequence == 0 {
        return Err(CodecError::Length);
    }
    output.fill(0);
    p32(output, 0, value.sequence);
    p32(output, 4, value.epoch);
    output[8] = value.role as u8;
    output[9] = value.action_kind as u8;
    output[10] = value.state as u8;
    output[11] = value.reason as u8;
    p32(output, 12, value.load_identity);
    p32(output, 16, value.detail_identity);
    p16(output, 20, value.procedure_step);
    for (index, argument) in value.arguments.iter().enumerate() {
        p32(output, 24 + index * 4, *argument as u32);
    }
    p32(output, 40, value.prior_chain);
    p32(output, 44, value.chain);
    p32(output, 60, crc32_ieee(&output[..60]));
    Ok(())
}

pub fn parse_kal11_record(input: &[u8]) -> Result<ActionLogRecord, CodecError> {
    if input.len() != KAL11_RECORD_LENGTH {
        return Err(CodecError::Length);
    }
    if crc32_ieee(&input[..60]) != g32(input, 60) {
        return Err(CodecError::Checksum);
    }
    if input[22] != 0 || input[23] != 0 || input[48..60].iter().any(|byte| *byte != 0) {
        return Err(CodecError::Reserved);
    }
    let mut arguments = [0; 4];
    for (index, argument) in arguments.iter_mut().enumerate() {
        *argument = g32(input, 24 + index * 4) as i32;
    }
    let value = ActionLogRecord {
        sequence: g32(input, 0),
        epoch: g32(input, 4),
        role: OperationalRole::parse(input[8])?,
        action_kind: UplinkControlKind::parse(input[9])?,
        state: UplinkState::parse(input[10])?,
        reason: UplinkReasonCode::parse(input[11])?,
        load_identity: g32(input, 12),
        detail_identity: g32(input, 16),
        procedure_step: g16(input, 20),
        arguments,
        prior_chain: g32(input, 40),
        chain: g32(input, 44),
    };
    if value.sequence == 0 {
        return Err(CodecError::Sequence);
    }
    Ok(value)
}

#[cfg(test)]
mod uplink_contract_tests {
    use super::*;

    fn load() -> UplinkCommandLoad {
        UplinkCommandLoad {
            load_identity: 0x11c0_0001,
            package_manifest_identity: 0x11f5_a001,
            plan_identity: 0x11a0_0001,
            abi: FlightAbiId::GlobalKlr10V1,
            source_estimator_identity: 0x11e0_0001,
            source_estimator_checksum: 0x1234_5678,
            stage_epoch: 100,
            not_before_epoch: 102,
            expires_epoch: 120,
            requested_effective_epoch: 104,
            required_capabilities: PACKAGE_CAP_GROUND_NAV_UPDATE,
            prerequisite_event_mask: 0,
            position_residual_limit_q12: 4_096,
            velocity_residual_limit_q24: 16_777,
            frame: crate::phase10::GlobalFrameId::EarthInertialEciV1,
            load_type: UplinkLoadType::GroundNavigationUpdate,
            arguments: [0; 16],
        }
    }

    #[test]
    fn uplink_load_and_receipt_are_strict() {
        let load = load();
        let mut bytes = [0; KUL11_LENGTH];
        write_kul11(&load, &mut bytes).unwrap();
        assert_eq!(parse_kul11(&bytes).unwrap(), load);
        bytes[200] = 1;
        let crc = crc32_ieee(&bytes[..508]);
        p32(&mut bytes, 508, crc);
        assert_eq!(parse_kul11(&bytes), Err(CodecError::Reserved));

        let receipt = UplinkControlRecord {
            kind: UplinkControlKind::CommitReceipt,
            control_identity: 7,
            load_identity: load.load_identity,
            package_manifest_identity: load.package_manifest_identity,
            plan_identity: load.plan_identity,
            request_epoch: 100,
            effective_epoch: 104,
            state: UplinkState::Committed,
            reason: UplinkReasonCode::Accepted,
            receipt_checksum: 0x55aa_1234,
        };
        let mut control = [0; KUA11_LENGTH];
        write_kua11(&receipt, &mut control).unwrap();
        assert_eq!(parse_kua11(&control).unwrap(), receipt);
        control[50] = 1;
        let crc = crc32_ieee(&control[..124]);
        p32(&mut control, 124, crc);
        assert_eq!(parse_kua11(&control), Err(CodecError::Reserved));
    }

    #[test]
    fn action_log_header_and_records_round_trip() {
        let header = ActionLogHeader {
            session_definition_identity: 1,
            transcript_identity: 2,
            package_manifest_identity: 3,
            plan_identity: 4,
            action_count: 1,
            final_chain: 0x99,
            complete: true,
        };
        let mut header_bytes = [0; KAL11_HEADER_LENGTH];
        write_kal11_header(&header, &mut header_bytes).unwrap();
        assert_eq!(parse_kal11_header(&header_bytes).unwrap(), header);

        let record = ActionLogRecord {
            sequence: 1,
            epoch: 100,
            role: OperationalRole::ScriptedOperator,
            action_kind: UplinkControlKind::CommitRequest,
            state: UplinkState::Committed,
            reason: UplinkReasonCode::Accepted,
            load_identity: 7,
            detail_identity: 8,
            procedure_step: 4,
            arguments: [1, 2, 3, 4],
            prior_chain: 9,
            chain: 10,
        };
        let mut record_bytes = [0; KAL11_RECORD_LENGTH];
        write_kal11_record(&record, &mut record_bytes).unwrap();
        assert_eq!(parse_kal11_record(&record_bytes).unwrap(), record);
    }

    #[test]
    fn no_uplink_load_type_can_name_a_physical_effector() {
        let supported = [
            UplinkLoadType::GroundNavigationUpdate,
            UplinkLoadType::MissionEventTarget,
            UplinkLoadType::ContingencyBranch,
            UplinkLoadType::NavigationMode,
            UplinkLoadType::HighLevelMode,
        ];
        assert_eq!(supported.len(), 5);
        for raw in 6..=u8::MAX {
            assert_eq!(UplinkLoadType::parse(raw), Err(CodecError::Enum));
        }
    }
}
