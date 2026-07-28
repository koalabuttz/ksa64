#ifndef KSA64_VIEWER_BRIDGE_H
#define KSA64_VIEWER_BRIDGE_H
#include <stdint.h>
#include <stddef.h>
#if defined(_WIN32)
#define KSA64_VIEWER_CALL __cdecl
#if defined(KSA64_VIEWER_BUILD)
#define KSA64_VIEWER_API __declspec(dllexport)
#else
#define KSA64_VIEWER_API
#endif
#elif defined(__GNUC__) || defined(__clang__)
#define KSA64_VIEWER_CALL
#define KSA64_VIEWER_API __attribute__((visibility("default")))
#else
#define KSA64_VIEWER_CALL
#define KSA64_VIEWER_API
#endif
#ifdef __cplusplus
extern "C" {
#endif

#define KSA64_VIEWER_ABI_VERSION 1u
#define KSA64_VIEWER_BUILD_IDENTITY 0x120B0001u
#define KSA64_VIEWER_MAX_ADVANCE_RELEASES 64u
#define KSA64_VIEWER_MAX_CALLER_SPAN 16777216ull
#define KSA64_VIEWER_OK 0
#define KSA64_VIEWER_QUEUED 1
#define KSA64_VIEWER_NO_DATA 2
#define KSA64_VIEWER_UNCHANGED 3
#define KSA64_VIEWER_INVALID_ARGUMENT -1
#define KSA64_VIEWER_ABI_MISMATCH -2
#define KSA64_VIEWER_STRUCT_SIZE -3
#define KSA64_VIEWER_INVALID_UTF8 -4
#define KSA64_VIEWER_UNSUPPORTED -5
#define KSA64_VIEWER_LIFECYCLE -6
#define KSA64_VIEWER_ACTION_UNAVAILABLE -7
#define KSA64_VIEWER_ACTION_REJECTED -8
#define KSA64_VIEWER_QUEUE_FULL -9
#define KSA64_VIEWER_CLOSED -10
#define KSA64_VIEWER_INTERNAL -11
#define KSA64_VIEWER_PANIC -12
#define KSA64_VIEWER_EVENT_OVERFLOW -13

typedef struct Ksa64ViewerHandle Ksa64ViewerHandle;

#define KSA64_VIEWER_FEATURE_PANIC_PROBE 0x00000001u
#define KSA64_VIEWER_FEATURE_OPERATIONS_V1 0x00000002u
#define KSA64_VIEWER_FEATURE_TYPED_ACTIONS_V1 0x00000004u
#define KSA64_VIEWER_FEATURE_ASYNC_STATUS_V1 0x00000008u
#define KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1 0x00000010u
#define KSA64_VIEWER_SCENARIO_LEGACY_GNSS_FIXTURE 0x120A0001u
#define KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS 0x12B00001u
#define KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE 1u
#define KSA64_VIEWER_TRAJECTORY_ONBOARD_ESTIMATE 2u
#define KSA64_VIEWER_TRAJECTORY_GROUND_ESTIMATE 3u
#define KSA64_VIEWER_TRAJECTORY_PRODUCT_PLANNED_REFERENCE 5u

typedef struct {
 uint32_t abi_version, struct_size, scenario_identity, role, initial_pace, flags;
 uint32_t reserved[6];
} Ksa64ViewerStartRequestV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask, publication_sequence;
 uint32_t scenario_identity, execution_adapter_identity, role, lifecycle, pace, release_epoch, release_period_micros, frame, mission_time_q16;
 int32_t navigation_position_q12[3], navigation_velocity_q24[3], ground_position_q12[3], ground_velocity_q24[3];
 uint32_t flight_checksum, navigation_checksum, command_checksum, procedure_state, procedure_step, staged_load_identity, action_count, rejected_loads, safe, gnss_state, prediction_identity, prediction_checksum;
 int32_t prediction_apogee_q12_km;
 uint32_t prediction_time_to_apogee_q16, prediction_time_to_impact_q16, presentation_flags, reserved[8];
} Ksa64ViewerOperationalViewV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask;
 uint32_t procedure_identity, state, active_step, step_count, entered_epoch, deadline_epoch, predicate_count;
 uint32_t predicate_identities[8], predicate_states[8], title_length, instruction_length;
 uint8_t title[64], instruction[192];
} Ksa64ViewerProcedureViewV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask;
 uint32_t overall, objective, vehicle, procedure, operator_disposition, avionics, evidence, reason_identity, reserved[5];
} Ksa64ViewerDispositionV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask;
 uint32_t proposal_identity, load_identity, load_type, stage_epoch, earliest_commit_epoch, activation_epoch, expires_epoch, payload_checksum, completed_event_mask, permitted_operations, label_length;
 uint8_t label[80];
} Ksa64ViewerActionProposalV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask, publication_sequence;
 uint32_t proposal_identity, load_identity, control_identity, receipt_epoch, effective_epoch, state, reason, accepted, operation, receipt_checksum, reserved[4];
} Ksa64ViewerActionReceiptV1;
typedef struct {
 uint32_t abi_version, struct_size, sequence, release_epoch, source, severity, event_identity, detail_identity, label_length, flags;
 uint8_t label[96];
} Ksa64ViewerTimelineEventV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask;
 uint32_t release_epoch, mission_time_q16, frame, flags;
 int32_t onboard_position_q12[3], onboard_velocity_q24[3], ground_position_q12[3], ground_velocity_q24[3], predicted_impact_q12[3], predicted_apogee_q12_km;
 int32_t altitude_q12_km, speed_q24_km_s, downrange_q12_km, crossrange_q12_km;
} Ksa64ViewerReleaseSampleV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask;
 uint32_t path_identity, product, model_identity, source_estimate_identity, source_estimate_checksum, source_epoch, generation_epoch, frame, terminal_reason, point_count, cadence_releases, path_checksum, reserved[5];
} Ksa64ViewerPredictionPathHeaderV1;
typedef struct {
 uint32_t abi_version, struct_size, path_identity, point_index, release_epoch, frame, flags, reserved0;
 int32_t position_q12_km[3], altitude_q12_km, downrange_q12_km, crossrange_q12_km;
} Ksa64ViewerPredictionPathPointV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask;
 uint32_t command_capacity, commands_pending, event_capacity, events_pending, timeline_capacity, timeline_pending, sample_capacity, samples_pending, worker_state, shutdown_requested, finalization_state, event_overflow, timeline_overflow, sample_overflow;
 int32_t last_command_result;
 uint32_t reserved[5];
} Ksa64ViewerTransportStatusV1;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask;
 uint32_t lifecycle, finalization_state, shutdown_state, evidence_identity;
 uint64_t evidence_length;
 uint32_t evidence_crc32, reserved[5];
} Ksa64ViewerFinishStatusV1;

typedef struct { uint32_t abi_version, struct_size, build_identity, release_hz, command_capacity, event_capacity, maximum_advance_releases, feature_flags, catalog_count, snapshot_size, event_size, span_size, owned_buffer_size; uint8_t source_commit[16], target_triple[32], catalog_sha256[32]; } Ksa64ViewerAbiInfo;
typedef struct { uint32_t abi_version, struct_size; const uint8_t* data; uint64_t length; } Ksa64ViewerSpan;
typedef struct { uint32_t abi_version, struct_size; uint8_t* data; uint64_t length, allocation_id; } Ksa64ViewerOwnedBuffer;
typedef struct { uint32_t abi_version, struct_size, sequence, release_epoch, kind, detail_identity; } Ksa64ViewerEvent;
typedef struct {
 uint32_t abi_version, struct_size; uint64_t validity_mask, command_sequence; int32_t command_result; uint32_t role;
 uint32_t definition_identity, lifecycle, pace, release_epoch, release_period_micros, frame, mission_time_q16;
 int32_t navigation_position_q12[3], navigation_velocity_q24[3];
 uint32_t flight_checksum, navigation_checksum, command_checksum, evidence_identity;
 uint32_t procedure_chain, journal_chain, action_chain, procedure_state, procedure_step;
 uint32_t staged_load_identity, action_count, event_count, rejected_loads, safe;
 uint32_t prediction_identity, prediction_checksum, prediction_frame, prediction_terminal_reason;
 int32_t prediction_apogee_q12_km, prediction_perigee_q12_km;
 uint32_t prediction_time_to_apogee_q16, prediction_time_to_impact_q16;
 int32_t prediction_impact_position_q12_km[3];
} Ksa64ViewerSnapshot;


#ifdef __cplusplus

static_assert(sizeof(Ksa64ViewerStartRequestV1) == 48, "start request v1 ABI drift");
static_assert(sizeof(Ksa64ViewerOperationalViewV1) == 208, "operational view v1 ABI drift");
static_assert(sizeof(Ksa64ViewerProcedureViewV1) == 376, "procedure view v1 ABI drift");
static_assert(sizeof(Ksa64ViewerDispositionV1) == 72, "disposition v1 ABI drift");
static_assert(sizeof(Ksa64ViewerActionProposalV1) == 144, "action proposal v1 ABI drift");
static_assert(sizeof(Ksa64ViewerActionReceiptV1) == 80, "action receipt v1 ABI drift");
static_assert(sizeof(Ksa64ViewerTimelineEventV1) == 136, "timeline event v1 ABI drift");
static_assert(sizeof(Ksa64ViewerReleaseSampleV1) == 112, "release sample v1 ABI drift");
static_assert(sizeof(Ksa64ViewerPredictionPathHeaderV1) == 88, "prediction header v1 ABI drift");
static_assert(sizeof(Ksa64ViewerPredictionPathPointV1) == 56, "prediction point v1 ABI drift");
static_assert(sizeof(Ksa64ViewerTransportStatusV1) == 96, "transport status v1 ABI drift");
static_assert(sizeof(Ksa64ViewerFinishStatusV1) == 64, "finish status v1 ABI drift");

static_assert(sizeof(Ksa64ViewerAbiInfo) == 132, "Ksa64ViewerAbiInfo ABI drift");
static_assert(sizeof(Ksa64ViewerSpan) == 24, "Ksa64ViewerSpan ABI drift");
static_assert(sizeof(Ksa64ViewerOwnedBuffer) == 32, "Ksa64ViewerOwnedBuffer ABI drift");
static_assert(sizeof(Ksa64ViewerEvent) == 24, "Ksa64ViewerEvent ABI drift");
static_assert(sizeof(Ksa64ViewerSnapshot) == 184, "Ksa64ViewerSnapshot ABI drift");
static_assert(offsetof(Ksa64ViewerSnapshot, command_sequence) == 16, "snapshot offset drift");
static_assert(offsetof(Ksa64ViewerSnapshot, navigation_position_q12) == 60, "snapshot offset drift");
static_assert(offsetof(Ksa64ViewerSnapshot, prediction_impact_position_q12_km) == 172, "snapshot offset drift");
#endif

KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_start_v1(const Ksa64ViewerStartRequestV1* request, Ksa64ViewerHandle** output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_operational_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerOperationalViewV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_procedure_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerProcedureViewV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_disposition_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerDispositionV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_timeline_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerTimelineEventV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_release_sample_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerReleaseSampleV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_prediction_path_header_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerPredictionPathHeaderV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_prediction_path_point_v1(const Ksa64ViewerHandle* handle, uint32_t point_index, Ksa64ViewerPredictionPathPointV1* output);
/* Source-selected Phase 12B presentation paths. The planned KPH10 reference
   populates altitude/downrange/crossrange; Cartesian position remains zero. */
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_trajectory_path_header_v1(const Ksa64ViewerHandle* handle, uint32_t source, Ksa64ViewerPredictionPathHeaderV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_trajectory_path_point_v1(const Ksa64ViewerHandle* handle, uint32_t source, uint32_t point_index, Ksa64ViewerPredictionPathPointV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_action_proposal_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerActionProposalV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_submit_action_proposal_v1(const Ksa64ViewerHandle* handle, uint32_t proposal_identity, uint32_t completed_event_mask);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_commit_action_v1(const Ksa64ViewerHandle* handle, uint32_t proposal_identity);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_cancel_action_v1(const Ksa64ViewerHandle* handle, uint32_t proposal_identity);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_action_receipt_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerActionReceiptV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_transport_status_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerTransportStatusV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_finish_status_v1(const Ksa64ViewerHandle* handle, Ksa64ViewerFinishStatusV1* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_request_shutdown_v1(const Ksa64ViewerHandle* handle);

KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_get_abi_info(Ksa64ViewerAbiInfo* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_catalog(Ksa64ViewerOwnedBuffer* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_start(const Ksa64ViewerSpan* role, Ksa64ViewerHandle** output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_destroy(Ksa64ViewerHandle* handle);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_pause(const Ksa64ViewerHandle* handle);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_resume(const Ksa64ViewerHandle* handle);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_set_pace(const Ksa64ViewerHandle* handle, uint32_t pace);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_step(const Ksa64ViewerHandle* handle);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_advance(const Ksa64ViewerHandle* handle, uint32_t maximum_releases);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_abort(const Ksa64ViewerHandle* handle, uint32_t reason_identity);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_snapshot(const Ksa64ViewerHandle* handle, Ksa64ViewerSnapshot* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_event(const Ksa64ViewerHandle* handle, Ksa64ViewerEvent* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_recommended_load(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_commit_request(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_completed_ksb11(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_library_diagnostic(Ksa64ViewerOwnedBuffer* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_diagnostic(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_submit_stage(const Ksa64ViewerHandle* handle, const Ksa64ViewerSpan* payload, uint32_t completed_event_mask);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_submit_commit(const Ksa64ViewerHandle* handle, const Ksa64ViewerSpan* payload);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_submit_cancel(const Ksa64ViewerHandle* handle, const Ksa64ViewerSpan* payload);
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_free_buffer(Ksa64ViewerOwnedBuffer* buffer);
/* Present only when built with --features panic-probe. */
KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL ksa64_viewer_test_panic_probe(const Ksa64ViewerHandle* handle);

#ifdef __cplusplus
}
#endif
#endif
