#pragma once

#include "CoreMinimal.h"
#include "Ksa64OperationsTypes.h"

namespace Ksa64OperationsPolicy
{
constexpr uint32 MaximumAdvanceReleases = 64;
constexpr int64 MaximumWallDebtNanoseconds = 2'000'000'000;
constexpr uint32 SimDirectorRole = 5;

KSA64OPERATIONS_API bool IsTruthFilteredRole(uint32 Role);
KSA64OPERATIONS_API FString RoleLabel(uint32 Role);
KSA64OPERATIONS_API FString LifecycleLabel(uint32 Value);
KSA64OPERATIONS_API FString FrameLabel(uint32 Value);
KSA64OPERATIONS_API FString ProcedureStateLabel(uint32 Value);
KSA64OPERATIONS_API FString GnssLabel(uint32 Value);
KSA64OPERATIONS_API FString OverallLabel(uint32 Value);
KSA64OPERATIONS_API FString ObjectiveLabel(uint32 Value);
KSA64OPERATIONS_API FString VehicleLabel(uint32 Value);
KSA64OPERATIONS_API FString ProcedureDispositionLabel(uint32 Value);
KSA64OPERATIONS_API FString OperatorLabel(uint32 Value);
KSA64OPERATIONS_API FString AvionicsLabel(uint32 Value);
KSA64OPERATIONS_API FString EvidenceLabel(uint32 Value);
KSA64OPERATIONS_API FString TimelineSourceLabel(uint32 Value);
KSA64OPERATIONS_API EKsa64OperationsActionState ActionStateFromReceipt(uint32 State);

class KSA64OPERATIONS_API FKsa64OperationsPacingController
{
public:
    void Reset();
    void Accumulate(float DeltaSeconds, EKsa64OperationsPace Pace);
    void AccumulateNanoseconds(int64 FrameNanoseconds, EKsa64OperationsPace Pace);
    uint32 ReleasesDue(
        EKsa64OperationsPace Pace,
        uint32 ReleasePeriodMicros,
        bool bSessionRunnable,
        bool bAdvanceOutstanding) const;
    void CommitAcceptedAdvance(uint32 Releases, uint32 ReleasePeriodMicros, EKsa64OperationsPace Pace);
    int64 DebtNanoseconds() const { return WallDebtNanoseconds; }

private:
    int64 WallDebtNanoseconds = 0;
};

class KSA64OPERATIONS_API FKsa64OperationsAdvanceTracker
{
public:
    void Reset();
    bool IsOutstanding() const { return bOutstanding; }
    void MarkAccepted(uint64 PublicationSequence);
    bool Observe(uint64 PublicationSequence, uint32 CommandsPending, uint32 Lifecycle);

private:
    uint64 BaselinePublicationSequence = 0;
    bool bOutstanding = false;
};

class KSA64OPERATIONS_API FKsa64OperationsActionGate
{
public:
    void Reset();
    void ObserveProposal(uint32 ProposalIdentity, uint32 ExpiresEpoch);
    void ObserveReceipt(uint32 ProposalIdentity, uint32 ReceiptState, bool bAccepted);
    void Expire(uint32 ReleaseEpoch);
    bool Review(uint32 ReleaseEpoch);
    bool CanStage(uint32 ReleaseEpoch) const;
    bool CanCommit(uint32 ReleaseEpoch) const;
    bool CanCancel(uint32 ReleaseEpoch) const;
    uint32 ProposalIdentity() const { return CurrentProposalIdentity; }
    bool IsReviewed() const { return bReviewed; }
    uint32 ReceiptState() const { return CurrentReceiptState; }

private:
    bool IsCurrent(uint32 ReleaseEpoch) const;
    uint32 CurrentProposalIdentity = 0;
    uint32 CurrentExpiresEpoch = 0;
    uint32 CurrentReceiptState = 0;
    bool bReceiptAccepted = false;
    bool bReviewed = false;
};

enum class EKsa64OperationsEvidenceReadiness : uint8
{
    InProgress,
    Complete,
    Failed
};

KSA64OPERATIONS_API EKsa64OperationsEvidenceReadiness ClassifyEvidenceReadiness(
    uint32 Lifecycle,
    uint32 FinalizationState,
    uint32 WorkerState,
    uint64 EvidenceLength,
    uint64 EvidenceValidityMask);

KSA64OPERATIONS_API void MergeReleaseSamples(
    TArray<FKsa64OperationsReleasePoint>& History,
    const TArray<FKsa64OperationsReleasePoint>& Incoming,
    int32 MaximumPoints,
    bool& bObservationComplete);
}
