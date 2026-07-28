#ifndef KSA64_VIEWER_BRIDGE_GLOBAL_V1_H
#define KSA64_VIEWER_BRIDGE_GLOBAL_V1_H

/* Optional Phase 12C extension. Older bridge libraries legitimately omit the
   ksa64_viewer_global_display_api_v1 symbol; base ABI-v1 loading must continue. */
#include "ksa64_viewer_bridge.h"

#ifdef __cplusplus
extern "C" {
#endif

#define KSA64_GLOBAL_DISPLAY_API_VERSION 1u
#define KSA64_GLOBAL_DISPLAY_API_IMPLEMENTED 0x00000001u
#define KSA64_GLOBAL_DISPLAY_API_ROLE_FILTERED 0x00000002u
#define KSA64_GLOBAL_DISPLAY_AVAILABILITY_ACCEPTED_EXACT 0x00000001u
#define KSA64_GLOBAL_DISPLAY_REPLAY_READ_ONLY 0x00000001u

typedef struct {
    uint32_t api_version, struct_size, role, flags;
    uint32_t reserved[8];
} Ksa64GlobalDisplayReplayStartRequestV1;

typedef struct {
    uint32_t api_version, struct_size, flags, role;
    uint32_t display_identity, available_source_mask, available_frame_mask;
    uint32_t sample_count, transition_count;
    uint32_t oldest_sample_release, newest_sample_release;
    uint32_t reserved[5];
} Ksa64GlobalDisplayAvailabilityV1;

typedef struct {
    uint32_t api_version, struct_size;
    uint32_t start_release, max_count, flags;
    uint32_t reserved[7];
} Ksa64GlobalDisplaySampleRangeRequestV1;

typedef struct {
    uint32_t api_version, struct_size;
    uint32_t source, display_frame, lod, chunk_index;
    uint32_t reserved[6];
} Ksa64GlobalDisplayPathRequestV1;

typedef int32_t (KSA64_VIEWER_CALL *Ksa64GlobalReplayStartFn)(
    const Ksa64GlobalDisplayReplayStartRequestV1*, Ksa64ViewerHandle**);
typedef int32_t (KSA64_VIEWER_CALL *Ksa64GlobalAvailabilityFn)(
    const Ksa64ViewerHandle*, Ksa64GlobalDisplayAvailabilityV1*);
typedef int32_t (KSA64_VIEWER_CALL *Ksa64GlobalPayloadFn)(
    const Ksa64ViewerHandle*, Ksa64ViewerOwnedBuffer*);
typedef int32_t (KSA64_VIEWER_CALL *Ksa64GlobalSampleRangeFn)(
    const Ksa64ViewerHandle*, const Ksa64GlobalDisplaySampleRangeRequestV1*,
    Ksa64ViewerOwnedBuffer*);
typedef int32_t (KSA64_VIEWER_CALL *Ksa64GlobalPathFn)(
    const Ksa64ViewerHandle*, const Ksa64GlobalDisplayPathRequestV1*,
    Ksa64ViewerOwnedBuffer*);

typedef struct {
    uint32_t api_version, struct_size, feature_flags;
    uint32_t replay_start_request_size, availability_size, path_request_size;
    uint32_t sample_range_request_size, owned_buffer_size;
    Ksa64GlobalReplayStartFn start_nominal_replay;
    Ksa64GlobalAvailabilityFn availability;
    Ksa64GlobalPayloadFn definition_payload;
    Ksa64GlobalPayloadFn poll_sample_payload;
    Ksa64GlobalSampleRangeFn sample_range_payload;
    Ksa64GlobalPayloadFn poll_transition_payload;
    Ksa64GlobalPayloadFn replay_index_payload;
    Ksa64GlobalPathFn path_chunk_payload;
    uint64_t reserved[6];
} Ksa64GlobalDisplayApiV1;

KSA64_VIEWER_API int32_t KSA64_VIEWER_CALL
ksa64_viewer_global_display_api_v1(Ksa64GlobalDisplayApiV1* output);

#ifdef __cplusplus
}
#endif
#endif
