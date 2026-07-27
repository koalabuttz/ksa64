#include "../ksa64_viewer_bridge.h"
#include "platform.hpp"
#include "sha256.hpp"

#include <algorithm>
#include <array>
#include <cctype>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>


namespace {

constexpr uint32_t kAbiVersion = KSA64_VIEWER_ABI_VERSION;
constexpr uint32_t kScriptedOperator = 6;
constexpr uint32_t kFastPace = 1;
constexpr uint32_t kCompleted = 5;
constexpr uint32_t kUpdateStageRelease = 6080;
constexpr uint32_t kUpdateCommitRelease = 6240;
constexpr uint32_t kBranchStageRelease = 6560;
constexpr uint32_t kBranchCommitRelease = 6720;
constexpr uint32_t kExpectedFinalRelease = 21591;
constexpr uint32_t kStageOperation = 1;
constexpr uint32_t kCommitOperation = 2;
constexpr uint32_t kStagedState = 1;
constexpr uint32_t kCommittedState = 2;
constexpr uint32_t kFinalFlag = 1;
constexpr uint16_t kIntegrityManifestKind = 19;
constexpr size_t kKsbHeaderLength = 64;
constexpr size_t kKsbTrailerLength = 4;
constexpr size_t kKsbManifestLength = 44;

// Frozen accepted scripted-operator evidence. A second command-line argument
// may override it while investigating a deliberately superseded contract:
//   ksa64_viewer_full_mission_harness.exe bridge.dll EXPECTED_SHA256
constexpr const char* kPhase12bAcceptedKsb11Sha256 =
    "7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4";

[[noreturn]] void fail(const std::string& message) {
    throw std::runtime_error(message);
}

void require(bool condition, const std::string& message) {
    if (!condition) {
        fail(message);
    }
}

template <class T>
T required_symbol(ksa64::native::LibraryHandle library, const char* name) {
    return ksa64::native::required_symbol<T>(library, name);
}

uint16_t read_u16(const uint8_t* input) {
    return static_cast<uint16_t>(input[0]) |
           static_cast<uint16_t>(static_cast<uint16_t>(input[1]) << 8U);
}

uint32_t read_u32(const uint8_t* input) {
    return static_cast<uint32_t>(input[0]) |
           (static_cast<uint32_t>(input[1]) << 8U) |
           (static_cast<uint32_t>(input[2]) << 16U) |
           (static_cast<uint32_t>(input[3]) << 24U);
}

uint32_t crc32(const uint8_t* data, size_t length) {
    uint32_t value = std::numeric_limits<uint32_t>::max();
    for (size_t index = 0; index < length; ++index) {
        value ^= data[index];
        for (unsigned bit = 0; bit < 8; ++bit) {
            value = (value >> 1U) ^ (0xedb88320U & (0U - (value & 1U)));
        }
    }
    return ~value;
}

std::array<uint8_t, 32> sha256(const uint8_t* data, size_t length) {
    return ksa64::crypto::sha256(data, length);
}

std::string hex(const std::array<uint8_t, 32>& bytes) {
    std::ostringstream output;
    output << std::hex << std::setfill('0');
    for (uint8_t byte : bytes) {
        output << std::setw(2) << static_cast<unsigned>(byte);
    }
    return output.str();
}

std::string normalize_hash(std::string value) {
    value.erase(
        std::remove_if(
            value.begin(),
            value.end(),
            [](unsigned char character) { return std::isspace(character) != 0; }),
        value.end());
    std::transform(
        value.begin(),
        value.end(),
        value.begin(),
        [](unsigned char character) {
            return static_cast<char>(std::tolower(character));
        });
    require(
        value.empty() ||
            (value.size() == 64 &&
             std::all_of(
                 value.begin(),
                 value.end(),
                 [](unsigned char character) {
                     return std::isxdigit(character) != 0;
                 })),
        "expected KSB11 SHA-256 must contain exactly 64 hexadecimal digits");
    return value;
}

struct KsbInspection {
    uint32_t definition_identity = 0;
    uint32_t action_identity = 0;
    uint32_t evidence_identity = 0;
    uint32_t segment_count = 0;
};

KsbInspection inspect_complete_ksb11(const uint8_t* input, size_t length) {
    require(input != nullptr, "KSB11 pointer is null");
    require(length >= kKsbHeaderLength + kKsbTrailerLength, "KSB11 is truncated");

    KsbInspection result{};
    size_t offset = 0;
    uint32_t expected_sequence = 0;
    uint32_t expected_prior_crc = 0;
    bool sealed = false;

    while (offset < length) {
        require(
            length - offset >= kKsbHeaderLength + kKsbTrailerLength,
            "KSB11 segment header is truncated");
        const uint8_t* segment = input + offset;
        require(std::memcmp(segment, "KSB1", 4) == 0, "KSB11 magic mismatch");
        require(read_u16(segment + 4) == 11, "KSB11 version mismatch");
        require(
            read_u16(segment + 6) == kKsbHeaderLength,
            "KSB11 header length mismatch");

        const size_t segment_length = read_u32(segment + 8);
        const size_t payload_length = read_u32(segment + 32);
        require(
            segment_length == kKsbHeaderLength + payload_length + kKsbTrailerLength,
            "KSB11 segment length mismatch");
        require(segment_length <= length - offset, "KSB11 segment is truncated");

        const uint32_t definition = read_u32(segment + 12);
        const uint32_t actions = read_u32(segment + 16);
        const uint32_t evidence = read_u32(segment + 20);
        require(
            definition != 0 && actions != 0 && evidence != 0,
            "completed KSB11 identity is incomplete");
        if (expected_sequence == 0) {
            result.definition_identity = definition;
            result.action_identity = actions;
            result.evidence_identity = evidence;
        } else {
            require(
                definition == result.definition_identity &&
                    actions == result.action_identity &&
                    evidence == result.evidence_identity,
                "KSB11 segment identity changed");
        }
        require(
            read_u32(segment + 24) == expected_sequence,
            "KSB11 segment sequence mismatch");
        const uint16_t kind = read_u16(segment + 28);
        require(kind >= 1 && kind <= kIntegrityManifestKind, "KSB11 segment kind invalid");
        const uint16_t flags = read_u16(segment + 30);
        require((flags & ~kFinalFlag) == 0, "KSB11 flags invalid");
        require(
            std::all_of(
                segment + 44,
                segment + kKsbHeaderLength,
                [](uint8_t value) { return value == 0; }),
            "KSB11 reserved bytes are nonzero");

        const uint8_t* payload = segment + kKsbHeaderLength;
        require(
            read_u32(segment + 36) == crc32(payload, payload_length),
            "KSB11 payload CRC mismatch");
        require(
            read_u32(segment + 40) == expected_prior_crc,
            "KSB11 prior-segment CRC mismatch");
        const uint32_t segment_crc =
            read_u32(segment + segment_length - kKsbTrailerLength);
        require(
            segment_crc == crc32(segment, segment_length - kKsbTrailerLength),
            "KSB11 segment CRC mismatch");

        if (kind == kIntegrityManifestKind) {
            require(flags == kFinalFlag, "KSB11 manifest is not final");
            require(
                payload_length == kKsbManifestLength,
                "KSB11 manifest length mismatch");
            require(std::memcmp(payload, "KSM1", 4) == 0, "KSB11 manifest magic mismatch");
            require(
                read_u32(payload + 4) == offset,
                "KSB11 manifest prefix length mismatch");
            require(
                read_u32(payload + 8) == result.segment_count,
                "KSB11 manifest segment count mismatch");
            const auto prefix_hash = sha256(input, offset);
            require(
                std::memcmp(payload + 12, prefix_hash.data(), prefix_hash.size()) == 0,
                "KSB11 manifest SHA-256 mismatch");
            require(offset + segment_length == length, "bytes follow KSB11 final manifest");
            sealed = true;
        } else {
            require(flags == 0, "non-manifest KSB11 segment is marked final");
            require(!sealed, "KSB11 segment follows final manifest");
            expected_prior_crc = segment_crc;
            ++result.segment_count;
            ++expected_sequence;
        }
        offset += segment_length;
    }
    require(sealed && offset == length, "KSB11 is not a complete sealed bundle");
    require(result.segment_count != 0, "KSB11 contains no evidence segments");
    return result;
}

template <class T>
T initialized() {
    T value{};
    value.abi_version = kAbiVersion;
    value.struct_size = sizeof(T);
    return value;
}

Ksa64ViewerOwnedBuffer empty_buffer() {
    return initialized<Ksa64ViewerOwnedBuffer>();
}

struct Api {
    int32_t (*get_abi_info)(Ksa64ViewerAbiInfo*);
    int32_t (*start_v1)(const Ksa64ViewerStartRequestV1*, Ksa64ViewerHandle**);
    int32_t (*destroy)(Ksa64ViewerHandle*);
    int32_t (*advance)(const Ksa64ViewerHandle*, uint32_t);
    int32_t (*poll_snapshot)(const Ksa64ViewerHandle*, Ksa64ViewerSnapshot*);
    int32_t (*poll_operational)(
        const Ksa64ViewerHandle*, Ksa64ViewerOperationalViewV1*);
    int32_t (*procedure)(const Ksa64ViewerHandle*, Ksa64ViewerProcedureViewV1*);
    int32_t (*disposition)(const Ksa64ViewerHandle*, Ksa64ViewerDispositionV1*);
    int32_t (*poll_timeline)(
        const Ksa64ViewerHandle*, Ksa64ViewerTimelineEventV1*);
    int32_t (*poll_release_sample)(
        const Ksa64ViewerHandle*, Ksa64ViewerReleaseSampleV1*);
    int32_t (*prediction_header)(
        const Ksa64ViewerHandle*, Ksa64ViewerPredictionPathHeaderV1*);
    int32_t (*prediction_point)(
        const Ksa64ViewerHandle*, uint32_t, Ksa64ViewerPredictionPathPointV1*);
    int32_t (*action_proposal)(
        const Ksa64ViewerHandle*, Ksa64ViewerActionProposalV1*);
    int32_t (*submit_action)(
        const Ksa64ViewerHandle*, uint32_t, uint32_t);
    int32_t (*commit_action)(const Ksa64ViewerHandle*, uint32_t);
    int32_t (*poll_action_receipt)(
        const Ksa64ViewerHandle*, Ksa64ViewerActionReceiptV1*);
    int32_t (*transport_status)(
        const Ksa64ViewerHandle*, Ksa64ViewerTransportStatusV1*);
    int32_t (*finish_status)(
        const Ksa64ViewerHandle*, Ksa64ViewerFinishStatusV1*);
    int32_t (*completed_ksb11)(
        const Ksa64ViewerHandle*, Ksa64ViewerOwnedBuffer*);
    int32_t (*free_buffer)(Ksa64ViewerOwnedBuffer*);
};

Api load_api(ksa64::native::LibraryHandle library) {
    return {
        required_symbol<decltype(Api::get_abi_info)>(
            library, "ksa64_viewer_get_abi_info"),
        required_symbol<decltype(Api::start_v1)>(
            library, "ksa64_viewer_start_v1"),
        required_symbol<decltype(Api::destroy)>(
            library, "ksa64_viewer_destroy"),
        required_symbol<decltype(Api::advance)>(
            library, "ksa64_viewer_advance"),
        required_symbol<decltype(Api::poll_snapshot)>(
            library, "ksa64_viewer_poll_snapshot"),
        required_symbol<decltype(Api::poll_operational)>(
            library, "ksa64_viewer_poll_operational_v1"),
        required_symbol<decltype(Api::procedure)>(
            library, "ksa64_viewer_procedure_v1"),
        required_symbol<decltype(Api::disposition)>(
            library, "ksa64_viewer_disposition_v1"),
        required_symbol<decltype(Api::poll_timeline)>(
            library, "ksa64_viewer_poll_timeline_v1"),
        required_symbol<decltype(Api::poll_release_sample)>(
            library, "ksa64_viewer_poll_release_sample_v1"),
        required_symbol<decltype(Api::prediction_header)>(
            library, "ksa64_viewer_prediction_path_header_v1"),
        required_symbol<decltype(Api::prediction_point)>(
            library, "ksa64_viewer_prediction_path_point_v1"),
        required_symbol<decltype(Api::action_proposal)>(
            library, "ksa64_viewer_action_proposal_v1"),
        required_symbol<decltype(Api::submit_action)>(
            library, "ksa64_viewer_submit_action_proposal_v1"),
        required_symbol<decltype(Api::commit_action)>(
            library, "ksa64_viewer_commit_action_v1"),
        required_symbol<decltype(Api::poll_action_receipt)>(
            library, "ksa64_viewer_poll_action_receipt_v1"),
        required_symbol<decltype(Api::transport_status)>(
            library, "ksa64_viewer_transport_status_v1"),
        required_symbol<decltype(Api::finish_status)>(
            library, "ksa64_viewer_finish_status_v1"),
        required_symbol<decltype(Api::completed_ksb11)>(
            library, "ksa64_viewer_completed_ksb11"),
        required_symbol<decltype(Api::free_buffer)>(
            library, "ksa64_viewer_free_buffer"),
    };
}

Ksa64ViewerSnapshot wait_for_command(
    const Api& api,
    Ksa64ViewerHandle* handle,
    uint64_t previous_sequence) {
    const auto deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(30);
    while (std::chrono::steady_clock::now() < deadline) {
        auto snapshot = initialized<Ksa64ViewerSnapshot>();
        const int32_t code = api.poll_snapshot(handle, &snapshot);
        if (code == KSA64_VIEWER_OK &&
            snapshot.command_sequence > previous_sequence) {
            return snapshot;
        }
        require(
            code == KSA64_VIEWER_OK || code == KSA64_VIEWER_NO_DATA ||
                code == KSA64_VIEWER_UNCHANGED,
            "snapshot poll failed while waiting for command completion");
        std::this_thread::yield();
    }
    fail("timed out waiting for full-mission worker command");
}

Ksa64ViewerSnapshot initial_snapshot(const Api& api, Ksa64ViewerHandle* handle) {
    const auto deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(30);
    while (std::chrono::steady_clock::now() < deadline) {
        auto snapshot = initialized<Ksa64ViewerSnapshot>();
        const int32_t code = api.poll_snapshot(handle, &snapshot);
        if (code == KSA64_VIEWER_OK) {
            return snapshot;
        }
        require(
            code == KSA64_VIEWER_NO_DATA || code == KSA64_VIEWER_UNCHANGED,
            "initial snapshot poll failed");
        std::this_thread::yield();
    }
    fail("timed out waiting for initial full-mission snapshot");
}

struct SurfaceEvidence {
    uint32_t operational_views = 0;
    uint32_t procedure_views = 0;
    uint32_t timeline_events = 0;
    uint32_t release_samples = 0;
    uint32_t prediction_headers = 0;
    uint32_t prediction_points = 0;
    uint32_t action_receipts = 0;
    uint32_t last_timeline_sequence = 0;
    uint32_t last_sample_release = 0;
};

void inspect_surfaces(
    const Api& api,
    Ksa64ViewerHandle* handle,
    SurfaceEvidence& evidence,
    bool expect_operational_update) {
    auto operational = initialized<Ksa64ViewerOperationalViewV1>();
    const int32_t operational_code = api.poll_operational(handle, &operational);
    require(
        operational_code == KSA64_VIEWER_OK ||
            operational_code == KSA64_VIEWER_UNCHANGED,
        "operational view poll failed");
    if (operational_code == KSA64_VIEWER_OK) {
        ++evidence.operational_views;
        require(
            operational.scenario_identity == KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS,
            "operational view scenario mismatch");
        require(operational.role == kScriptedOperator, "operational view role mismatch");
    } else {
        require(!expect_operational_update, "fresh operational view was not published");
    }

    auto procedure = initialized<Ksa64ViewerProcedureViewV1>();
    const int32_t procedure_code = api.procedure(handle, &procedure);
    require(
        procedure_code == KSA64_VIEWER_OK ||
            procedure_code == KSA64_VIEWER_NO_DATA,
        "procedure view poll failed");
    if (procedure_code == KSA64_VIEWER_OK) {
        ++evidence.procedure_views;
        require(procedure.validity_mask != 0, "procedure view is not valid");
        require(
            procedure.active_step < procedure.step_count,
            "procedure active step is out of range");
        require(
            procedure.predicate_count <= 8,
            "procedure predicate count exceeds the ABI bound");
    }

    for (;;) {
        auto event = initialized<Ksa64ViewerTimelineEventV1>();
        const int32_t code = api.poll_timeline(handle, &event);
        if (code == KSA64_VIEWER_NO_DATA) {
            break;
        }
        require(code == KSA64_VIEWER_OK, "timeline poll failed");
        require(
            event.sequence > evidence.last_timeline_sequence,
            "timeline sequence is not strictly increasing");
        evidence.last_timeline_sequence = event.sequence;
        ++evidence.timeline_events;
    }

    for (;;) {
        auto sample = initialized<Ksa64ViewerReleaseSampleV1>();
        const int32_t code = api.poll_release_sample(handle, &sample);
        if (code == KSA64_VIEWER_NO_DATA) {
            break;
        }
        require(code == KSA64_VIEWER_OK, "release-sample poll failed");
        require((sample.flags & 1U) == 0, "non-SIM operator sample exposed SIM truth");
        require(
            sample.release_epoch >= evidence.last_sample_release,
            "release samples are out of order");
        evidence.last_sample_release = sample.release_epoch;
        ++evidence.release_samples;
    }

    auto prediction = initialized<Ksa64ViewerPredictionPathHeaderV1>();
    const int32_t prediction_code = api.prediction_header(handle, &prediction);
    require(
        prediction_code == KSA64_VIEWER_OK ||
            prediction_code == KSA64_VIEWER_NO_DATA,
        "prediction header poll failed");
    if (prediction_code == KSA64_VIEWER_OK) {
        ++evidence.prediction_headers;
        require(prediction.validity_mask != 0, "prediction header is not valid");
        require(prediction.point_count != 0, "prediction path is empty");
        const uint32_t indices[2] = {0, prediction.point_count - 1};
        for (uint32_t index : indices) {
            auto point = initialized<Ksa64ViewerPredictionPathPointV1>();
            require(
                api.prediction_point(handle, index, &point) == KSA64_VIEWER_OK,
                "prediction path point poll failed");
            require(
                point.path_identity == prediction.path_identity &&
                    point.point_index == index,
                "prediction path point identity mismatch");
            ++evidence.prediction_points;
        }
    }

    auto status = initialized<Ksa64ViewerTransportStatusV1>();
    require(
        api.transport_status(handle, &status) == KSA64_VIEWER_OK,
        "transport status poll failed");
    require(status.worker_state == 1 || status.worker_state == 2, "worker failed");
    require(
        status.event_overflow == 0 && status.timeline_overflow == 0 &&
            status.sample_overflow == 0,
        "a bridge presentation queue overflowed");
}

void advance_to(
    const Api& api,
    Ksa64ViewerHandle* handle,
    Ksa64ViewerSnapshot& snapshot,
    uint32_t target,
    SurfaceEvidence& evidence) {
    require(snapshot.release_epoch <= target, "advance target is in the past");
    while (snapshot.release_epoch < target) {
        const uint32_t count =
            std::min<uint32_t>(
                KSA64_VIEWER_MAX_ADVANCE_RELEASES,
                target - snapshot.release_epoch);
        const uint64_t sequence = snapshot.command_sequence;
        require(
            api.advance(handle, count) == KSA64_VIEWER_QUEUED,
            "bounded advancement was not queued");
        snapshot = wait_for_command(api, handle, sequence);
        require(snapshot.command_result == KSA64_VIEWER_OK, "bounded advancement failed");
        inspect_surfaces(api, handle, evidence, true);
    }
    require(snapshot.release_epoch == target, "bounded advancement missed target release");
}

uint32_t require_proposal(
    const Api& api,
    Ksa64ViewerHandle* handle,
    uint32_t expected_release) {
    auto proposal = initialized<Ksa64ViewerActionProposalV1>();
    require(
        api.action_proposal(handle, &proposal) == KSA64_VIEWER_OK,
        "typed action proposal is unavailable at accepted stage window");
    require(
        proposal.validity_mask != 0 &&
            proposal.proposal_identity != 0 &&
            proposal.proposal_identity == proposal.load_identity,
        "typed action proposal is malformed");
    require(
        proposal.permitted_operations == kStageOperation,
        "typed action proposal does not require review/stage");
    require(
        proposal.earliest_commit_epoch >= expected_release + 2,
        "typed action proposal violates the two-release lead rule");
    return proposal.proposal_identity;
}

void wait_for_receipt(
    const Api& api,
    Ksa64ViewerHandle* handle,
    uint32_t operation,
    uint32_t state,
    uint32_t proposal_identity,
    SurfaceEvidence& evidence) {
    const auto deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(30);
    while (std::chrono::steady_clock::now() < deadline) {
        auto receipt = initialized<Ksa64ViewerActionReceiptV1>();
        const int32_t code = api.poll_action_receipt(handle, &receipt);
        if (code == KSA64_VIEWER_OK) {
            require(receipt.accepted == 1, "typed action receipt rejected");
            require(receipt.operation == operation, "typed action receipt operation mismatch");
            require(receipt.state == state, "typed action receipt state mismatch");
            require(
                receipt.proposal_identity == proposal_identity &&
                    receipt.load_identity == proposal_identity,
                "typed action receipt identity mismatch");
            ++evidence.action_receipts;
            return;
        }
        require(
            code == KSA64_VIEWER_NO_DATA || code == KSA64_VIEWER_UNCHANGED,
            "typed action receipt poll failed");
        std::this_thread::yield();
    }
    fail("timed out waiting for typed action receipt");
}

void stage_action(
    const Api& api,
    Ksa64ViewerHandle* handle,
    Ksa64ViewerSnapshot& snapshot,
    uint32_t proposal_identity,
    SurfaceEvidence& evidence) {
    const uint64_t sequence = snapshot.command_sequence;
    require(
        api.submit_action(handle, proposal_identity, 0) == KSA64_VIEWER_QUEUED,
        "typed Review/Stage action was not queued");
    snapshot = wait_for_command(api, handle, sequence);
    require(snapshot.command_result == KSA64_VIEWER_OK, "typed stage action failed");
    wait_for_receipt(
        api,
        handle,
        kStageOperation,
        kStagedState,
        proposal_identity,
        evidence);
    inspect_surfaces(api, handle, evidence, true);
}

void commit_action(
    const Api& api,
    Ksa64ViewerHandle* handle,
    Ksa64ViewerSnapshot& snapshot,
    uint32_t proposal_identity,
    SurfaceEvidence& evidence) {
    const uint64_t sequence = snapshot.command_sequence;
    require(
        api.commit_action(handle, proposal_identity) == KSA64_VIEWER_QUEUED,
        "typed Commit action was not queued");
    snapshot = wait_for_command(api, handle, sequence);
    require(snapshot.command_result == KSA64_VIEWER_OK, "typed commit action failed");
    wait_for_receipt(
        api,
        handle,
        kCommitOperation,
        kCommittedState,
        proposal_identity,
        evidence);
    inspect_surfaces(api, handle, evidence, true);
}

int run(const char* library_path, const std::string& expected_hash) {
    auto library = ksa64::native::open_library(library_path);
    if (library == nullptr) {
        fail("dynamic library load failed: " + ksa64::native::loader_error());
    }

    try {
        const Api api = load_api(library);
        auto info = initialized<Ksa64ViewerAbiInfo>();
        require(api.get_abi_info(&info) == KSA64_VIEWER_OK, "ABI info request failed");
        require(
            info.abi_version == kAbiVersion &&
                info.build_identity == KSA64_VIEWER_BUILD_IDENTITY,
            "bridge ABI/build identity mismatch");
        require(
            (info.feature_flags & KSA64_VIEWER_FEATURE_OPERATIONS_V1) != 0 &&
                (info.feature_flags & KSA64_VIEWER_FEATURE_TYPED_ACTIONS_V1) != 0 &&
                (info.feature_flags & KSA64_VIEWER_FEATURE_ASYNC_STATUS_V1) != 0,
            "bridge does not advertise the complete Phase 12B feature set");

        auto request = initialized<Ksa64ViewerStartRequestV1>();
        request.scenario_identity = KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS;
        request.role = kScriptedOperator;
        request.initial_pace = kFastPace;
        Ksa64ViewerHandle* handle = nullptr;
        require(
            api.start_v1(&request, &handle) == KSA64_VIEWER_OK && handle != nullptr,
            "full GNSS-loss session failed to start");

        try {
            auto snapshot = initial_snapshot(api, handle);
            require(snapshot.release_epoch == 0, "full mission did not start at release zero");
            SurfaceEvidence evidence{};
            inspect_surfaces(api, handle, evidence, true);

            advance_to(
                api, handle, snapshot, kUpdateStageRelease, evidence);
            const uint32_t update_identity =
                require_proposal(api, handle, kUpdateStageRelease);
            stage_action(api, handle, snapshot, update_identity, evidence);

            advance_to(
                api, handle, snapshot, kUpdateCommitRelease, evidence);
            commit_action(api, handle, snapshot, update_identity, evidence);

            advance_to(
                api, handle, snapshot, kBranchStageRelease, evidence);
            const uint32_t branch_identity =
                require_proposal(api, handle, kBranchStageRelease);
            require(
                branch_identity != update_identity,
                "the two accepted actions share an identity");
            stage_action(api, handle, snapshot, branch_identity, evidence);

            advance_to(
                api, handle, snapshot, kBranchCommitRelease, evidence);
            commit_action(api, handle, snapshot, branch_identity, evidence);

            while (snapshot.lifecycle != kCompleted) {
                const uint64_t sequence = snapshot.command_sequence;
                require(
                    api.advance(handle, KSA64_VIEWER_MAX_ADVANCE_RELEASES) ==
                        KSA64_VIEWER_QUEUED,
                    "mission completion advancement was not queued");
                snapshot = wait_for_command(api, handle, sequence);
                require(
                    snapshot.command_result == KSA64_VIEWER_OK,
                    "mission completion advancement failed");
                inspect_surfaces(api, handle, evidence, true);
            }
            require(
                snapshot.release_epoch == kExpectedFinalRelease,
                "full mission completed on an unexpected release: " +
                    std::to_string(snapshot.release_epoch));
            require(snapshot.action_count == 4, "accepted action transcript is incomplete");

            auto finish = initialized<Ksa64ViewerFinishStatusV1>();
            const auto finish_deadline =
                std::chrono::steady_clock::now() + std::chrono::seconds(30);
            for (;;) {
                finish = initialized<Ksa64ViewerFinishStatusV1>();
                require(
                    api.finish_status(handle, &finish) == KSA64_VIEWER_OK,
                    "finish status is unavailable");
                if (finish.finalization_state == 2) {
                    break;
                }
                require(
                    std::chrono::steady_clock::now() < finish_deadline,
                    "timed out waiting for KSB11 finalization");
                std::this_thread::yield();
            }
            require(
                finish.lifecycle == kCompleted &&
                    finish.finalization_state == 2 &&
                    finish.evidence_identity != 0 &&
                    finish.evidence_length != 0,
                "full mission was not finalized");

            auto disposition = initialized<Ksa64ViewerDispositionV1>();
            require(
                api.disposition(handle, &disposition) == KSA64_VIEWER_OK,
                "verified completed disposition is unavailable");
            require(
                disposition.validity_mask != 0 &&
                    disposition.overall != 0 &&
                    disposition.objective != 0 &&
                    disposition.vehicle != 0 &&
                    disposition.procedure != 0 &&
                    disposition.operator_disposition != 0 &&
                    disposition.avionics != 0 &&
                    disposition.evidence != 0,
                "verified completed disposition axes are incomplete");

            auto bundle = empty_buffer();
            require(
                api.completed_ksb11(handle, &bundle) == KSA64_VIEWER_OK,
                "completed KSB11 is unavailable");
            require(
                bundle.data != nullptr && bundle.length == finish.evidence_length,
                "completed KSB11 ownership/length mismatch");
            require(
                crc32(bundle.data, static_cast<size_t>(bundle.length)) ==
                    finish.evidence_crc32,
                "finish-status KSB11 CRC mismatch");
            const KsbInspection inspection =
                inspect_complete_ksb11(
                    bundle.data, static_cast<size_t>(bundle.length));
            require(
                inspection.evidence_identity == finish.evidence_identity,
                "finish status and KSB11 evidence identities differ");

            const std::string observed_hash =
                hex(sha256(bundle.data, static_cast<size_t>(bundle.length)));
            if (!expected_hash.empty()) {
                require(
                    observed_hash == expected_hash,
                    "completed KSB11 SHA-256 mismatch: observed " + observed_hash +
                        ", expected " + expected_hash);
            }
            require(
                api.free_buffer(&bundle) == KSA64_VIEWER_OK,
                "completed KSB11 buffer could not be released");

            require(
                evidence.operational_views > 100 &&
                    evidence.procedure_views > 0 &&
                    evidence.timeline_events > 0 &&
                    evidence.release_samples >= kExpectedFinalRelease / 8 &&
                    evidence.prediction_headers > 0 &&
                    evidence.prediction_points > 0 &&
                    evidence.action_receipts == 4,
                "one or more additive presentation surfaces were not exercised");

            require(api.destroy(handle) == KSA64_VIEWER_OK, "bridge destroy failed");
            handle = nullptr;
            ksa64::native::close_library(library);
            library = nullptr;

            std::cout
                << "KSA64 Phase 12B full-mission ABI harness passed\n"
                << "release=" << snapshot.release_epoch
                << " actions=" << snapshot.action_count
                << " evidence_bytes=" << finish.evidence_length
                << " ksb11_sha256=" << observed_hash << "\n"
                << "surfaces: operational=" << evidence.operational_views
                << " procedure=" << evidence.procedure_views
                << " timeline=" << evidence.timeline_events
                << " samples=" << evidence.release_samples
                << " predictions=" << evidence.prediction_headers
                << " receipts=" << evidence.action_receipts << "\n";
            if (expected_hash.empty()) {
                std::cout
                    << "NOTE: Phase 12B accepted KSB11 SHA-256 is not frozen in "
                       "the harness yet; pass it as argument 2 or replace "
                       "kPhase12bAcceptedKsb11Sha256.\n";
            }
            return 0;
        } catch (...) {
            if (handle != nullptr) {
                api.destroy(handle);
            }
            throw;
        }
    } catch (...) {
        ksa64::native::close_library(library);
        throw;
    }
}

}  // namespace

int main(int argc, char** argv) {
    try {
        const char* library_path =
            argc > 1 ? argv[1] : ksa64::native::kDefaultBridgePath;
        const std::string expected_hash = normalize_hash(
            argc > 2 ? argv[2] : kPhase12bAcceptedKsb11Sha256);
        return run(library_path, expected_hash);
    } catch (const std::exception& error) {
        std::cerr << "KSA64 Phase 12B full-mission ABI harness failed: "
                  << error.what() << "\n";
        return 1;
    }
}
