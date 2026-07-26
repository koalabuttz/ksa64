//! Phase 11 profile-specific flight-package envelope.
//!
//! This layer is deliberately additive: KLR10 is still the wire ABI and the
//! inactive reference package delegates every release to the frozen Phase 10
//! flight computer without modifying inputs or outputs.

use crate::phase10::{GlobalFlightComputer, GlobalFlightConfig, GlobalFlightEvidence};
use ksa64_interface::phase10::{
    GlobalAidFrameCell, GlobalFastSensorCell, GlobalTransitionCell, KLR10_CONTRACT_ID,
};
use ksa64_interface::phase11::{
    FlightAbiId, FlightSoftwarePackageId, FlightSoftwarePackageManifest,
    PackageCommandLossBehavior, PackageResourceClaim, PackageSafeStateId, PACKAGE_CAP_MASK,
    PACKAGE_LOAD_MASK, PACKAGE_SEGMENT_MASK, PACKAGE_TARGET_EXTERNAL, PACKAGE_TARGET_HOST,
    PACKAGE_TARGET_RUST_MOS,
};

pub const KSA_G10R_REFERENCE_OPS_MANIFEST_ID: u32 = 0x11f5_a001;
pub const KSA_G10R_REFERENCE_OPS_IMPLEMENTATION_ID: u32 = 0x11f5_1001;
pub const GLOBAL_ECEF_PROFILE_ID: u32 = 5;
pub const KSA_G10R_MISSION_COMPATIBILITY_ID: u32 = 0x10a0_0001;

pub trait GlobalKlr10FlightPackage {
    fn manifest(&self) -> FlightSoftwarePackageManifest;

    fn process_release(
        &mut self,
        fast: Option<GlobalFastSensorCell>,
        aid: Option<GlobalAidFrameCell>,
        transition: Option<GlobalTransitionCell>,
    ) -> GlobalFlightEvidence;
}

pub const fn ksa_g10r_reference_ops_manifest() -> FlightSoftwarePackageManifest {
    FlightSoftwarePackageManifest {
        manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
        package: FlightSoftwarePackageId::KsaG10rReferenceOpsV1,
        implementation_identity: KSA_G10R_REFERENCE_OPS_IMPLEMENTATION_ID,
        abi: FlightAbiId::GlobalKlr10V1,
        vehicle_profile_identity: GLOBAL_ECEF_PROFILE_ID,
        mission_compatibility_identity: KSA_G10R_MISSION_COMPATIBILITY_ID,
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
        code_identity: crate::phase10::GLOBAL_FLIGHT_CONTRACT_ID,
        configuration_identity: KLR10_CONTRACT_ID,
        resource_evidence_sha256: [0x11; 32],
    }
}

pub struct KsaG10rReferenceOpsV1 {
    inner: GlobalFlightComputer,
}

impl KsaG10rReferenceOpsV1 {
    pub fn new(config: GlobalFlightConfig) -> Option<Self> {
        Some(Self {
            inner: GlobalFlightComputer::new(config)?,
        })
    }

    pub const fn frozen_inner(&self) -> &GlobalFlightComputer {
        &self.inner
    }
}

impl GlobalKlr10FlightPackage for KsaG10rReferenceOpsV1 {
    fn manifest(&self) -> FlightSoftwarePackageManifest {
        ksa_g10r_reference_ops_manifest()
    }

    fn process_release(
        &mut self,
        fast: Option<GlobalFastSensorCell>,
        aid: Option<GlobalAidFrameCell>,
        transition: Option<GlobalTransitionCell>,
    ) -> GlobalFlightEvidence {
        self.inner.tick(fast, aid, transition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase10::ksa_g10r_reference_flight_config;
    use ksa64_interface::phase10::{
        GlobalFrameId, GLOBAL_FAST_ATTITUDE, GLOBAL_FAST_DELTA_ANGLE, GLOBAL_FAST_DELTA_V,
    };

    fn fast(epoch: u16) -> GlobalFastSensorCell {
        GlobalFastSensorCell {
            session: 0x10a0,
            measurement_epoch: epoch,
            production_epoch: epoch,
            frame: GlobalFrameId::LocalEnuV1,
            validity: GLOBAL_FAST_DELTA_V | GLOBAL_FAST_DELTA_ANGLE | GLOBAL_FAST_ATTITUDE,
            mission_time_q16: u32::from(epoch) * 2_048,
            delta_velocity_q24: [0, 0, 1],
            delta_angle_q24: [0; 3],
            attitude_vector_q15: [0; 3],
            angular_rate_q15: [0; 3],
            dynamic_pressure_q10: 0,
            mach_q12: 0,
            gimbal_applied_q15: [0; 2],
            rcs_propellant_q21: 5 << 21,
            actuator_feedback: 0,
            vehicle_status: 2,
            sensor_checksum: epoch,
        }
    }

    #[test]
    fn inactive_reference_wrapper_is_exactly_the_frozen_flight_computer() {
        let config = ksa_g10r_reference_flight_config();
        let mut frozen = GlobalFlightComputer::new(config).unwrap();
        let mut wrapped = KsaG10rReferenceOpsV1::new(config).unwrap();
        for epoch in 0..64u16 {
            let cell = fast(epoch);
            let expected = frozen.tick(Some(cell), None, None);
            let actual = wrapped.process_release(Some(cell), None, None);
            assert_eq!(actual, expected);
        }
        assert_eq!(
            wrapped.manifest().abi as u32,
            ksa64_interface::phase10::KLR10_CONTRACT_ID
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectReceiveError {
    Sequence,
    Identity,
    Capacity,
    Checksum,
    Contract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectReceiveState {
    Receiving,
    Complete,
}

pub struct BoundedObjectReceiver {
    bytes: [u8; 4_096],
    object_type: Option<ksa64_interface::phase11::PackageObjectType>,
    object_identity: u32,
    total_length: u16,
    total_crc32: u32,
    segment_count: u16,
    next_segment: u16,
    complete: bool,
}

impl BoundedObjectReceiver {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 4_096],
            object_type: None,
            object_identity: 0,
            total_length: 0,
            total_crc32: 0,
            segment_count: 0,
            next_segment: 0,
            complete: false,
        }
    }

    pub fn reset(&mut self) {
        self.object_type = None;
        self.object_identity = 0;
        self.total_length = 0;
        self.total_crc32 = 0;
        self.segment_count = 0;
        self.next_segment = 0;
        self.complete = false;
    }

    pub fn receive(
        &mut self,
        segment: &ksa64_interface::phase11::PackageObjectSegment<'_>,
    ) -> Result<ObjectReceiveState, ObjectReceiveError> {
        if segment.total_length == 0 || segment.total_length > self.bytes.len() as u32 {
            return Err(ObjectReceiveError::Capacity);
        }
        if segment.segment_index == 0 {
            self.reset();
            self.object_type = Some(segment.object_type);
            self.object_identity = segment.object_identity;
            self.total_length = segment.total_length as u16;
            self.total_crc32 = segment.total_crc32;
            self.segment_count = segment.segment_count;
        }
        if self.complete
            || self.object_type != Some(segment.object_type)
            || self.object_identity != segment.object_identity
            || u32::from(self.total_length) != segment.total_length
            || self.total_crc32 != segment.total_crc32
            || self.segment_count != segment.segment_count
        {
            self.reset();
            return Err(ObjectReceiveError::Identity);
        }
        if segment.segment_index != self.next_segment
            || usize::from(segment.logical_offset) + segment.payload.len()
                > usize::from(self.total_length)
        {
            self.reset();
            return Err(ObjectReceiveError::Sequence);
        }
        let start = usize::from(segment.logical_offset);
        let end = start + segment.payload.len();
        self.bytes[start..end].copy_from_slice(segment.payload);
        self.next_segment = self.next_segment.saturating_add(1);
        if self.next_segment != self.segment_count {
            return Ok(ObjectReceiveState::Receiving);
        }
        if ksa64_interface::crc32_ieee(&self.bytes[..usize::from(self.total_length)])
            != self.total_crc32
        {
            self.reset();
            return Err(ObjectReceiveError::Checksum);
        }
        self.complete = true;
        Ok(ObjectReceiveState::Complete)
    }

    pub fn activate_mission_plan(
        &self,
        manifest: &FlightSoftwarePackageManifest,
    ) -> Result<ksa64_interface::phase11::MissionPlan, ObjectReceiveError> {
        if !self.complete
            || self.object_type != Some(ksa64_interface::phase11::PackageObjectType::MissionPlan)
            || usize::from(self.total_length) != ksa64_interface::phase11::KMP11_LENGTH
        {
            return Err(ObjectReceiveError::Sequence);
        }
        let plan = ksa64_interface::phase11::parse_kmp11(
            &self.bytes[..ksa64_interface::phase11::KMP11_LENGTH],
        )
        .map_err(|_| ObjectReceiveError::Contract)?;
        if plan.package_manifest_identity != manifest.manifest_identity
            || plan.package != manifest.package
            || plan.abi != manifest.abi
            || plan.vehicle_profile_identity != manifest.vehicle_profile_identity
            || plan.mission_identity != manifest.mission_compatibility_identity
            || plan.required_capabilities & !manifest.capabilities != 0
            || plan.event_count > manifest.maximum_plan_events
            || plan.branch_count > manifest.maximum_branches
            || plan.decision_count > manifest.maximum_decisions
        {
            return Err(ObjectReceiveError::Identity);
        }
        Ok(plan)
    }
}

impl Default for BoundedObjectReceiver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod object_tests {
    use super::*;

    fn reference_plan() -> ksa64_interface::phase11::MissionPlan {
        use ksa64_interface::phase11::*;
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
            package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
            package: FlightSoftwarePackageId::KsaG10rReferenceOpsV1,
            abi: FlightAbiId::GlobalKlr10V1,
            vehicle_profile_identity: GLOBAL_ECEF_PROFILE_ID,
            mission_identity: KSA_G10R_MISSION_COMPATIBILITY_ID,
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

    fn reference_plan_bytes() -> [u8; ksa64_interface::phase11::KMP11_LENGTH] {
        let mut bytes = [0; ksa64_interface::phase11::KMP11_LENGTH];
        ksa64_interface::phase11::write_kmp11(&reference_plan(), &mut bytes).unwrap();
        bytes
    }

    #[test]
    fn incomplete_object_cannot_activate_and_corruption_never_replaces_state() {
        let manifest = ksa_g10r_reference_ops_manifest();
        let bytes = reference_plan_bytes();
        let crc = ksa64_interface::crc32_ieee(&bytes);
        let count = bytes
            .len()
            .div_ceil(ksa64_interface::phase11::KPX11_PAYLOAD_MAX) as u16;
        let mut receiver = BoundedObjectReceiver::new();
        for index in 0..count {
            let offset = usize::from(index) * ksa64_interface::phase11::KPX11_PAYLOAD_MAX;
            let end = (offset + ksa64_interface::phase11::KPX11_PAYLOAD_MAX).min(bytes.len());
            let segment = ksa64_interface::phase11::PackageObjectSegment {
                object_type: ksa64_interface::phase11::PackageObjectType::MissionPlan,
                object_identity: 0x11a0_0001,
                total_length: bytes.len() as u32,
                total_crc32: crc,
                segment_index: index,
                segment_count: count,
                logical_offset: offset as u16,
                payload: &bytes[offset..end],
            };
            let state = receiver.receive(&segment).unwrap();
            if index + 1 != count {
                assert_eq!(state, ObjectReceiveState::Receiving);
                assert_eq!(
                    receiver.activate_mission_plan(&manifest),
                    Err(ObjectReceiveError::Sequence)
                );
            } else {
                assert_eq!(state, ObjectReceiveState::Complete);
            }
        }
        assert_eq!(
            receiver
                .activate_mission_plan(&manifest)
                .unwrap()
                .event_count,
            1
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingUplink {
    load: ksa64_interface::phase11::UplinkCommandLoad,
    state: ksa64_interface::phase11::UplinkState,
    effective_epoch: u32,
}

pub struct AtomicUplinkManager {
    pending: Option<PendingUplink>,
    last_executed_identity: u32,
    receipt_chain: u32,
}

impl AtomicUplinkManager {
    pub const fn new() -> Self {
        Self {
            pending: None,
            last_executed_identity: 0,
            receipt_chain: 0x811c_9dc5,
        }
    }

    pub const fn state(&self) -> ksa64_interface::phase11::UplinkState {
        match self.pending {
            Some(value) => value.state,
            None => ksa64_interface::phase11::UplinkState::Empty,
        }
    }

    pub fn stage(
        &mut self,
        load: ksa64_interface::phase11::UplinkCommandLoad,
        current_epoch: u32,
        completed_event_mask: u32,
        navigation: &crate::phase10::GlobalNavigation,
        manifest: &FlightSoftwarePackageManifest,
        plan: &ksa64_interface::phase11::MissionPlan,
    ) -> ksa64_interface::phase11::UplinkControlRecord {
        use ksa64_interface::phase11::{UplinkReasonCode as Reason, UplinkState as State};
        if self.last_executed_identity == load.load_identity {
            return self.receipt(
                ksa64_interface::phase11::UplinkControlKind::StageReceipt,
                &load,
                current_epoch,
                current_epoch,
                State::Executed,
                Reason::AlreadyApplied,
            );
        }
        if let Some(pending) = self.pending {
            if pending.load == load {
                return self.receipt(
                    ksa64_interface::phase11::UplinkControlKind::StageReceipt,
                    &load,
                    current_epoch,
                    pending.effective_epoch,
                    pending.state,
                    Reason::Accepted,
                );
            }
            return self.receipt(
                ksa64_interface::phase11::UplinkControlKind::StageReceipt,
                &load,
                current_epoch,
                current_epoch,
                State::Rejected,
                Reason::Occupied,
            );
        }
        let reason = self.validate_stage(
            &load,
            current_epoch,
            completed_event_mask,
            navigation,
            manifest,
            plan,
        );
        if reason != Reason::Accepted {
            return self.receipt(
                ksa64_interface::phase11::UplinkControlKind::StageReceipt,
                &load,
                current_epoch,
                current_epoch,
                State::Rejected,
                reason,
            );
        }
        self.pending = Some(PendingUplink {
            load,
            state: State::Staged,
            effective_epoch: load.requested_effective_epoch,
        });
        self.receipt(
            ksa64_interface::phase11::UplinkControlKind::StageReceipt,
            &load,
            current_epoch,
            load.requested_effective_epoch,
            State::Staged,
            Reason::Accepted,
        )
    }

    fn validate_stage(
        &self,
        load: &ksa64_interface::phase11::UplinkCommandLoad,
        current_epoch: u32,
        completed_event_mask: u32,
        navigation: &crate::phase10::GlobalNavigation,
        manifest: &FlightSoftwarePackageManifest,
        plan: &ksa64_interface::phase11::MissionPlan,
    ) -> ksa64_interface::phase11::UplinkReasonCode {
        use ksa64_interface::phase11::UplinkReasonCode as Reason;
        if load.package_manifest_identity != manifest.manifest_identity
            || load.plan_identity != plan.plan_identity
            || load.abi != manifest.abi
        {
            return Reason::Identity;
        }
        if load.frame != navigation.frame {
            return Reason::Frame;
        }
        if load.stage_epoch != current_epoch || load.expires_epoch < current_epoch {
            return Reason::Stale;
        }
        if load.requested_effective_epoch < current_epoch.saturating_add(2)
            || load.requested_effective_epoch < load.not_before_epoch
            || load.requested_effective_epoch > load.expires_epoch
        {
            return Reason::Late;
        }
        if manifest.command_load_support & load.load_type.support_bit() == 0 {
            return Reason::Unsupported;
        }
        if manifest.capabilities & load.required_capabilities != load.required_capabilities
            || plan.required_capabilities & load.load_type.capability() == 0
        {
            return Reason::Capability;
        }
        if completed_event_mask & load.prerequisite_event_mask != load.prerequisite_event_mask {
            return Reason::Prerequisite;
        }
        if load.load_type == ksa64_interface::phase11::UplinkLoadType::GroundNavigationUpdate {
            for axis in 0..3 {
                if residual(load.arguments[axis], navigation.position_q12[axis])
                    > load.position_residual_limit_q12 as u64
                    || residual(load.arguments[axis + 3], navigation.velocity_q24[axis])
                        > load.velocity_residual_limit_q24 as u64
                {
                    return Reason::Residual;
                }
            }
        }
        Reason::Accepted
    }

    pub fn commit(
        &mut self,
        request: &ksa64_interface::phase11::UplinkControlRecord,
        current_epoch: u32,
    ) -> ksa64_interface::phase11::UplinkControlRecord {
        use ksa64_interface::phase11::{
            UplinkControlKind as Kind, UplinkReasonCode as Reason, UplinkState as State,
        };
        let Some(mut pending) = self.pending else {
            return *request;
        };
        if request.kind != Kind::CommitRequest
            || request.load_identity != pending.load.load_identity
            || request.package_manifest_identity != pending.load.package_manifest_identity
            || request.plan_identity != pending.load.plan_identity
        {
            return self.receipt(
                Kind::CommitReceipt,
                &pending.load,
                current_epoch,
                request.effective_epoch,
                State::Rejected,
                Reason::Identity,
            );
        }
        if pending.state == State::Committed {
            return self.receipt(
                Kind::CommitReceipt,
                &pending.load,
                current_epoch,
                pending.effective_epoch,
                State::Committed,
                Reason::Accepted,
            );
        }
        if request.effective_epoch != pending.load.requested_effective_epoch
            || request.effective_epoch < current_epoch.saturating_add(2)
            || request.effective_epoch < pending.load.not_before_epoch
            || request.effective_epoch > pending.load.expires_epoch
        {
            return self.receipt(
                Kind::CommitReceipt,
                &pending.load,
                current_epoch,
                request.effective_epoch,
                State::Rejected,
                Reason::Late,
            );
        }
        pending.state = State::Committed;
        pending.effective_epoch = request.effective_epoch;
        self.pending = Some(pending);
        self.receipt(
            Kind::CommitReceipt,
            &pending.load,
            current_epoch,
            pending.effective_epoch,
            State::Committed,
            Reason::Accepted,
        )
    }

    pub fn cancel(
        &mut self,
        request: &ksa64_interface::phase11::UplinkControlRecord,
        current_epoch: u32,
    ) -> ksa64_interface::phase11::UplinkControlRecord {
        use ksa64_interface::phase11::{
            UplinkControlKind as Kind, UplinkReasonCode as Reason, UplinkState as State,
        };
        let Some(pending) = self.pending else {
            return *request;
        };
        if request.kind != Kind::Cancellation
            || request.load_identity != pending.load.load_identity
            || request.package_manifest_identity != pending.load.package_manifest_identity
            || request.plan_identity != pending.load.plan_identity
        {
            return self.receipt(
                Kind::Cancellation,
                &pending.load,
                current_epoch,
                current_epoch,
                State::Rejected,
                Reason::Identity,
            );
        }
        self.pending = None;
        self.receipt(
            Kind::Cancellation,
            &pending.load,
            current_epoch,
            current_epoch,
            State::Cancelled,
            Reason::Accepted,
        )
    }

    pub fn release(
        &mut self,
        epoch: u32,
    ) -> Option<(
        ksa64_interface::phase11::UplinkCommandLoad,
        ksa64_interface::phase11::UplinkControlRecord,
    )> {
        use ksa64_interface::phase11::{
            UplinkControlKind as Kind, UplinkReasonCode as Reason, UplinkState as State,
        };
        let pending = self.pending?;
        if epoch > pending.load.expires_epoch {
            self.pending = None;
            return None;
        }
        if pending.state != State::Committed || epoch != pending.effective_epoch {
            return None;
        }
        self.pending = None;
        self.last_executed_identity = pending.load.load_identity;
        let receipt = self.receipt(
            Kind::ExecutionAcknowledgement,
            &pending.load,
            epoch,
            epoch,
            State::Executed,
            Reason::Accepted,
        );
        Some((pending.load, receipt))
    }

    fn receipt(
        &mut self,
        kind: ksa64_interface::phase11::UplinkControlKind,
        load: &ksa64_interface::phase11::UplinkCommandLoad,
        request_epoch: u32,
        effective_epoch: u32,
        state: ksa64_interface::phase11::UplinkState,
        reason: ksa64_interface::phase11::UplinkReasonCode,
    ) -> ksa64_interface::phase11::UplinkControlRecord {
        let control_identity = load.load_identity ^ request_epoch.rotate_left(11) ^ kind as u32;
        self.receipt_chain = hash_receipt(
            self.receipt_chain,
            load.load_identity,
            request_epoch,
            effective_epoch,
            state as u8,
            reason as u8,
        );
        ksa64_interface::phase11::UplinkControlRecord {
            kind,
            control_identity: control_identity.max(1),
            load_identity: load.load_identity,
            package_manifest_identity: load.package_manifest_identity,
            plan_identity: load.plan_identity,
            request_epoch,
            effective_epoch,
            state,
            reason,
            receipt_checksum: self.receipt_chain,
        }
    }
}

impl Default for AtomicUplinkManager {
    fn default() -> Self {
        Self::new()
    }
}

fn residual(left: i32, right: i32) -> u64 {
    (i64::from(left) - i64::from(right)).unsigned_abs()
}

fn hash_receipt(
    mut hash: u32,
    load: u32,
    request_epoch: u32,
    effective_epoch: u32,
    state: u8,
    reason: u8,
) -> u32 {
    for value in [
        load,
        request_epoch,
        effective_epoch,
        u32::from(state),
        u32::from(reason),
    ] {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    hash
}

#[cfg(test)]
mod uplink_tests {
    use super::*;
    use ksa64_interface::phase10::GlobalFrameId;
    use ksa64_interface::phase11::*;

    fn plan() -> MissionPlan {
        MissionPlan {
            plan_identity: 0x11a0_0001,
            package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
            package: FlightSoftwarePackageId::KsaG10rReferenceOpsV1,
            abi: FlightAbiId::GlobalKlr10V1,
            vehicle_profile_identity: GLOBAL_ECEF_PROFILE_ID,
            mission_identity: KSA_G10R_MISSION_COMPATIBILITY_ID,
            earth_identity: 0x10e0_0001,
            environment_identity: 0x10a7_0001,
            onboard_prediction_model: 0x11d0_0001,
            ground_prediction_model: 0x11d0_0002,
            required_capabilities: PACKAGE_CAP_MISSION_PLAN | PACKAGE_CAP_GROUND_NAV_UPDATE,
            event_count: 0,
            branch_count: 0,
            decision_count: 0,
            events: [MissionPlanEvent::EMPTY; KMP11_MAX_EVENTS],
            branches: [ContingencyBranch::EMPTY; KMP11_MAX_BRANCHES],
            decisions: [OperatorDecisionPoint::EMPTY; KMP11_MAX_DECISIONS],
        }
    }

    fn load(epoch: u32, identity: u32) -> UplinkCommandLoad {
        UplinkCommandLoad {
            load_identity: identity,
            package_manifest_identity: KSA_G10R_REFERENCE_OPS_MANIFEST_ID,
            plan_identity: 0x11a0_0001,
            abi: FlightAbiId::GlobalKlr10V1,
            source_estimator_identity: 0x11e0_0001,
            source_estimator_checksum: 0x1234_5678,
            stage_epoch: epoch,
            not_before_epoch: epoch + 2,
            expires_epoch: epoch + 20,
            requested_effective_epoch: epoch + 4,
            required_capabilities: PACKAGE_CAP_GROUND_NAV_UPDATE,
            prerequisite_event_mask: 0,
            position_residual_limit_q12: 4_096,
            velocity_residual_limit_q24: 16_777,
            frame: GlobalFrameId::LocalEnuV1,
            load_type: UplinkLoadType::GroundNavigationUpdate,
            arguments: [0; 16],
        }
    }

    fn navigation() -> crate::phase10::GlobalNavigation {
        GlobalFlightComputer::new(crate::phase10::ksa_g10r_reference_flight_config())
            .unwrap()
            .navigation()
    }

    fn commit_request(load: &UplinkCommandLoad, request_epoch: u32) -> UplinkControlRecord {
        UplinkControlRecord {
            kind: UplinkControlKind::CommitRequest,
            control_identity: load.load_identity ^ 0x55,
            load_identity: load.load_identity,
            package_manifest_identity: load.package_manifest_identity,
            plan_identity: load.plan_identity,
            request_epoch,
            effective_epoch: load.requested_effective_epoch,
            state: UplinkState::Staged,
            reason: UplinkReasonCode::Accepted,
            receipt_checksum: 0,
        }
    }

    #[test]
    fn load_cannot_execute_before_separate_commit_and_executes_exactly_on_release() {
        let manifest = ksa_g10r_reference_ops_manifest();
        let plan = plan();
        let navigation = navigation();
        let load = load(100, 0x11c0_0001);
        let mut manager = AtomicUplinkManager::new();
        let staged = manager.stage(load, 100, 0, &navigation, &manifest, &plan);
        assert_eq!(staged.state, UplinkState::Staged);
        assert_eq!(manager.release(101), None);
        assert_eq!(manager.release(102), None);

        let committed = manager.commit(&commit_request(&load, 101), 101);
        assert_eq!(committed.state, UplinkState::Committed);
        assert_eq!(manager.release(103), None);
        let (executed, acknowledgement) = manager.release(104).unwrap();
        assert_eq!(executed, load);
        assert_eq!(acknowledgement.state, UplinkState::Executed);
        assert_eq!(manager.release(105), None);

        let duplicate = manager.stage(load, 106, 0, &navigation, &manifest, &plan);
        assert_eq!(duplicate.reason, UplinkReasonCode::AlreadyApplied);
        assert_eq!(duplicate.state, UplinkState::Executed);
    }

    #[test]
    fn staged_uncommitted_load_expires_without_execution_during_blackout() {
        let manifest = ksa_g10r_reference_ops_manifest();
        let plan = plan();
        let navigation = navigation();
        let load = load(200, 0x11c0_0002);
        let mut manager = AtomicUplinkManager::new();
        assert_eq!(
            manager
                .stage(load, 200, 0, &navigation, &manifest, &plan)
                .state,
            UplinkState::Staged
        );
        for epoch in 201..=221 {
            assert_eq!(manager.release(epoch), None);
        }
        assert_eq!(manager.state(), UplinkState::Empty);
    }

    #[test]
    fn identity_frame_residual_and_late_commit_fail_closed() {
        let manifest = ksa_g10r_reference_ops_manifest();
        let plan = plan();
        let navigation = navigation();
        let mut excessive = load(300, 0x11c0_0003);
        excessive.arguments[0] = 10_000;
        excessive.position_residual_limit_q12 = 1;
        let mut manager = AtomicUplinkManager::new();
        let receipt = manager.stage(excessive, 300, 0, &navigation, &manifest, &plan);
        assert_eq!(receipt.reason, UplinkReasonCode::Residual);
        assert_eq!(manager.state(), UplinkState::Empty);

        let valid = load(300, 0x11c0_0004);
        assert_eq!(
            manager
                .stage(valid, 300, 0, &navigation, &manifest, &plan)
                .state,
            UplinkState::Staged
        );
        let late = manager.commit(&commit_request(&valid, 303), 303);
        assert_eq!(late.reason, UplinkReasonCode::Late);
        assert_eq!(manager.state(), UplinkState::Staged);
    }
}
