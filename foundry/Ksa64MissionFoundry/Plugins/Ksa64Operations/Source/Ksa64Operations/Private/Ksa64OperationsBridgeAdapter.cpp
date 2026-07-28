#include "Ksa64OperationsBridgeAdapter.h"

#include "Ksa64BridgeModule.h"
#include "Ksa64OperationsPolicy.h"

namespace
{
constexpr uint64 LegacyValidFrame = 1ull << 0;
constexpr uint64 LegacyValidMissionTime = 1ull << 1;
constexpr uint64 LegacyValidPosition = 1ull << 2;
constexpr uint64 LegacyValidVelocity = 1ull << 3;
constexpr uint64 LegacyValidPrediction = 1ull << 7;
constexpr uint64 LegacyValidStagedLoad = 1ull << 9;
constexpr uint64 LegacyValidSafe = 1ull << 10;
constexpr uint64 TypedValidMissionTime = 1ull << 0;
constexpr uint64 TypedValidNavigation = 1ull << 1;
constexpr uint64 TypedValidGround = 1ull << 2;
constexpr uint64 TypedValidPrediction = 1ull << 3;
constexpr uint64 TypedValidProcedure = 1ull << 4;
constexpr uint64 TypedValidAction = 1ull << 5;
constexpr uint64 TypedValidDisposition = 1ull << 6;
constexpr uint64 TypedValidEvidence = 1ull << 7;
constexpr uint64 TypedValidGnss = 1ull << 8;
constexpr int32 MaximumDrainPerPoll = 256;
constexpr uint32 MaximumPredictionPoints = 4096;

FString FixedUtf8(const uint8* Bytes, uint32 DeclaredLength, uint32 Capacity)
{
    const uint32 Length = FMath::Min(DeclaredLength, Capacity);
    if (Length == 0) return {};
    FUTF8ToTCHAR Converted(reinterpret_cast<const ANSICHAR*>(Bytes), static_cast<int32>(Length));
    return FString(Converted.Length(), Converted.Get());
}

using namespace Ksa64OperationsPolicy;

EKsa64OperationsAdapterResult MapResult(int32 Result)
{
    switch (Result)
    {
    case KSA64_VIEWER_OK: return EKsa64OperationsAdapterResult::Ok;
    case KSA64_VIEWER_QUEUED: return EKsa64OperationsAdapterResult::Queued;
    case KSA64_VIEWER_NO_DATA: return EKsa64OperationsAdapterResult::NoData;
    case KSA64_VIEWER_UNCHANGED: return EKsa64OperationsAdapterResult::Unchanged;
    case KSA64_VIEWER_UNSUPPORTED:
    case KSA64_VIEWER_ACTION_UNAVAILABLE:
        return EKsa64OperationsAdapterResult::Unsupported;
    case KSA64_VIEWER_QUEUE_FULL: return EKsa64OperationsAdapterResult::QueueFull;
    case KSA64_VIEWER_LIFECYCLE:
    case KSA64_VIEWER_CLOSED:
        return EKsa64OperationsAdapterResult::Lifecycle;
    default: return EKsa64OperationsAdapterResult::Failed;
    }
}

FKsa64OperationsViewModel MapTypedOperational(
    const Ksa64ViewerOperationalViewV1& Value,
    const FString& Diagnostic,
    const FKsa64OperationsBridgeCapabilities& Capabilities)
{
    FKsa64OperationsViewModel View;
    View.bBridgeReady = true;
    View.bSessionOpen = true;
    View.bSnapshotValid = true;
    View.bTruthFiltered = IsTruthFilteredRole(Value.role);
    View.BridgeStatus = TEXT("BRIDGE 12B QUALIFIED");
    View.SessionStatus = LifecycleLabel(Value.lifecycle);
    View.RoleLabel = RoleLabel(Value.role);
    View.ValidityMask = 0;
    if ((Value.validity_mask & TypedValidMissionTime) != 0) View.ValidityMask |= LegacyValidMissionTime;
    if ((Value.validity_mask & TypedValidNavigation) != 0) View.ValidityMask |= LegacyValidPosition | LegacyValidVelocity;
    if ((Value.validity_mask & TypedValidPrediction) != 0) View.ValidityMask |= LegacyValidPrediction;
    if ((Value.validity_mask & TypedValidAction) != 0) View.ValidityMask |= LegacyValidStagedLoad;
    if (Value.safe != 0) View.ValidityMask |= LegacyValidSafe;
    View.CommandSequence = Value.publication_sequence;
    View.CommandResult = 0;
    View.DefinitionIdentity = Value.scenario_identity;
    View.Lifecycle = Value.lifecycle;
    View.BridgePace = Value.pace;
    View.ReleaseEpoch = Value.release_epoch;
    View.ReleasePeriodMicros = Value.release_period_micros;
    View.FrameIdentity = Value.frame;
    View.FrameLabel = FrameLabel(Value.frame);
    View.MissionTimeQ16 = Value.mission_time_q16;
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        View.NavigationPositionQ12[Axis] = Value.navigation_position_q12[Axis];
        View.NavigationVelocityQ24[Axis] = Value.navigation_velocity_q24[Axis];
        View.GroundPositionQ12[Axis] = Value.ground_position_q12[Axis];
        View.GroundVelocityQ24[Axis] = Value.ground_velocity_q24[Axis];
    }
    View.GnssState = Value.gnss_state;
    View.FlightChecksum = Value.flight_checksum;
    View.NavigationChecksum = Value.navigation_checksum;
    View.CommandChecksum = Value.command_checksum;
    View.ProcedureState = Value.procedure_state;
    View.ProcedureStep = Value.procedure_step;
    View.StagedLoadIdentity = Value.staged_load_identity;
    View.ActionCount = Value.action_count;
    View.RejectedLoads = Value.rejected_loads;
    View.Safe = Value.safe;
    View.PredictionIdentity = Value.prediction_identity;
    View.PredictionApogeeQ12Km = Value.prediction_apogee_q12_km;
    View.PredictionTimeToApogeeQ16 = Value.prediction_time_to_apogee_q16;
    View.PredictionTimeToImpactQ16 = Value.prediction_time_to_impact_q16;
    View.NavigationLabel = (Value.validity_mask & TypedValidNavigation) != 0
        ? FString::Printf(TEXT("ONBOARD ESTIMATE VALID · %s"), *GnssLabel(Value.gnss_state))
        : TEXT("ONBOARD ESTIMATE UNAVAILABLE");
    View.CommunicationsLabel = (Value.validity_mask & TypedValidGround) != 0
        ? TEXT("GROUND TRACKING AVAILABLE")
        : TEXT("GROUND TRACKING UNAVAILABLE");
    if ((Value.validity_mask & TypedValidPrediction) != 0)
        View.DispositionLabel = TEXT("PREDICTION ACTIVE · DISPOSITION PENDING");
    if (Value.safe != 0) View.SessionStatus += TEXT(" · SAFE");
    View.LastDiagnostic = Diagnostic;
    View.Capabilities = Capabilities;
    return View;
}

class FKsa64OperationsBridgeAdapter final : public IKsa64OperationsBridgeAdapter
{
public:
    virtual bool IsReady() const override
    {
        return FKsa64BridgeModule::IsAvailable()
            && FKsa64BridgeModule::Get().GetStatus() == EKsa64BridgeStatus::Ready;
    }

    virtual FString GetDiagnostic() const override
    {
        return FKsa64BridgeModule::IsAvailable()
            ? FKsa64BridgeModule::Get().GetDiagnostic()
            : TEXT("Ksa64Bridge module is unavailable");
    }

    virtual FKsa64OperationsBridgeCapabilities GetCapabilities() const override
    {
        FKsa64OperationsBridgeCapabilities Result;
        if (!FKsa64BridgeModule::IsAvailable()) return Result;
        const FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
        const bool bOperations = Module.SupportsFeature(KSA64_VIEWER_FEATURE_OPERATIONS_V1);
        const bool bActions = Module.SupportsFeature(KSA64_VIEWER_FEATURE_TYPED_ACTIONS_V1);
        const bool bAsync = Module.SupportsFeature(KSA64_VIEWER_FEATURE_ASYNC_STATUS_V1);
        Result.bTypedOperationalView = bOperations;
        Result.bTypedProcedure = bOperations;
        Result.bTypedActions = bActions;
        Result.bTimeline = bOperations;
        Result.bReleaseHistory = bOperations;
        Result.bPredictionPaths = bOperations
            && Module.SupportsFeature(KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1);
        Result.bTransportStatus = bAsync;
        Result.bDisposition = bOperations;
        Result.bAsyncShutdown = bAsync;
        return Result;
    }

    virtual bool StartGuidedOperations() override
    {
        if (!FKsa64BridgeModule::IsAvailable()) return false;
        FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
        bTyped = Module.SupportsFeature(KSA64_VIEWER_FEATURE_OPERATIONS_V1);
        ResetSessionCaches();
        const bool bStarted = bTyped
            ? Module.StartGuidedOperationsV1()
            : Module.StartGuidedGnssLoss();
        bGlobalDisplaySession = bStarted && Module.SupportsGlobalDisplayV1();
        return bStarted;
    }

    virtual bool StartNominalGlobalReplay() override
    {
        if (!FKsa64BridgeModule::IsAvailable()) return false;
        FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
        bTyped = false;
        ResetSessionCaches();
        const bool bStarted = Module.StartNominalGlobalReplayV1();
        bGlobalDisplaySession = bStarted && Module.SupportsGlobalDisplayV1();
        bNominalGlobalReplaySession = bGlobalDisplaySession;
        return bStarted;
    }

    virtual void Close() override
    {
        if (FKsa64BridgeModule::IsAvailable())
        {
            FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
            if (!Module.RequestAsyncClose())
            {
                // Legacy bridges have no typed async-close contract. The Phase
                // 12B adapter never takes this fallback after a typed start.
                Module.CloseSession();
            }
        }
        bTyped = false;
        bGlobalDisplaySession = false;
        bNominalGlobalReplaySession = false;
    }

    virtual EKsa64OperationsAdapterResult AdvanceOneRelease() override
    {
        return AdvanceReleases(1);
    }

    virtual EKsa64OperationsAdapterResult AdvanceReleases(uint32 MaximumReleases) override
    {
        return FKsa64BridgeModule::IsAvailable()
            ? MapResult(FKsa64BridgeModule::Get().AdvanceReleases(MaximumReleases))
            : EKsa64OperationsAdapterResult::Failed;
    }

    virtual EKsa64OperationsAdapterResult Poll(FKsa64OperationsViewModel& OutView) override
    {
        if (!FKsa64BridgeModule::IsAvailable()) return EKsa64OperationsAdapterResult::Failed;
        FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
        if (!bTyped)
        {
            Ksa64ViewerSnapshot Snapshot = {};
            const int32 Result = Module.PollSnapshot(Snapshot);
            if (Result == KSA64_VIEWER_OK)
            {
                OutView = MapLegacySnapshot(Snapshot, GetDiagnostic());
                OutView.Capabilities = GetCapabilities();
            }
            return MapResult(Result);
        }

        Ksa64ViewerOperationalViewV1 Operational = {};
        const int32 OperationalResult = Module.PollOperationalV1(Operational);
        if (OperationalResult == KSA64_VIEWER_OK)
        {
            LastOperational = Operational;
            bHasLastOperational = true;
        }
        else if (OperationalResult == KSA64_VIEWER_UNCHANGED && bHasLastOperational)
        {
            Operational = LastOperational;
        }
        else
        {
            return MapResult(OperationalResult);
        }
        OutView = MapTypedOperational(Operational, GetDiagnostic(), GetCapabilities());
        ActionGate.Expire(Operational.release_epoch);

        Ksa64ViewerProcedureViewV1 Procedure = {};
        if (Module.ProcedureV1(Procedure) == KSA64_VIEWER_OK
            && (Procedure.validity_mask & TypedValidProcedure) != 0)
        {
            OutView.ProcedureIdentity = Procedure.procedure_identity;
            OutView.ProcedureState = Procedure.state;
            OutView.ProcedureStep = Procedure.active_step;
            OutView.ProcedureStepCount = Procedure.step_count;
            OutView.ProcedureEnteredEpoch = Procedure.entered_epoch;
            OutView.ProcedureDeadlineEpoch = Procedure.deadline_epoch;
            const FString Title = FixedUtf8(Procedure.title, Procedure.title_length, sizeof(Procedure.title));
            const FString Instruction = FixedUtf8(Procedure.instruction, Procedure.instruction_length, sizeof(Procedure.instruction));
            OutView.ProcedureLabel = FString::Printf(TEXT("STEP %u/%u · %s · %s"), Procedure.active_step, Procedure.step_count, *ProcedureStateLabel(Procedure.state), *Title);
            FString Predicates;
            for (uint32 Index = 0; Index < FMath::Min<uint32>(Procedure.predicate_count, 8); ++Index)
            {
                if (!Predicates.IsEmpty()) Predicates += TEXT("  ·  ");
                Predicates += FString::Printf(TEXT("P%u:%s"), Procedure.predicate_identities[Index], Procedure.predicate_states[Index] == 2 ? TEXT("PASS") : Procedure.predicate_states[Index] == 1 ? TEXT("WAIT") : TEXT("INVALID"));
            }
            OutView.ProcedureGuard = FString::Printf(TEXT("%s\nWindow REL %u–%u\n%s"), *Instruction, Procedure.entered_epoch, Procedure.deadline_epoch, *Predicates);
        }

        Ksa64ViewerDispositionV1 Disposition = {};
        if (Module.DispositionV1(Disposition) == KSA64_VIEWER_OK
            && (Disposition.validity_mask & TypedValidDisposition) != 0)
        {
            OutView.OverallDisposition = Disposition.overall;
            OutView.ObjectiveDisposition = Disposition.objective;
            OutView.VehicleDisposition = Disposition.vehicle;
            OutView.ProcedureDisposition = Disposition.procedure;
            OutView.OperatorDisposition = Disposition.operator_disposition;
            OutView.AvionicsDisposition = Disposition.avionics;
            OutView.EvidenceDisposition = Disposition.evidence;
            OutView.DispositionLabel = FString::Printf(
                TEXT("MISSION       %s\nOBJECTIVE     %s\nVEHICLE       %s\nPROCEDURE     %s\nOPERATOR      %s\nAVIONICS      %s\nEVIDENCE      %s"),
                *OverallLabel(Disposition.overall), *ObjectiveLabel(Disposition.objective), *VehicleLabel(Disposition.vehicle), *ProcedureDispositionLabel(Disposition.procedure), *OperatorLabel(Disposition.operator_disposition), *AvionicsLabel(Disposition.avionics), *EvidenceLabel(Disposition.evidence));
        }

        Ksa64ViewerActionProposalV1 Proposal = {};
        const int32 ProposalResult = Module.ActionProposalV1(Proposal);
        if (ProposalResult == KSA64_VIEWER_OK && (Proposal.validity_mask & TypedValidAction) != 0)
        {
            if (CurrentProposal.proposal_identity != Proposal.proposal_identity)
            {
                CurrentReceipt = {};
            }
            CurrentProposal = Proposal;
            ActionGate.ObserveProposal(Proposal.proposal_identity, Proposal.expires_epoch);
            OutView.ActionProposalIdentity = Proposal.proposal_identity;
            OutView.ActionLoadIdentity = Proposal.load_identity;
            OutView.ActionEarliestCommitEpoch = Proposal.earliest_commit_epoch;
            OutView.ActionActivationEpoch = Proposal.activation_epoch;
            OutView.ActionExpiresEpoch = Proposal.expires_epoch;
            OutView.ActionPermittedOperations = Proposal.permitted_operations;
            OutView.ActionProposalLabel = FixedUtf8(Proposal.label, Proposal.label_length, sizeof(Proposal.label));
            OutView.ActionState = ActionGate.IsReviewed() ? EKsa64OperationsActionState::Reviewing : EKsa64OperationsActionState::Available;
            OutView.UplinkLabel = FString::Printf(TEXT("%s\nPROPOSAL %08X · COMMIT ≥ REL %u · ACTIVATE REL %u · EXPIRES REL %u"), *OutView.ActionProposalLabel, Proposal.proposal_identity, Proposal.earliest_commit_epoch, Proposal.activation_epoch, Proposal.expires_epoch);
        }
        else if (CurrentProposal.proposal_identity != 0
            && Operational.release_epoch <= CurrentProposal.expires_epoch)
        {
            OutView.ActionProposalIdentity = CurrentProposal.proposal_identity;
            OutView.ActionLoadIdentity = CurrentProposal.load_identity;
            OutView.ActionEarliestCommitEpoch = CurrentProposal.earliest_commit_epoch;
            OutView.ActionActivationEpoch = CurrentProposal.activation_epoch;
            OutView.ActionExpiresEpoch = CurrentProposal.expires_epoch;
            OutView.ActionPermittedOperations = CurrentProposal.permitted_operations;
            OutView.ActionProposalLabel = FixedUtf8(CurrentProposal.label, CurrentProposal.label_length, sizeof(CurrentProposal.label));
            OutView.ActionState = ActionGate.IsReviewed() ? EKsa64OperationsActionState::Reviewing : EKsa64OperationsActionState::Available;
            OutView.UplinkLabel = FString::Printf(TEXT("%s\nPROPOSAL %08X · COMMIT ≥ REL %u · ACTIVATE REL %u · EXPIRES REL %u"), *OutView.ActionProposalLabel, CurrentProposal.proposal_identity, CurrentProposal.earliest_commit_epoch, CurrentProposal.activation_epoch, CurrentProposal.expires_epoch);
        }

        Ksa64ViewerActionReceiptV1 Receipt = {};
        const int32 ReceiptResult = Module.PollActionReceiptV1(Receipt);
        if (ReceiptResult == KSA64_VIEWER_OK)
        {
            CurrentReceipt = Receipt;
            ActionGate.ObserveReceipt(Receipt.proposal_identity, Receipt.state, Receipt.accepted != 0);
            OutView.ActionReceiptSequence = Receipt.publication_sequence;
            OutView.ActionReceiptState = Receipt.state;
            OutView.ActionReceiptReason = Receipt.reason;
            OutView.ActionReceiptAccepted = Receipt.accepted;
            OutView.ActionReceiptLabel = FString::Printf(TEXT("RECEIPT %llu · STATE %u · REASON %u · %s"), static_cast<unsigned long long>(Receipt.publication_sequence), Receipt.state, Receipt.reason, Receipt.accepted ? TEXT("ACCEPTED") : TEXT("REJECTED"));
        }
        else if (CurrentReceipt.publication_sequence != 0)
        {
            OutView.ActionReceiptSequence = CurrentReceipt.publication_sequence;
            OutView.ActionReceiptState = CurrentReceipt.state;
            OutView.ActionReceiptReason = CurrentReceipt.reason;
            OutView.ActionReceiptAccepted = CurrentReceipt.accepted;
            OutView.ActionReceiptLabel = FString::Printf(TEXT("RECEIPT %llu · STATE %u · REASON %u · %s"), static_cast<unsigned long long>(CurrentReceipt.publication_sequence), CurrentReceipt.state, CurrentReceipt.reason, CurrentReceipt.accepted ? TEXT("ACCEPTED") : TEXT("REJECTED"));
        }
        if (CurrentReceipt.publication_sequence != 0)
        {
            OutView.ActionState = ActionStateFromReceipt(ActionGate.ReceiptState());
            OutView.UplinkLabel += TEXT("\n") + OutView.ActionReceiptLabel;
        }

        // The legacy event stream remains part of ABI v1 even though Phase
        // 12B presents the richer typed timeline. Consume and validate it so
        // a typed client cannot silently overflow an unobserved compatibility
        // queue during a long mission.
        for (int32 Count = 0; Count < MaximumDrainPerPoll; ++Count)
        {
            Ksa64ViewerEvent Event = {};
            const int32 EventResult = Module.PollEvent(Event);
            if (EventResult == KSA64_VIEWER_NO_DATA)
            {
                break;
            }
            if (EventResult != KSA64_VIEWER_OK)
            {
                return MapResult(EventResult);
            }
        }

        Ksa64ViewerTransportStatusV1 Transport = {};
        if (Module.TransportStatusV1(Transport) == KSA64_VIEWER_OK)
        {
            OutView.CommandCapacity = Transport.command_capacity;
            OutView.CommandsPending = Transport.commands_pending;
            OutView.TimelineCapacity = Transport.timeline_capacity;
            OutView.TimelinePending = Transport.timeline_pending;
            OutView.SampleCapacity = Transport.sample_capacity;
            OutView.SamplesPending = Transport.samples_pending;
            OutView.WorkerState = Transport.worker_state;
            OutView.FinalizationState = Transport.finalization_state;
            OutView.CommandResult = Transport.last_command_result;
            OutView.TransportOverflow = Transport.event_overflow
                | (Transport.timeline_overflow << 1)
                | (Transport.sample_overflow << 2);
            if (OutView.TransportOverflow != 0) OutView.bObservationComplete = false;
        }

        Ksa64ViewerFinishStatusV1 Finish = {};
        if (Module.FinishStatusV1(Finish) == KSA64_VIEWER_OK)
        {
            OutView.EvidenceIdentity = Finish.evidence_identity;
            OutView.EvidenceLength = Finish.evidence_length;
            OutView.EvidenceCrc32 = Finish.evidence_crc32;
            OutView.FinalizationState = Finish.finalization_state;
            switch (ClassifyEvidenceReadiness(
                Finish.lifecycle,
                Finish.finalization_state,
                OutView.WorkerState,
                Finish.evidence_length,
                Finish.validity_mask))
            {
            case EKsa64OperationsEvidenceReadiness::Complete:
                OutView.EvidenceStatus = TEXT("EVIDENCE VERIFIED / READY TO SAVE");
                break;
            case EKsa64OperationsEvidenceReadiness::Failed:
                OutView.EvidenceStatus = TEXT("EVIDENCE FAILED / UNAVAILABLE");
                OutView.bObservationComplete = false;
                break;
            default:
                OutView.EvidenceStatus = TEXT("EVIDENCE FINALIZATION IN PROGRESS");
                break;
            }
        }
        RefreshPrediction();
        RefreshTrajectoryPaths();
        return EKsa64OperationsAdapterResult::Ok;
    }

    virtual void DrainTimeline(TArray<FKsa64OperationsTimelineItem>& OutItems) override
    {
        OutItems.Reset();
        if (!bTyped || !FKsa64BridgeModule::IsAvailable()) return;
        for (int32 Count = 0; Count < MaximumDrainPerPoll; ++Count)
        {
            Ksa64ViewerTimelineEventV1 Event = {};
            const int32 Result = FKsa64BridgeModule::Get().PollTimelineV1(Event);
            if (Result != KSA64_VIEWER_OK) break;
            FKsa64OperationsTimelineItem Item;
            Item.Sequence = Event.sequence;
            Item.ReleaseEpoch = Event.release_epoch;
            Item.Category = TimelineSourceLabel(Event.source);
            Item.Summary = FixedUtf8(Event.label, Event.label_length, sizeof(Event.label));
            Item.bAttention = Event.severity >= 1;
            OutItems.Add(MoveTemp(Item));
        }
    }

    virtual void DrainReleaseSamples(TArray<FKsa64OperationsReleasePoint>& OutSamples) override
    {
        OutSamples.Reset();
        if (!bTyped || !FKsa64BridgeModule::IsAvailable()) return;
        for (int32 Count = 0; Count < MaximumDrainPerPoll; ++Count)
        {
            Ksa64ViewerReleaseSampleV1 Sample = {};
            const int32 Result = FKsa64BridgeModule::Get().PollReleaseSampleV1(Sample);
            if (Result != KSA64_VIEWER_OK) break;
            FKsa64OperationsReleasePoint Point;
            Point.ReleaseEpoch = Sample.release_epoch;
            Point.MissionTimeQ16 = Sample.mission_time_q16;
            Point.FrameIdentity = Sample.frame;
            Point.bHasMissionTime = (Sample.validity_mask & TypedValidMissionTime) != 0;
            Point.bHasPosition = (Sample.validity_mask & TypedValidNavigation) != 0;
            Point.bHasGroundEstimate = (Sample.validity_mask & TypedValidGround) != 0;
            Point.AltitudeQ12Km = Sample.altitude_q12_km;
            Point.SpeedQ24KmS = Sample.speed_q24_km_s;
            Point.DownrangeQ12Km = Sample.downrange_q12_km;
            Point.CrossrangeQ12Km = Sample.crossrange_q12_km;
            for (int32 Axis = 0; Axis < 3; ++Axis)
            {
                Point.PositionQ12[Axis] = Sample.onboard_position_q12[Axis];
                Point.GroundPositionQ12[Axis] = Sample.ground_position_q12[Axis];
            }
            OutSamples.Add(Point);
        }
    }

    virtual void ReadPredictionPath(TArray<FKsa64OperationsPredictionPoint>& OutPoints) override
    {
        OutPoints = CachedPrediction;
    }

    virtual void ReadTrajectoryPath(
        EKsa64OperationsTrajectorySource Source,
        TArray<FKsa64OperationsPredictionPoint>& OutPoints) override
    {
        switch (Source)
        {
        case EKsa64OperationsTrajectorySource::PlannedReference:
            OutPoints = CachedPlannedReference;
            break;
        case EKsa64OperationsTrajectorySource::OnboardEstimate:
            OutPoints = CachedOnboardEstimate;
            break;
        case EKsa64OperationsTrajectorySource::GroundEstimate:
            OutPoints = CachedGroundEstimate;
            break;
        default:
            OutPoints.Reset();
            break;
        }
    }

    virtual bool SupportsGlobalDisplayV1() const override
    {
        return bGlobalDisplaySession
            && FKsa64BridgeModule::IsAvailable()
            && FKsa64BridgeModule::Get().SupportsGlobalDisplayV1();
    }

    virtual EKsa64OperationsAdapterResult GlobalDisplayAvailability(
        Ksa64GlobalDisplayAvailabilityV1& OutAvailability) const override
    {
        return SupportsGlobalDisplayV1()
            ? MapResult(FKsa64BridgeModule::Get().GlobalDisplayAvailability(OutAvailability))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult GlobalDisplayDefinition(
        TArray<uint8>& OutPayload) const override
    {
        return SupportsGlobalDisplayV1()
            ? MapResult(FKsa64BridgeModule::Get().GlobalDisplayDefinition(OutPayload))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult PollGlobalDisplaySample(
        TArray<uint8>& OutPayload) const override
    {
        return SupportsGlobalDisplayV1()
            ? MapResult(FKsa64BridgeModule::Get().PollGlobalDisplaySample(OutPayload))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult GlobalDisplaySampleRange(
        const Ksa64GlobalDisplaySampleRangeRequestV1& Request,
        TArray<uint8>& OutPayload) const override
    {
        return SupportsGlobalDisplayV1()
            ? MapResult(FKsa64BridgeModule::Get().GlobalDisplaySampleRange(Request, OutPayload))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult PollGlobalDisplayTransition(
        TArray<uint8>& OutPayload) const override
    {
        return SupportsGlobalDisplayV1()
            ? MapResult(FKsa64BridgeModule::Get().PollGlobalDisplayTransition(OutPayload))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult GlobalReplayIndex(
        TArray<uint8>& OutPayload) const override
    {
        return SupportsGlobalDisplayV1()
            ? MapResult(FKsa64BridgeModule::Get().GlobalReplayIndex(OutPayload))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult GlobalPathChunk(
        const Ksa64GlobalDisplayPathRequestV1& Request,
        TArray<uint8>& OutPayload) const override
    {
        return SupportsGlobalDisplayV1()
            ? MapResult(FKsa64BridgeModule::Get().GlobalPathChunk(Request, OutPayload))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult ReviewAction() override
    {
        if (!bTyped || !ActionGate.Review(LastOperational.release_epoch))
            return EKsa64OperationsAdapterResult::Unsupported;
        return EKsa64OperationsAdapterResult::Ok;
    }

    virtual EKsa64OperationsAdapterResult StageAction() override
    {
        if (!bTyped || !ActionGate.CanStage(LastOperational.release_epoch))
            return EKsa64OperationsAdapterResult::Unsupported;
        return MapResult(FKsa64BridgeModule::Get().SubmitActionProposalV1(
            CurrentProposal.proposal_identity,
            CurrentProposal.completed_event_mask));
    }

    virtual EKsa64OperationsAdapterResult CommitAction() override
    {
        if (!bTyped
            || LastOperational.release_epoch < CurrentProposal.earliest_commit_epoch
            || !ActionGate.CanCommit(LastOperational.release_epoch))
            return EKsa64OperationsAdapterResult::Unsupported;
        return MapResult(FKsa64BridgeModule::Get().CommitActionV1(CurrentProposal.proposal_identity));
    }

    virtual EKsa64OperationsAdapterResult CancelAction() override
    {
        if (!bTyped || !ActionGate.CanCancel(LastOperational.release_epoch))
            return EKsa64OperationsAdapterResult::Unsupported;
        return MapResult(FKsa64BridgeModule::Get().CancelActionV1(CurrentProposal.proposal_identity));
    }

    virtual EKsa64OperationsAdapterResult RequestShutdown() override
    {
        if (!FKsa64BridgeModule::IsAvailable())
        {
            return EKsa64OperationsAdapterResult::Unsupported;
        }
        FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
        if (bNominalGlobalReplaySession)
        {
            Module.CloseSession();
            bGlobalDisplaySession = false;
            bNominalGlobalReplaySession = false;
            return EKsa64OperationsAdapterResult::Ok;
        }
        return bTyped
            ? MapResult(Module.RequestShutdownV1())
            : EKsa64OperationsAdapterResult::Unsupported;
    }

    virtual EKsa64OperationsAdapterResult GetCompletedEvidence(TArray<uint8>& OutBytes) const override
    {
        return bTyped && FKsa64BridgeModule::IsAvailable()
            ? MapResult(FKsa64BridgeModule::Get().GetCompletedKsb11(OutBytes))
            : EKsa64OperationsAdapterResult::Unsupported;
    }

private:
    void ResetSessionCaches()
    {
        ActionGate.Reset();
        bHasLastOperational = false;
        LastOperational = {};
        CurrentProposal = {};
        CurrentReceipt = {};
        CachedPrediction.Reset();
        CachedPredictionIdentity = 0;
        CachedPlannedReference.Reset();
        CachedOnboardEstimate.Reset();
        CachedGroundEstimate.Reset();
        CachedPlannedReferenceIdentity = 0;
        CachedOnboardEstimateIdentity = 0;
        CachedGroundEstimateIdentity = 0;
        bGlobalDisplaySession = false;
        bNominalGlobalReplaySession = false;
    }

    void RefreshPrediction()
    {
        if (!bTyped || !FKsa64BridgeModule::IsAvailable()) return;
        Ksa64ViewerPredictionPathHeaderV1 Header = {};
        if (FKsa64BridgeModule::Get().PredictionPathHeaderV1(Header) != KSA64_VIEWER_OK
            || Header.path_identity == 0
            || Header.path_identity == CachedPredictionIdentity)
            return;
        if (Header.point_count > MaximumPredictionPoints)
        {
            CachedPrediction.Reset();
            CachedPredictionIdentity = 0;
            return;
        }
        TArray<FKsa64OperationsPredictionPoint> Candidate;
        Candidate.Reserve(static_cast<int32>(Header.point_count));
        for (uint32 Index = 0; Index < Header.point_count; ++Index)
        {
            Ksa64ViewerPredictionPathPointV1 Value = {};
            if (FKsa64BridgeModule::Get().PredictionPathPointV1(Index, Value) != KSA64_VIEWER_OK
                || Value.path_identity != Header.path_identity
                || Value.point_index != Index)
                return;
            FKsa64OperationsPredictionPoint Point;
            Point.PathIdentity = Value.path_identity;
            Point.ProductIdentity = Header.product;
            Point.ReleaseEpoch = Value.release_epoch;
            Point.FrameIdentity = Value.frame;
            Point.AltitudeQ12Km = Value.altitude_q12_km;
            Point.DownrangeQ12Km = Value.downrange_q12_km;
            Point.CrossrangeQ12Km = Value.crossrange_q12_km;
            for (int32 Axis = 0; Axis < 3; ++Axis) Point.PositionQ12Km[Axis] = Value.position_q12_km[Axis];
            Candidate.Add(Point);
        }
        CachedPrediction = MoveTemp(Candidate);
        CachedPredictionIdentity = Header.path_identity;
    }

    void RefreshTrajectoryPaths()
    {
        if (!bTyped || !FKsa64BridgeModule::IsAvailable()
            || !FKsa64BridgeModule::Get().SupportsFeature(KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1))
            return;
        RefreshTrajectoryPath(
            KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE,
            KSA64_VIEWER_TRAJECTORY_PRODUCT_PLANNED_REFERENCE,
            CachedPlannedReference,
            CachedPlannedReferenceIdentity);
        RefreshTrajectoryPath(
            KSA64_VIEWER_TRAJECTORY_ONBOARD_ESTIMATE,
            2u,
            CachedOnboardEstimate,
            CachedOnboardEstimateIdentity);
        RefreshTrajectoryPath(
            KSA64_VIEWER_TRAJECTORY_GROUND_ESTIMATE,
            3u,
            CachedGroundEstimate,
            CachedGroundEstimateIdentity);
    }

    void RefreshTrajectoryPath(
        uint32 Source,
        uint32 ExpectedProduct,
        TArray<FKsa64OperationsPredictionPoint>& OutCache,
        uint32& InOutIdentity)
    {
        FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
        Ksa64ViewerPredictionPathHeaderV1 Header = {};
        const int32 HeaderResult = Module.TrajectoryPathHeaderV1(Source, Header);
        if (HeaderResult == KSA64_VIEWER_NO_DATA)
        {
            OutCache.Reset();
            InOutIdentity = 0;
            return;
        }
        if (HeaderResult != KSA64_VIEWER_OK
            || Header.path_identity == 0
            || Header.product != ExpectedProduct
            || Header.path_identity == InOutIdentity)
            return;
        if (Header.point_count > MaximumPredictionPoints)
        {
            OutCache.Reset();
            InOutIdentity = 0;
            return;
        }
        TArray<FKsa64OperationsPredictionPoint> Candidate;
        Candidate.Reserve(static_cast<int32>(Header.point_count));
        for (uint32 Index = 0; Index < Header.point_count; ++Index)
        {
            Ksa64ViewerPredictionPathPointV1 Value = {};
            if (Module.TrajectoryPathPointV1(Source, Index, Value) != KSA64_VIEWER_OK
                || Value.path_identity != Header.path_identity
                || Value.point_index != Index)
                return;
            FKsa64OperationsPredictionPoint Point;
            Point.PathIdentity = Value.path_identity;
            Point.ProductIdentity = Header.product;
            Point.ReleaseEpoch = Value.release_epoch;
            Point.FrameIdentity = Value.frame;
            Point.AltitudeQ12Km = Value.altitude_q12_km;
            Point.DownrangeQ12Km = Value.downrange_q12_km;
            Point.CrossrangeQ12Km = Value.crossrange_q12_km;
            for (int32 Axis = 0; Axis < 3; ++Axis) Point.PositionQ12Km[Axis] = Value.position_q12_km[Axis];
            Candidate.Add(Point);
        }
        OutCache = MoveTemp(Candidate);
        InOutIdentity = Header.path_identity;
    }

    bool bTyped = false;
    bool bGlobalDisplaySession = false;
    bool bNominalGlobalReplaySession = false;
    FKsa64OperationsActionGate ActionGate;
    bool bHasLastOperational = false;
    uint32 CachedPredictionIdentity = 0;
    uint32 CachedPlannedReferenceIdentity = 0;
    uint32 CachedOnboardEstimateIdentity = 0;
    uint32 CachedGroundEstimateIdentity = 0;
    Ksa64ViewerOperationalViewV1 LastOperational = {};
    Ksa64ViewerActionProposalV1 CurrentProposal = {};
    Ksa64ViewerActionReceiptV1 CurrentReceipt = {};
    TArray<FKsa64OperationsPredictionPoint> CachedPrediction;
    TArray<FKsa64OperationsPredictionPoint> CachedPlannedReference;
    TArray<FKsa64OperationsPredictionPoint> CachedOnboardEstimate;
    TArray<FKsa64OperationsPredictionPoint> CachedGroundEstimate;
};
}

TUniquePtr<IKsa64OperationsBridgeAdapter> IKsa64OperationsBridgeAdapter::Create()
{
    return MakeUnique<FKsa64OperationsBridgeAdapter>();
}

FKsa64OperationsViewModel IKsa64OperationsBridgeAdapter::MapLegacySnapshot(
    const Ksa64ViewerSnapshot& Snapshot,
    const FString& BridgeDiagnostic)
{
    FKsa64OperationsViewModel View;
    View.bBridgeReady = true;
    View.bSessionOpen = true;
    View.bSnapshotValid = true;
    View.bTruthFiltered = IsTruthFilteredRole(Snapshot.role);
    View.BridgeStatus = TEXT("BRIDGE 12A COMPATIBILITY");
    View.SessionStatus = LifecycleLabel(Snapshot.lifecycle);
    View.RoleLabel = RoleLabel(Snapshot.role);
    View.ValidityMask = Snapshot.validity_mask;
    View.CommandSequence = Snapshot.command_sequence;
    View.CommandResult = Snapshot.command_result;
    View.DefinitionIdentity = Snapshot.definition_identity;
    View.Lifecycle = Snapshot.lifecycle;
    View.BridgePace = Snapshot.pace;
    View.ReleaseEpoch = Snapshot.release_epoch;
    View.ReleasePeriodMicros = Snapshot.release_period_micros;
    View.FrameIdentity = Snapshot.frame;
    View.FrameLabel = (Snapshot.validity_mask & LegacyValidFrame) != 0 ? FrameLabel(Snapshot.frame) : TEXT("FRAME —");
    View.MissionTimeQ16 = Snapshot.mission_time_q16;
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        View.NavigationPositionQ12[Axis] = Snapshot.navigation_position_q12[Axis];
        View.NavigationVelocityQ24[Axis] = Snapshot.navigation_velocity_q24[Axis];
    }
    View.FlightChecksum = Snapshot.flight_checksum;
    View.NavigationChecksum = Snapshot.navigation_checksum;
    View.CommandChecksum = Snapshot.command_checksum;
    View.ProcedureState = Snapshot.procedure_state;
    View.ProcedureStep = Snapshot.procedure_step;
    View.StagedLoadIdentity = Snapshot.staged_load_identity;
    View.ActionCount = Snapshot.action_count;
    View.EventCount = Snapshot.event_count;
    View.RejectedLoads = Snapshot.rejected_loads;
    View.Safe = Snapshot.safe;
    View.PredictionIdentity = Snapshot.prediction_identity;
    View.PredictionApogeeQ12Km = Snapshot.prediction_apogee_q12_km;
    View.PredictionTimeToApogeeQ16 = Snapshot.prediction_time_to_apogee_q16;
    View.ProcedureLabel = FString::Printf(TEXT("STEP %u · %s"), Snapshot.procedure_step, *ProcedureStateLabel(Snapshot.procedure_state));
    View.ProcedureGuard = TEXT("Detailed guard requires the typed Phase 12B bridge");
    View.NavigationLabel = (Snapshot.validity_mask & (LegacyValidPosition | LegacyValidVelocity)) != 0 ? TEXT("ONBOARD ESTIMATE VALID") : TEXT("ONBOARD ESTIMATE UNAVAILABLE");
    if ((Snapshot.validity_mask & LegacyValidStagedLoad) != 0)
    {
        View.UplinkLabel = FString::Printf(TEXT("LOAD %08X STAGED · typed receipt unavailable"), Snapshot.staged_load_identity);
        View.ActionState = EKsa64OperationsActionState::Staged;
    }
    else
    {
        View.UplinkLabel = TEXT("Typed action service not negotiated");
        View.ActionState = EKsa64OperationsActionState::Unavailable;
    }
    if ((Snapshot.validity_mask & LegacyValidPrediction) != 0) View.DispositionLabel = TEXT("PREDICTION ACTIVE · DISPOSITION PENDING");
    if ((Snapshot.validity_mask & LegacyValidSafe) != 0 && Snapshot.safe != 0) View.SessionStatus += TEXT(" · SAFE");
    View.LastDiagnostic = BridgeDiagnostic;
    return View;
}
