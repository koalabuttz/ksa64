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
