#ifndef KSA64_VIEWER_BRIDGE_H
#define KSA64_VIEWER_BRIDGE_H
#include <stdint.h>
#include <stddef.h>
#ifdef _WIN32
#define KSA64_VIEWER_CALL __cdecl
#ifdef __cplusplus
extern "C" {
#endif

#define KSA64_VIEWER_ABI_VERSION 1u
#define KSA64_VIEWER_BUILD_IDENTITY 0x120A0001u
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
static_assert(sizeof(Ksa64ViewerSpan) == 24, "Ksa64ViewerSpan ABI drift");
static_assert(sizeof(Ksa64ViewerOwnedBuffer) == 32, "Ksa64ViewerOwnedBuffer ABI drift");
static_assert(sizeof(Ksa64ViewerEvent) == 24, "Ksa64ViewerEvent ABI drift");
static_assert(sizeof(Ksa64ViewerSnapshot) == 184, "Ksa64ViewerSnapshot ABI drift");
static_assert(offsetof(Ksa64ViewerSnapshot, command_sequence) == 16, "snapshot offset drift");
static_assert(offsetof(Ksa64ViewerSnapshot, navigation_position_q12) == 60, "snapshot offset drift");
static_assert(offsetof(Ksa64ViewerSnapshot, prediction_impact_position_q12_km) == 172, "snapshot offset drift");
#endif
int32_t KSA64_VIEWER_CALL ksa64_viewer_get_abi_info(Ksa64ViewerAbiInfo* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_catalog(Ksa64ViewerOwnedBuffer* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_start(const Ksa64ViewerSpan* role, Ksa64ViewerHandle** output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_destroy(Ksa64ViewerHandle* handle);
int32_t KSA64_VIEWER_CALL ksa64_viewer_pause(const Ksa64ViewerHandle* handle);
int32_t KSA64_VIEWER_CALL ksa64_viewer_resume(const Ksa64ViewerHandle* handle);
int32_t KSA64_VIEWER_CALL ksa64_viewer_set_pace(const Ksa64ViewerHandle* handle, uint32_t pace);
int32_t KSA64_VIEWER_CALL ksa64_viewer_step(const Ksa64ViewerHandle* handle);
int32_t KSA64_VIEWER_CALL ksa64_viewer_advance(const Ksa64ViewerHandle* handle, uint32_t maximum_releases);
int32_t KSA64_VIEWER_CALL ksa64_viewer_abort(const Ksa64ViewerHandle* handle, uint32_t reason_identity);
int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_snapshot(const Ksa64ViewerHandle* handle, Ksa64ViewerSnapshot* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_poll_event(const Ksa64ViewerHandle* handle, Ksa64ViewerEvent* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_recommended_load(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_commit_request(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_completed_ksb11(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_library_diagnostic(Ksa64ViewerOwnedBuffer* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_diagnostic(const Ksa64ViewerHandle* handle, Ksa64ViewerOwnedBuffer* output);
int32_t KSA64_VIEWER_CALL ksa64_viewer_submit_stage(const Ksa64ViewerHandle* handle, const Ksa64ViewerSpan* payload, uint32_t completed_event_mask);
int32_t KSA64_VIEWER_CALL ksa64_viewer_submit_commit(const Ksa64ViewerHandle* handle, const Ksa64ViewerSpan* payload);
int32_t KSA64_VIEWER_CALL ksa64_viewer_submit_cancel(const Ksa64ViewerHandle* handle, const Ksa64ViewerSpan* payload);
int32_t KSA64_VIEWER_CALL ksa64_viewer_free_buffer(Ksa64ViewerOwnedBuffer* buffer);
/* Present only when built with --features panic-probe. */
int32_t KSA64_VIEWER_CALL ksa64_viewer_test_panic_probe(const Ksa64ViewerHandle* handle);

#ifdef __cplusplus
}
#endif
#endif
#endif