//! Additive Phase 12B presentation ABI.
//!
//! These fixed-width structures are noncanonical views. They deliberately
//! contain no private world-truth fields; role filtering happens before data
//! reaches this boundary.

use std::mem::size_of;

pub const PRESENTATION_VERSION: u32 = 1;
pub const SCENARIO_LEGACY_GNSS_FIXTURE: u32 = 0x120a_0001;
pub const SCENARIO_FULL_GNSS_LOSS: u32 = 0x12b0_0001;
pub const KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE: u32 = 1;
pub const KSA64_VIEWER_TRAJECTORY_ONBOARD_ESTIMATE: u32 = 2;
pub const KSA64_VIEWER_TRAJECTORY_GROUND_ESTIMATE: u32 = 3;
pub const KSA64_VIEWER_TRAJECTORY_PRODUCT_PLANNED_REFERENCE: u32 = 5;
pub const START_FLAG_CONTINUOUS: u32 = 1 << 0;
pub const START_FLAG_MASK: u32 = START_FLAG_CONTINUOUS;

pub const VIEW_VALID_MISSION_TIME: u64 = 1 << 0;
pub const VIEW_VALID_NAVIGATION: u64 = 1 << 1;
pub const VIEW_VALID_GROUND_ESTIMATE: u64 = 1 << 2;
pub const VIEW_VALID_PREDICTION: u64 = 1 << 3;
pub const VIEW_VALID_PROCEDURE: u64 = 1 << 4;
pub const VIEW_VALID_ACTION: u64 = 1 << 5;
pub const VIEW_VALID_DISPOSITION: u64 = 1 << 6;
pub const VIEW_VALID_EVIDENCE: u64 = 1 << 7;
pub const VIEW_VALID_GNSS: u64 = 1 << 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartRequestV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub scenario_identity: u32,
    pub role: u32,
    pub initial_pace: u32,
    pub flags: u32,
    pub reserved: [u32; 6],
}
impl Default for StartRequestV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            scenario_identity: SCENARIO_FULL_GNSS_LOSS,
            role: 2,
            initial_pace: 2,
            flags: START_FLAG_CONTINUOUS,
            reserved: [0; 6],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationalViewV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub publication_sequence: u64,
    pub scenario_identity: u32,
    pub execution_adapter_identity: u32,
    pub role: u32,
    pub lifecycle: u32,
    pub pace: u32,
    pub release_epoch: u32,
    pub release_period_micros: u32,
    pub frame: u32,
    pub mission_time_q16: u32,
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub ground_position_q12: [i32; 3],
    pub ground_velocity_q24: [i32; 3],
    pub flight_checksum: u32,
    pub navigation_checksum: u32,
    pub command_checksum: u32,
    pub procedure_state: u32,
    pub procedure_step: u32,
    pub staged_load_identity: u32,
    pub action_count: u32,
    pub rejected_loads: u32,
    pub safe: u32,
    pub gnss_state: u32,
    pub prediction_identity: u32,
    pub prediction_checksum: u32,
    pub prediction_apogee_q12_km: i32,
    pub prediction_time_to_apogee_q16: u32,
    pub prediction_time_to_impact_q16: u32,
    pub presentation_flags: u32,
    pub reserved: [u32; 8],
}
impl Default for OperationalViewV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            publication_sequence: 0,
            scenario_identity: 0,
            execution_adapter_identity: 0,
            role: 0,
            lifecycle: 0,
            pace: 0,
            release_epoch: 0,
            release_period_micros: 0,
            frame: 0,
            mission_time_q16: 0,
            navigation_position_q12: [0; 3],
            navigation_velocity_q24: [0; 3],
            ground_position_q12: [0; 3],
            ground_velocity_q24: [0; 3],
            flight_checksum: 0,
            navigation_checksum: 0,
            command_checksum: 0,
            procedure_state: 0,
            procedure_step: 0,
            staged_load_identity: 0,
            action_count: 0,
            rejected_loads: 0,
            safe: 0,
            gnss_state: 0,
            prediction_identity: 0,
            prediction_checksum: 0,
            prediction_apogee_q12_km: 0,
            prediction_time_to_apogee_q16: 0,
            prediction_time_to_impact_q16: 0,
            presentation_flags: 0,
            reserved: [0; 8],
        }
    }
}

pub const PROCEDURE_PREDICATE_CAPACITY: usize = 8;
pub const PROCEDURE_TITLE_CAPACITY: usize = 64;
pub const PROCEDURE_INSTRUCTION_CAPACITY: usize = 192;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedureViewV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub procedure_identity: u32,
    pub state: u32,
    pub active_step: u32,
    pub step_count: u32,
    pub entered_epoch: u32,
    pub deadline_epoch: u32,
    pub predicate_count: u32,
    pub predicate_identities: [u32; PROCEDURE_PREDICATE_CAPACITY],
    pub predicate_states: [u32; PROCEDURE_PREDICATE_CAPACITY],
    pub title_length: u32,
    pub instruction_length: u32,
    pub title: [u8; PROCEDURE_TITLE_CAPACITY],
    pub instruction: [u8; PROCEDURE_INSTRUCTION_CAPACITY],
}
impl Default for ProcedureViewV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            procedure_identity: 0,
            state: 0,
            active_step: 0,
            step_count: 0,
            entered_epoch: 0,
            deadline_epoch: 0,
            predicate_count: 0,
            predicate_identities: [0; PROCEDURE_PREDICATE_CAPACITY],
            predicate_states: [0; PROCEDURE_PREDICATE_CAPACITY],
            title_length: 0,
            instruction_length: 0,
            title: [0; PROCEDURE_TITLE_CAPACITY],
            instruction: [0; PROCEDURE_INSTRUCTION_CAPACITY],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispositionV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub overall: u32,
    pub objective: u32,
    pub vehicle: u32,
    pub procedure: u32,
    pub operator: u32,
    pub avionics: u32,
    pub evidence: u32,
    pub reason_identity: u32,
    pub reserved: [u32; 5],
}
impl Default for DispositionV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            overall: 0,
            objective: 0,
            vehicle: 0,
            procedure: 0,
            operator: 0,
            avionics: 0,
            evidence: 0,
            reason_identity: 0,
            reserved: [0; 5],
        }
    }
}

pub const ACTION_LABEL_CAPACITY: usize = 80;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionProposalV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub proposal_identity: u32,
    pub load_identity: u32,
    pub load_type: u32,
    pub stage_epoch: u32,
    pub earliest_commit_epoch: u32,
    pub activation_epoch: u32,
    pub expires_epoch: u32,
    pub payload_checksum: u32,
    pub completed_event_mask: u32,
    pub permitted_operations: u32,
    pub label_length: u32,
    pub label: [u8; ACTION_LABEL_CAPACITY],
}
impl Default for ActionProposalV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            proposal_identity: 0,
            load_identity: 0,
            load_type: 0,
            stage_epoch: 0,
            earliest_commit_epoch: 0,
            activation_epoch: 0,
            expires_epoch: 0,
            payload_checksum: 0,
            completed_event_mask: 0,
            permitted_operations: 0,
            label_length: 0,
            label: [0; ACTION_LABEL_CAPACITY],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionReceiptV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub publication_sequence: u64,
    pub proposal_identity: u32,
    pub load_identity: u32,
    pub control_identity: u32,
    pub receipt_epoch: u32,
    pub effective_epoch: u32,
    pub state: u32,
    pub reason: u32,
    pub accepted: u32,
    pub operation: u32,
    pub receipt_checksum: u32,
    pub reserved: [u32; 4],
}
impl Default for ActionReceiptV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            publication_sequence: 0,
            proposal_identity: 0,
            load_identity: 0,
            control_identity: 0,
            receipt_epoch: 0,
            effective_epoch: 0,
            state: 0,
            reason: 0,
            accepted: 0,
            operation: 0,
            receipt_checksum: 0,
            reserved: [0; 4],
        }
    }
}

pub const TIMELINE_LABEL_CAPACITY: usize = 96;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineEventV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub sequence: u32,
    pub release_epoch: u32,
    pub source: u32,
    pub severity: u32,
    pub event_identity: u32,
    pub detail_identity: u32,
    pub label_length: u32,
    pub flags: u32,
    pub label: [u8; TIMELINE_LABEL_CAPACITY],
}
impl Default for TimelineEventV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            sequence: 0,
            release_epoch: 0,
            source: 0,
            severity: 0,
            event_identity: 0,
            detail_identity: 0,
            label_length: 0,
            flags: 0,
            label: [0; TIMELINE_LABEL_CAPACITY],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseSampleV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub release_epoch: u32,
    pub mission_time_q16: u32,
    pub frame: u32,
    pub flags: u32,
    pub onboard_position_q12: [i32; 3],
    pub onboard_velocity_q24: [i32; 3],
    pub ground_position_q12: [i32; 3],
    pub ground_velocity_q24: [i32; 3],
    pub predicted_impact_q12: [i32; 3],
    pub predicted_apogee_q12_km: i32,
    pub altitude_q12_km: i32,
    pub speed_q24_km_s: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
}
impl Default for ReleaseSampleV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            release_epoch: 0,
            mission_time_q16: 0,
            frame: 0,
            flags: 0,
            onboard_position_q12: [0; 3],
            onboard_velocity_q24: [0; 3],
            ground_position_q12: [0; 3],
            ground_velocity_q24: [0; 3],
            predicted_impact_q12: [0; 3],
            predicted_apogee_q12_km: 0,
            altitude_q12_km: 0,
            speed_q24_km_s: 0,
            downrange_q12_km: 0,
            crossrange_q12_km: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PredictionPathHeaderV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub path_identity: u32,
    pub product: u32,
    pub model_identity: u32,
    pub source_estimate_identity: u32,
    pub source_estimate_checksum: u32,
    pub source_epoch: u32,
    pub generation_epoch: u32,
    pub frame: u32,
    pub terminal_reason: u32,
    pub point_count: u32,
    pub cadence_releases: u32,
    pub path_checksum: u32,
    pub reserved: [u32; 5],
}
impl Default for PredictionPathHeaderV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            path_identity: 0,
            product: 0,
            model_identity: 0,
            source_estimate_identity: 0,
            source_estimate_checksum: 0,
            source_epoch: 0,
            generation_epoch: 0,
            frame: 0,
            terminal_reason: 0,
            point_count: 0,
            cadence_releases: 0,
            path_checksum: 0,
            reserved: [0; 5],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PredictionPathPointV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub path_identity: u32,
    pub point_index: u32,
    pub release_epoch: u32,
    pub frame: u32,
    pub flags: u32,
    pub reserved0: u32,
    pub position_q12_km: [i32; 3],
    pub altitude_q12_km: i32,
    pub downrange_q12_km: i32,
    pub crossrange_q12_km: i32,
}
impl Default for PredictionPathPointV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            path_identity: 0,
            point_index: 0,
            release_epoch: 0,
            frame: 0,
            flags: 0,
            reserved0: 0,
            position_q12_km: [0; 3],
            altitude_q12_km: 0,
            downrange_q12_km: 0,
            crossrange_q12_km: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportStatusV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub command_capacity: u32,
    pub commands_pending: u32,
    pub event_capacity: u32,
    pub events_pending: u32,
    pub timeline_capacity: u32,
    pub timeline_pending: u32,
    pub sample_capacity: u32,
    pub samples_pending: u32,
    pub worker_state: u32,
    pub shutdown_requested: u32,
    pub finalization_state: u32,
    pub event_overflow: u32,
    pub timeline_overflow: u32,
    pub sample_overflow: u32,
    pub last_command_result: i32,
    pub reserved: [u32; 5],
}
impl Default for TransportStatusV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            command_capacity: super::KSA64_VIEWER_COMMAND_CAPACITY as u32,
            commands_pending: 0,
            event_capacity: super::KSA64_VIEWER_EVENT_CAPACITY as u32,
            events_pending: 0,
            timeline_capacity: super::KSA64_VIEWER_EVENT_CAPACITY as u32,
            timeline_pending: 0,
            sample_capacity: super::KSA64_VIEWER_EVENT_CAPACITY as u32,
            samples_pending: 0,
            worker_state: 0,
            shutdown_requested: 0,
            finalization_state: 0,
            event_overflow: 0,
            timeline_overflow: 0,
            sample_overflow: 0,
            last_command_result: 0,
            reserved: [0; 5],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinishStatusV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub validity_mask: u64,
    pub lifecycle: u32,
    pub finalization_state: u32,
    pub shutdown_state: u32,
    pub evidence_identity: u32,
    pub evidence_length: u64,
    pub evidence_crc32: u32,
    pub reserved: [u32; 5],
}
impl Default for FinishStatusV1 {
    fn default() -> Self {
        Self {
            abi_version: super::KSA64_VIEWER_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            validity_mask: 0,
            lifecycle: 0,
            finalization_state: 0,
            shutdown_state: 0,
            evidence_identity: 0,
            evidence_length: 0,
            evidence_crc32: 0,
            reserved: [0; 5],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_layouts_are_frozen_for_abi_v1() {
        assert_eq!(size_of::<StartRequestV1>(), 48);
        assert_eq!(size_of::<OperationalViewV1>(), 208);
        assert_eq!(size_of::<ProcedureViewV1>(), 376);
        assert_eq!(size_of::<DispositionV1>(), 72);
        assert_eq!(size_of::<ActionProposalV1>(), 144);
        assert_eq!(size_of::<ActionReceiptV1>(), 80);
        assert_eq!(size_of::<TimelineEventV1>(), 136);
        assert_eq!(size_of::<ReleaseSampleV1>(), 112);
        assert_eq!(size_of::<PredictionPathHeaderV1>(), 88);
        assert_eq!(size_of::<PredictionPathPointV1>(), 56);
        assert_eq!(size_of::<TransportStatusV1>(), 96);
        assert_eq!(size_of::<FinishStatusV1>(), 64);
    }
}
