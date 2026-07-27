#pragma once

#include "CoreMinimal.h"
#include "ksa64_viewer_bridge.h"

namespace Ksa64BridgeTypedValidation
{
constexpr uint64 ValidMissionTime = 1ull << 0;
constexpr uint64 ValidNavigation = 1ull << 1;
constexpr uint64 ValidGroundEstimate = 1ull << 2;
constexpr uint64 ValidPrediction = 1ull << 3;
constexpr uint64 ValidProcedure = 1ull << 4;
constexpr uint64 ValidAction = 1ull << 5;
constexpr uint64 ValidDisposition = 1ull << 6;
constexpr uint64 ValidEvidence = 1ull << 7;
constexpr uint64 ValidGnss = 1ull << 8;
constexpr uint64 OperationalValidityMask =
    ValidMissionTime
    | ValidNavigation
    | ValidGroundEstimate
    | ValidPrediction
    | ValidProcedure
    | ValidAction
    | ValidDisposition
    | ValidEvidence
    | ValidGnss;
constexpr uint64 ReleaseSampleValidityMask =
    ValidMissionTime | ValidNavigation | ValidGroundEstimate | ValidPrediction;
constexpr uint32 GuidedOperatorRole = 2;
constexpr uint32 LegacyExecutionAdapterIdentity = 0x120b1001u;
constexpr uint32 FullExecutionAdapterIdentity = 0x120b1002u;
constexpr uint32 MaximumPredictionPoints = 4096;
constexpr uint32 CommandCapacity = 32;
constexpr uint32 EventCapacity = 256;
constexpr uint32 TimelineCapacity = 256;
constexpr uint32 SampleCapacity = 256;

template <typename T>
bool HasExpectedHeader(const T& Value)
{
    return Value.abi_version == KSA64_VIEWER_ABI_VERSION
        && Value.struct_size == static_cast<uint32>(sizeof(T));
}

template <typename T, SIZE_T Count>
bool AllZero(const T (&Values)[Count])
{
    for (SIZE_T Index = 0; Index < Count; ++Index)
    {
        if (Values[Index] != 0)
        {
            return false;
        }
    }
    return true;
}

inline bool IsBoundedText(
    const uint8* Text,
    uint32 Length,
    uint32 Capacity,
    bool bRequireNonEmpty)
{
    if (Text == nullptr
        || Length > Capacity
        || (bRequireNonEmpty && Length == 0))
    {
        return false;
    }
    for (uint32 Index = Length; Index < Capacity; ++Index)
    {
        if (Text[Index] != 0)
        {
            return false;
        }
    }
    return true;
}

inline bool IsFrame(uint32 Frame)
{
    return Frame >= 1 && Frame <= 3;
}

inline bool IsLifecycle(uint32 Lifecycle)
{
    return Lifecycle >= 1 && Lifecycle <= 6;
}

inline bool IsPace(uint32 Pace)
{
    return Pace >= 1 && Pace <= 4;
}

inline uint32 ExpectedAdapterForScenario(uint32 ScenarioIdentity)
{
    switch (ScenarioIdentity)
    {
    case KSA64_VIEWER_SCENARIO_LEGACY_GNSS_FIXTURE:
        return LegacyExecutionAdapterIdentity;
    case KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS:
        return FullExecutionAdapterIdentity;
    default:
        return 0;
    }
}

inline bool Operational(
    const Ksa64ViewerOperationalViewV1& Value,
    uint32 ExpectedScenarioIdentity,
    uint32 ExpectedAdapterIdentity)
{
    if (!HasExpectedHeader(Value)
        || ExpectedScenarioIdentity == 0
        || ExpectedAdapterIdentity == 0
        || Value.scenario_identity != ExpectedScenarioIdentity
        || Value.execution_adapter_identity != ExpectedAdapterIdentity
        || Value.role != GuidedOperatorRole
        || !IsLifecycle(Value.lifecycle)
        || !IsPace(Value.pace)
        || Value.release_period_micros != 31'250
        || Value.frame > 3
        || Value.safe > 1
        || Value.presentation_flags != 0
        || (Value.validity_mask & ~OperationalValidityMask) != 0
        || !AllZero(Value.reserved))
    {
        return false;
    }
    if ((Value.validity_mask & ValidNavigation) != 0 && !IsFrame(Value.frame))
    {
        return false;
    }
    if ((Value.validity_mask & ValidGroundEstimate) == 0
        && (!AllZero(Value.ground_position_q12) || !AllZero(Value.ground_velocity_q24)))
    {
        return false;
    }
    if ((Value.validity_mask & ValidPrediction) != 0)
    {
        if (Value.prediction_identity == 0 || Value.prediction_checksum == 0)
        {
            return false;
        }
    }
    else if (Value.prediction_identity != 0
        || Value.prediction_checksum != 0
        || Value.prediction_apogee_q12_km != 0
        || Value.prediction_time_to_apogee_q16 != 0
        || Value.prediction_time_to_impact_q16 != 0)
    {
        return false;
    }
    if ((Value.validity_mask & ValidAction) == 0 && Value.staged_load_identity != 0)
    {
        return false;
    }
    if ((Value.validity_mask & ValidGnss) != 0)
    {
        if (Value.gnss_state < 1 || Value.gnss_state > 3)
        {
            return false;
        }
    }
    else if (Value.gnss_state != 0)
    {
        return false;
    }
    if ((Value.validity_mask & ValidDisposition) != 0
        && Value.lifecycle != 5
        && Value.lifecycle != 6)
    {
        return false;
    }
    if ((Value.validity_mask & ValidEvidence) != 0
        && (Value.validity_mask & ValidDisposition) == 0)
    {
        return false;
    }
    return true;
}

inline bool Procedure(const Ksa64ViewerProcedureViewV1& Value)
{
    if (!HasExpectedHeader(Value)
        || (Value.validity_mask & ~ValidProcedure) != 0
        || Value.predicate_count > 8
        || Value.title_length > 64
        || Value.instruction_length > 192)
    {
        return false;
    }
    if (Value.validity_mask == 0)
    {
        return Value.procedure_identity == 0
            && Value.state == 0
            && Value.active_step == 0
            && Value.step_count == 0
            && Value.predicate_count == 0
            && Value.title_length == 0
            && Value.instruction_length == 0
            && AllZero(Value.predicate_identities)
            && AllZero(Value.predicate_states)
            && IsBoundedText(Value.title, 0, 64, false)
            && IsBoundedText(Value.instruction, 0, 192, false);
    }
    if (Value.procedure_identity == 0
        || Value.state > 6
        || Value.step_count == 0
        || Value.active_step >= Value.step_count
        || !IsBoundedText(Value.title, Value.title_length, 64, true)
        || !IsBoundedText(Value.instruction, Value.instruction_length, 192, true))
    {
        return false;
    }
    for (uint32 Index = 0; Index < 8; ++Index)
    {
        if (Index < Value.predicate_count)
        {
            if (Value.predicate_identities[Index] == 0 || Value.predicate_states[Index] > 2)
            {
                return false;
            }
        }
        else if (Value.predicate_identities[Index] != 0 || Value.predicate_states[Index] != 0)
        {
            return false;
        }
    }
    return true;
}

inline bool Disposition(const Ksa64ViewerDispositionV1& Value)
{
    if (!HasExpectedHeader(Value)
        || (Value.validity_mask & ~(ValidDisposition | ValidEvidence)) != 0
        || !AllZero(Value.reserved))
    {
        return false;
    }
    if (Value.validity_mask == 0)
    {
        return Value.overall == 0
            && Value.objective == 0
            && Value.vehicle == 0
            && Value.procedure == 0
            && Value.operator_disposition == 0
            && Value.avionics == 0
            && Value.evidence == 0
            && Value.reason_identity == 0;
    }
    if ((Value.validity_mask & ValidDisposition) == 0
        || Value.overall < 1
        || Value.overall > 5
        || Value.objective < 1
        || Value.objective > 5
        || Value.vehicle < 1
        || Value.vehicle > 6
        || Value.procedure < 1
        || Value.procedure > 6
        || Value.operator_disposition < 1
        || Value.operator_disposition > 5
        || Value.avionics < 1
        || Value.avionics > 4
        || Value.evidence < 1
        || Value.evidence > 5)
    {
        return false;
    }
    return ((Value.validity_mask & ValidEvidence) != 0) == (Value.evidence == 1);
}

inline bool Timeline(const Ksa64ViewerTimelineEventV1& Value)
{
    return HasExpectedHeader(Value)
        && Value.source >= 1
        && Value.source <= 6
        && Value.severity >= 1
        && Value.severity <= 3
        && Value.event_identity != 0
        && Value.flags == 0
        && IsBoundedText(Value.label, Value.label_length, 96, true);
}

inline bool ReleaseSample(const Ksa64ViewerReleaseSampleV1& Value)
{
    if (!HasExpectedHeader(Value)
        || (Value.validity_mask & ~ReleaseSampleValidityMask) != 0
        || (Value.validity_mask & ValidMissionTime) == 0
        || Value.frame > 3
        // Bit zero identifies SIM Director truth and may never cross this
        // Guided Operator boundary. Bit one is the public GNSS-loss marker.
        || (Value.flags & ~2u) != 0)
    {
        return false;
    }
    if ((Value.validity_mask & ValidNavigation) != 0)
    {
        if (!IsFrame(Value.frame))
        {
            return false;
        }
    }
    else if (!AllZero(Value.onboard_position_q12)
        || !AllZero(Value.onboard_velocity_q24))
    {
        return false;
    }
    if ((Value.validity_mask & ValidGroundEstimate) == 0
        && (!AllZero(Value.ground_position_q12)
            || !AllZero(Value.ground_velocity_q24)))
    {
        return false;
    }
    if ((Value.validity_mask & ValidPrediction) == 0
        && (!AllZero(Value.predicted_impact_q12)
            || Value.predicted_apogee_q12_km != 0))
    {
        return false;
    }
    return true;
}

inline bool PredictionHeader(const Ksa64ViewerPredictionPathHeaderV1& Value)
{
    return HasExpectedHeader(Value)
        && Value.validity_mask == ValidPrediction
        && Value.path_identity != 0
        && Value.product >= 1
        // Product four is the SIM-truth counterfactual and is not available to
        // a Guided Operator.
        && Value.product <= 3
        && Value.model_identity != 0
        && Value.source_estimate_identity != 0
        && Value.source_epoch <= Value.generation_epoch
        && IsFrame(Value.frame)
        && Value.terminal_reason >= 1
        && Value.terminal_reason <= 5
        && Value.point_count >= 1
        && Value.point_count <= MaximumPredictionPoints
        && Value.cadence_releases >= 1
        && Value.path_checksum != 0
        && AllZero(Value.reserved);
}

inline bool IsTrajectorySource(uint32 Source)
{
    return Source >= KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE
        && Source <= KSA64_VIEWER_TRAJECTORY_GROUND_ESTIMATE;
}

inline uint32 ExpectedTrajectoryProduct(uint32 Source)
{
    switch (Source)
    {
    case KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE:
        return KSA64_VIEWER_TRAJECTORY_PRODUCT_PLANNED_REFERENCE;
    case KSA64_VIEWER_TRAJECTORY_ONBOARD_ESTIMATE:
        return 2;
    case KSA64_VIEWER_TRAJECTORY_GROUND_ESTIMATE:
        return 3;
    default:
        return 0;
    }
}

inline bool TrajectoryHeader(
    const Ksa64ViewerPredictionPathHeaderV1& Value,
    uint32 Source)
{
    const uint32 ExpectedProduct = ExpectedTrajectoryProduct(Source);
    return ExpectedProduct != 0
        && HasExpectedHeader(Value)
        && Value.validity_mask == ValidPrediction
        && Value.path_identity != 0
        && Value.product == ExpectedProduct
        && Value.model_identity != 0
        && Value.source_estimate_identity != 0
        && Value.source_estimate_checksum != 0
        && Value.source_epoch <= Value.generation_epoch
        && IsFrame(Value.frame)
        && Value.terminal_reason >= 1
        && Value.terminal_reason <= 5
        && Value.point_count >= 1
        && Value.point_count <= MaximumPredictionPoints
        && Value.cadence_releases >= 1
        && Value.path_checksum != 0
        && AllZero(Value.reserved);
}

inline bool PredictionPoint(
    const Ksa64ViewerPredictionPathPointV1& Value,
    uint32 RequestedIndex,
    uint32 ExpectedPathIdentity,
    uint32 PointCount)
{
    return HasExpectedHeader(Value)
        && ExpectedPathIdentity != 0
        && PointCount >= 1
        && PointCount <= MaximumPredictionPoints
        && RequestedIndex < PointCount
        && Value.path_identity == ExpectedPathIdentity
        && Value.point_index == RequestedIndex
        && IsFrame(Value.frame)
        && (Value.flags & ~3u) == 0
        && Value.reserved0 == 0;
}

inline bool ActionProposal(const Ksa64ViewerActionProposalV1& Value)
{
    return HasExpectedHeader(Value)
        && Value.validity_mask == ValidAction
        && Value.proposal_identity != 0
        && Value.load_identity == Value.proposal_identity
        && Value.load_type >= 1
        && Value.load_type <= 5
        && Value.earliest_commit_epoch <= Value.activation_epoch
        && Value.activation_epoch <= Value.expires_epoch
        && Value.permitted_operations == 1
        && IsBoundedText(Value.label, Value.label_length, 80, true);
}

inline bool ActionReceipt(const Ksa64ViewerActionReceiptV1& Value)
{
    if (!HasExpectedHeader(Value)
        || Value.validity_mask != ValidAction
        || Value.publication_sequence == 0
        || Value.proposal_identity == 0
        || Value.load_identity != Value.proposal_identity
        || Value.control_identity == 0
        || Value.state > 6
        || Value.reason > 13
        || Value.accepted > 1
        || Value.operation < 1
        || Value.operation > 3
        || !AllZero(Value.reserved))
    {
        return false;
    }
    if (Value.accepted != 0)
    {
        if (Value.reason != 0)
        {
            return false;
        }
        switch (Value.operation)
        {
        case 1:
            return Value.state == 1;
        case 2:
            return Value.state == 2 || Value.state == 3;
        case 3:
            return Value.state == 4;
        default:
            return false;
        }
    }
    return Value.reason != 0
        && (Value.state == 5 || Value.state == 6);
}

inline bool Transport(const Ksa64ViewerTransportStatusV1& Value)
{
    return HasExpectedHeader(Value)
        && Value.validity_mask == MAX_uint64
        && Value.command_capacity == CommandCapacity
        && Value.commands_pending <= Value.command_capacity
        && Value.event_capacity == EventCapacity
        && Value.events_pending <= Value.event_capacity
        && Value.timeline_capacity == TimelineCapacity
        && Value.timeline_pending <= Value.timeline_capacity
        && Value.sample_capacity == SampleCapacity
        && Value.samples_pending <= Value.sample_capacity
        && Value.worker_state >= 1
        && Value.worker_state <= 3
        && Value.shutdown_requested <= 1
        && Value.finalization_state >= 1
        && Value.finalization_state <= 3
        && Value.event_overflow <= 1
        && Value.timeline_overflow <= 1
        && Value.sample_overflow <= 1
        && Value.last_command_result >= KSA64_VIEWER_EVENT_OVERFLOW
        && Value.last_command_result <= KSA64_VIEWER_UNCHANGED
        && AllZero(Value.reserved);
}

inline bool Finish(const Ksa64ViewerFinishStatusV1& Value)
{
    if (!HasExpectedHeader(Value)
        || (Value.validity_mask & ~ValidEvidence) != 0
        || !IsLifecycle(Value.lifecycle)
        || Value.finalization_state < 1
        || Value.finalization_state > 3
        || Value.shutdown_state > 2
        || !AllZero(Value.reserved))
    {
        return false;
    }
    const bool bHasEvidence = (Value.validity_mask & ValidEvidence) != 0;
    if (bHasEvidence)
    {
        return Value.finalization_state == 2
            && Value.evidence_identity != 0
            && Value.evidence_length != 0;
    }
    return Value.finalization_state != 2
        && Value.evidence_identity == 0
        && Value.evidence_length == 0
        && Value.evidence_crc32 == 0;
}
}
