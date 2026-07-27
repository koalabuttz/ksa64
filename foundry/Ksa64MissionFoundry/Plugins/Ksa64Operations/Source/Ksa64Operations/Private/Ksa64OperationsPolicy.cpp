#include "Ksa64OperationsPolicy.h"

namespace Ksa64OperationsPolicy
{
namespace
{
constexpr uint32 Sha256RoundConstants[64] = {
    0x428A2F98u, 0x71374491u, 0xB5C0FBCFu, 0xE9B5DBA5u,
    0x3956C25Bu, 0x59F111F1u, 0x923F82A4u, 0xAB1C5ED5u,
    0xD807AA98u, 0x12835B01u, 0x243185BEu, 0x550C7DC3u,
    0x72BE5D74u, 0x80DEB1FEu, 0x9BDC06A7u, 0xC19BF174u,
    0xE49B69C1u, 0xEFBE4786u, 0x0FC19DC6u, 0x240CA1CCu,
    0x2DE92C6Fu, 0x4A7484AAu, 0x5CB0A9DCu, 0x76F988DAu,
    0x983E5152u, 0xA831C66Du, 0xB00327C8u, 0xBF597FC7u,
    0xC6E00BF3u, 0xD5A79147u, 0x06CA6351u, 0x14292967u,
    0x27B70A85u, 0x2E1B2138u, 0x4D2C6DFCu, 0x53380D13u,
    0x650A7354u, 0x766A0ABBu, 0x81C2C92Eu, 0x92722C85u,
    0xA2BFE8A1u, 0xA81A664Bu, 0xC24B8B70u, 0xC76C51A3u,
    0xD192E819u, 0xD6990624u, 0xF40E3585u, 0x106AA070u,
    0x19A4C116u, 0x1E376C08u, 0x2748774Cu, 0x34B0BCB5u,
    0x391C0CB3u, 0x4ED8AA4Au, 0x5B9CCA4Fu, 0x682E6FF3u,
    0x748F82EEu, 0x78A5636Fu, 0x84C87814u, 0x8CC70208u,
    0x90BEFFFAu, 0xA4506CEBu, 0xBEF9A3F7u, 0xC67178F2u,
};

FORCEINLINE uint32 Sha256RotateRight(uint32 Value, uint32 Shift)
{
    return (Value >> Shift) | (Value << (32u - Shift));
}

void Sha256Transform(uint32 State[8], const uint8 Block[64])
{
    uint32 Words[64] = {};
    for (uint32 Index = 0; Index < 16; ++Index)
    {
        const uint32 Offset = Index * 4;
        Words[Index] = (static_cast<uint32>(Block[Offset]) << 24)
            | (static_cast<uint32>(Block[Offset + 1]) << 16)
            | (static_cast<uint32>(Block[Offset + 2]) << 8)
            | static_cast<uint32>(Block[Offset + 3]);
    }
    for (uint32 Index = 16; Index < 64; ++Index)
    {
        const uint32 Sigma0 = Sha256RotateRight(Words[Index - 15], 7)
            ^ Sha256RotateRight(Words[Index - 15], 18)
            ^ (Words[Index - 15] >> 3);
        const uint32 Sigma1 = Sha256RotateRight(Words[Index - 2], 17)
            ^ Sha256RotateRight(Words[Index - 2], 19)
            ^ (Words[Index - 2] >> 10);
        Words[Index] = Words[Index - 16] + Sigma0 + Words[Index - 7] + Sigma1;
    }

    uint32 A = State[0];
    uint32 B = State[1];
    uint32 C = State[2];
    uint32 D = State[3];
    uint32 E = State[4];
    uint32 F = State[5];
    uint32 G = State[6];
    uint32 H = State[7];
    for (uint32 Index = 0; Index < 64; ++Index)
    {
        const uint32 UpperSigma1 = Sha256RotateRight(E, 6)
            ^ Sha256RotateRight(E, 11)
            ^ Sha256RotateRight(E, 25);
        const uint32 Choice = (E & F) ^ (~E & G);
        const uint32 Temporary1 = H + UpperSigma1 + Choice
            + Sha256RoundConstants[Index] + Words[Index];
        const uint32 UpperSigma0 = Sha256RotateRight(A, 2)
            ^ Sha256RotateRight(A, 13)
            ^ Sha256RotateRight(A, 22);
        const uint32 Majority = (A & B) ^ (A & C) ^ (B & C);
        const uint32 Temporary2 = UpperSigma0 + Majority;
        H = G;
        G = F;
        F = E;
        E = D + Temporary1;
        D = C;
        C = B;
        B = A;
        A = Temporary1 + Temporary2;
    }
    State[0] += A;
    State[1] += B;
    State[2] += C;
    State[3] += D;
    State[4] += E;
    State[5] += F;
    State[6] += G;
    State[7] += H;
}
}

FString Sha256Hex(const uint8* Data, uint64 Length)
{
    if (Data == nullptr && Length != 0)
    {
        return {};
    }
    uint32 State[8] = {
        0x6A09E667u, 0xBB67AE85u, 0x3C6EF372u, 0xA54FF53Au,
        0x510E527Fu, 0x9B05688Cu, 0x1F83D9ABu, 0x5BE0CD19u,
    };
    uint64 Offset = 0;
    while (Length - Offset >= 64)
    {
        Sha256Transform(State, Data + Offset);
        Offset += 64;
    }

    uint8 Tail[128] = {};
    const uint32 Remaining = static_cast<uint32>(Length - Offset);
    if (Remaining != 0)
    {
        FMemory::Memcpy(Tail, Data + Offset, Remaining);
    }
    Tail[Remaining] = 0x80;
    const uint32 PaddingLength = Remaining < 56 ? 64 : 128;
    const uint64 BitLength = Length * 8;
    for (uint32 Index = 0; Index < 8; ++Index)
    {
        Tail[PaddingLength - 1 - Index] = static_cast<uint8>(BitLength >> (Index * 8));
    }
    Sha256Transform(State, Tail);
    if (PaddingLength == 128)
    {
        Sha256Transform(State, Tail + 64);
    }

    static constexpr TCHAR Digits[] = TEXT("0123456789abcdef");
    FString Output;
    Output.Reserve(64);
    for (const uint32 Word : State)
    {
        for (int32 Shift = 28; Shift >= 0; Shift -= 4)
        {
            Output.AppendChar(Digits[(Word >> Shift) & 0x0Fu]);
        }
    }
    return Output;
}

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

int64 NearestRankP99Nanoseconds(const TArray<int64>& Samples)
{
    if (Samples.IsEmpty())
    {
        return -1;
    }
    TArray<int64> Ordered = Samples;
    Ordered.Sort();
    const int64 Rank = (static_cast<int64>(Ordered.Num()) * 99 + 99) / 100;
    return Ordered[static_cast<int32>(Rank - 1)];
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
    BaselineReleaseEpoch = 0;
    bOutstanding = false;
}

void FKsa64OperationsAdvanceTracker::MarkAccepted(
    uint64 PublicationSequence,
    uint32 InBaselineReleaseEpoch)
{
    BaselinePublicationSequence = PublicationSequence;
    BaselineReleaseEpoch = InBaselineReleaseEpoch;
    bOutstanding = true;
}

bool FKsa64OperationsAdvanceTracker::Observe(
    uint64 PublicationSequence,
    uint32 ReleaseEpoch,
    uint32 CommandsPending,
    uint32 Lifecycle)
{
    if (!bOutstanding)
    {
        return false;
    }
    const bool bTerminal = Lifecycle == 5 || Lifecycle == 6;
    const bool bAdvanceCompletionPublished =
        ReleaseEpoch > BaselineReleaseEpoch
        && CommandsPending == 0
        && PublicationSequence != BaselinePublicationSequence;
    if (bTerminal || bAdvanceCompletionPublished)
    {
        BaselineReleaseEpoch = 0;
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
