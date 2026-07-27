#include "Ksa64OperationsPolicy.h"

namespace Ksa64OperationsPolicy
{
bool IsTruthFilteredRole(uint32 Role)
{
    // Unknown roles fail closed. SIM Director is the sole truth-bearing role.
    return Role != SimDirectorRole;
}

FString RoleLabel(uint32 Role)
{
    switch (Role)
    {
    case 1: return TEXT("OBSERVER");
    case 2: return TEXT("GUIDED OPERATOR");
    case 3: return TEXT("FLIGHT CONTROLLER");
    case 4: return TEXT("FLIGHT-SOFTWARE ENGINEER");
    case 5: return TEXT("SIM DIRECTOR");
    case 6: return TEXT("SCRIPTED OPERATOR");
    default: return FString::Printf(TEXT("ROLE %u"), Role);
    }
}

FString LifecycleLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("COMPILED");
    case 2: return TEXT("READY");
    case 3: return TEXT("RUNNING");
    case 4: return TEXT("PAUSED");
    case 5: return TEXT("COMPLETED");
    case 6: return TEXT("ABORTED");
    default: return FString::Printf(TEXT("LIFECYCLE %u"), Value);
    }
}

FString FrameLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("LOCAL ENU");
    case 2: return TEXT("EARTH FIXED / ECEF");
    case 3: return TEXT("EARTH INERTIAL / GCRF");
    default: return FString::Printf(TEXT("FRAME %u"), Value);
    }
}

FString ProcedureStateLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("ACTIVE");
    case 2: return TEXT("COMPLETED");
    case 3: return TEXT("SKIPPED");
    case 4: return TEXT("FAILED");
    case 5: return TEXT("MISTIMED");
    case 6: return TEXT("MANUAL OVERRIDE");
    default: return TEXT("UNAVAILABLE");
    }
}

FString GnssLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("GNSS VALID");
    case 2: return TEXT("GNSS LOST / QUALIFYING");
    case 3: return TEXT("GNSS INVALID / INERTIAL PROPAGATION");
    default: return TEXT("GNSS STATE UNKNOWN");
    }
}

FString OverallLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("NOMINAL SUCCESS");
    case 2: return TEXT("DEGRADED SUCCESS");
    case 3: return TEXT("CONTINGENCY SUCCESS");
    case 4: return TEXT("MISSION FAILURE");
    case 5: return TEXT("INDETERMINATE");
    default: return TEXT("PENDING");
    }
}

FString ObjectiveLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("PRIMARY ACHIEVED");
    case 2: return TEXT("ALTERNATE ACHIEVED");
    case 3: return TEXT("CONTINGENCY ACHIEVED");
    case 4: return TEXT("NOT ACHIEVED");
    case 5: return TEXT("INDETERMINATE");
    default: return TEXT("PENDING");
    }
}

FString VehicleLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("NOMINAL");
    case 2: return TEXT("DEGRADED");
    case 3: return TEXT("RECOVERED");
    case 4: return TEXT("SAFE STATE");
    case 5: return TEXT("LOST");
    case 6: return TEXT("UNKNOWN");
    default: return TEXT("PENDING");
    }
}

FString ProcedureDispositionLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("COMPLETED");
    case 2: return TEXT("ALTERNATE BRANCH");
    case 3: return TEXT("SKIPPED");
    case 4: return TEXT("MISTIMED");
    case 5: return TEXT("OVERRIDDEN");
    case 6: return TEXT("FAILED");
    default: return TEXT("PENDING");
    }
}

FString OperatorLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("TIMELY REFERENCE");
    case 2: return TEXT("TIMELY ALTERNATE");
    case 3: return TEXT("DELAYED VALID");
    case 4: return TEXT("NO ACTION");
    case 5: return TEXT("REJECTED ACTION");
    default: return TEXT("PENDING");
    }
}

FString AvionicsLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("NOMINAL");
    case 2: return TEXT("DEGRADED OPERATIONAL");
    case 3: return TEXT("SAFE RECOVERY");
    case 4: return TEXT("FAILED");
    default: return TEXT("UNKNOWN");
    }
}

FString EvidenceLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("COMPLETE");
    case 2: return TEXT("OBSERVATION INCOMPLETE");
    case 3: return TEXT("ABORTED");
    case 4: return TEXT("INVALID");
    case 5: return TEXT("UNAVAILABLE");
    default: return TEXT("PENDING");
    }
}

FString TimelineSourceLabel(uint32 Value)
{
    switch (Value)
    {
    case 1: return TEXT("WORLD");
    case 2: return TEXT("AVIONICS");
    case 3: return TEXT("GROUND");
    case 4: return TEXT("PROCEDURE");
    case 5: return TEXT("OPERATOR");
    case 6: return TEXT("EVIDENCE");
    default: return TEXT("SYSTEM");
    }
}

EKsa64OperationsActionState ActionStateFromReceipt(uint32 State)
{
    switch (State)
    {
    case 1: return EKsa64OperationsActionState::Staged;
    case 2: return EKsa64OperationsActionState::Committed;
    case 3: return EKsa64OperationsActionState::Executed;
    case 4: return EKsa64OperationsActionState::Cancelled;
    case 5: return EKsa64OperationsActionState::Rejected;
    case 6: return EKsa64OperationsActionState::Expired;
    default: return EKsa64OperationsActionState::Unavailable;
    }
}

void FKsa64OperationsPacingController::Reset()
{
    WallDebtNanoseconds = 0;
}

void FKsa64OperationsPacingController::Accumulate(
    float DeltaSeconds,
    EKsa64OperationsPace Pace)
{
    AccumulateNanoseconds(
        FMath::Max<int64>(
            0,
            FMath::RoundToInt64(static_cast<double>(DeltaSeconds) * 1'000'000'000.0)),
        Pace);
}

void FKsa64OperationsPacingController::AccumulateNanoseconds(
    int64 FrameNanoseconds,
    EKsa64OperationsPace Pace)
{
    if (Pace == EKsa64OperationsPace::Paused || Pace == EKsa64OperationsPace::Fastest)
    {
        return;
    }
    const int64 Multiplier = Pace == EKsa64OperationsPace::FourX
        ? 4
        : Pace == EKsa64OperationsPace::SixteenX ? 16 : 1;
    const int64 BoundedFrameNanoseconds = FMath::Clamp<int64>(
        FrameNanoseconds,
        0,
        MaximumWallDebtNanoseconds);
    const int64 BoundedAddition = FMath::Min<int64>(
        MaximumWallDebtNanoseconds,
        BoundedFrameNanoseconds * Multiplier);
    WallDebtNanoseconds = FMath::Min<int64>(
        MaximumWallDebtNanoseconds,
        WallDebtNanoseconds + BoundedAddition);
}

uint32 FKsa64OperationsPacingController::ReleasesDue(
    EKsa64OperationsPace Pace,
    uint32 ReleasePeriodMicros,
    bool bSessionRunnable,
    bool bAdvanceOutstanding) const
{
    if (!bSessionRunnable
        || bAdvanceOutstanding
        || Pace == EKsa64OperationsPace::Paused
        || ReleasePeriodMicros == 0)
    {
        return 0;
    }
    if (Pace == EKsa64OperationsPace::Fastest)
    {
        return MaximumAdvanceReleases;
    }
    const int64 PeriodNanoseconds = static_cast<int64>(ReleasePeriodMicros) * 1'000;
    return static_cast<uint32>(FMath::Clamp<int64>(
        WallDebtNanoseconds / PeriodNanoseconds,
        0,
        MaximumAdvanceReleases));
}

void FKsa64OperationsPacingController::CommitAcceptedAdvance(
    uint32 Releases,
    uint32 ReleasePeriodMicros,
    EKsa64OperationsPace Pace)
{
    if (Pace == EKsa64OperationsPace::Fastest)
    {
        return;
    }
    const int64 PeriodNanoseconds = static_cast<int64>(ReleasePeriodMicros) * 1'000;
    WallDebtNanoseconds = FMath::Max<int64>(
        0,
        WallDebtNanoseconds - static_cast<int64>(Releases) * PeriodNanoseconds);
}

void FKsa64OperationsAdvanceTracker::Reset()
{
    BaselinePublicationSequence = 0;
    bOutstanding = false;
}

void FKsa64OperationsAdvanceTracker::MarkAccepted(uint64 PublicationSequence)
{
    BaselinePublicationSequence = PublicationSequence;
    bOutstanding = true;
}

bool FKsa64OperationsAdvanceTracker::Observe(
    uint64 PublicationSequence,
    uint32 CommandsPending,
    uint32 Lifecycle)
{
    if (!bOutstanding)
    {
        return false;
    }
    const bool bTerminal = Lifecycle == 5 || Lifecycle == 6;
    if (bTerminal || (CommandsPending == 0 && PublicationSequence != BaselinePublicationSequence))
    {
        bOutstanding = false;
        return true;
    }
    return false;
}

void FKsa64OperationsActionGate::Reset()
{
    CurrentProposalIdentity = 0;
    CurrentExpiresEpoch = 0;
    CurrentReceiptState = 0;
    bReceiptAccepted = false;
    bReviewed = false;
}

void FKsa64OperationsActionGate::ObserveProposal(uint32 ProposalIdentity, uint32 ExpiresEpoch)
{
    if (CurrentProposalIdentity != ProposalIdentity)
    {
        CurrentReceiptState = 0;
        bReceiptAccepted = false;
        bReviewed = false;
    }
    CurrentProposalIdentity = ProposalIdentity;
    CurrentExpiresEpoch = ExpiresEpoch;
}

void FKsa64OperationsActionGate::ObserveReceipt(
    uint32 ProposalIdentity,
    uint32 ReceiptState,
    bool bAccepted)
{
    if (ProposalIdentity != CurrentProposalIdentity)
    {
        return;
    }
    CurrentReceiptState = ReceiptState;
    bReceiptAccepted = bAccepted;
}

void FKsa64OperationsActionGate::Expire(uint32 ReleaseEpoch)
{
    if (CurrentProposalIdentity != 0 && ReleaseEpoch > CurrentExpiresEpoch)
    {
        CurrentReceiptState = 6;
        bReceiptAccepted = false;
        bReviewed = false;
    }
}

bool FKsa64OperationsActionGate::Review(uint32 ReleaseEpoch)
{
    if (!IsCurrent(ReleaseEpoch) || CurrentReceiptState != 0)
    {
        return false;
    }
    bReviewed = true;
    return true;
}

bool FKsa64OperationsActionGate::CanStage(uint32 ReleaseEpoch) const
{
    return IsCurrent(ReleaseEpoch) && bReviewed && CurrentReceiptState == 0;
}

bool FKsa64OperationsActionGate::CanCommit(uint32 ReleaseEpoch) const
{
    return IsCurrent(ReleaseEpoch) && bReceiptAccepted && CurrentReceiptState == 1;
}

bool FKsa64OperationsActionGate::CanCancel(uint32 ReleaseEpoch) const
{
    return IsCurrent(ReleaseEpoch)
        && bReceiptAccepted
        && (CurrentReceiptState == 1 || CurrentReceiptState == 2);
}

bool FKsa64OperationsActionGate::IsCurrent(uint32 ReleaseEpoch) const
{
    return CurrentProposalIdentity != 0 && ReleaseEpoch <= CurrentExpiresEpoch;
}

EKsa64OperationsEvidenceReadiness ClassifyEvidenceReadiness(
    uint32 Lifecycle,
    uint32 FinalizationState,
    uint32 WorkerState,
    uint64 EvidenceLength,
    uint64 EvidenceValidityMask)
{
    if (Lifecycle == 6 || FinalizationState == 3 || WorkerState == 3)
    {
        return EKsa64OperationsEvidenceReadiness::Failed;
    }
    if (Lifecycle == 5
        && FinalizationState == 2
        && EvidenceLength > 0
        && EvidenceValidityMask != 0)
    {
        return EKsa64OperationsEvidenceReadiness::Complete;
    }
    return EKsa64OperationsEvidenceReadiness::InProgress;
}

void MergeReleaseSamples(
    TArray<FKsa64OperationsReleasePoint>& History,
    const TArray<FKsa64OperationsReleasePoint>& Incoming,
    int32 MaximumPoints,
    bool& bObservationComplete)
{
    for (const FKsa64OperationsReleasePoint& Point : Incoming)
    {
        int32 InsertAt = 0;
        while (InsertAt < History.Num() && History[InsertAt].ReleaseEpoch < Point.ReleaseEpoch)
        {
            ++InsertAt;
        }
        if (InsertAt < History.Num() && History[InsertAt].ReleaseEpoch == Point.ReleaseEpoch)
        {
            // Typed bridge samples replace sparse snapshot fallbacks at the same
            // exact release because they carry the richer operational fields.
            History[InsertAt] = Point;
        }
        else
        {
            History.Insert(Point, InsertAt);
        }
    }
    if (MaximumPoints < 0)
    {
        MaximumPoints = 0;
    }
    if (History.Num() > MaximumPoints)
    {
        History.RemoveAt(0, History.Num() - MaximumPoints, EAllowShrinking::No);
        bObservationComplete = false;
    }
}
}
