#pragma once

#include "CoreMinimal.h"
#include "Ksa64OperationsTypes.h"
#include "ksa64_viewer_bridge.h"

enum class EKsa64OperationsAdapterResult : uint8
{
    Ok,
    Queued,
    NoData,
    Unchanged,
    Unsupported,
    QueueFull,
    Lifecycle,
    Failed
};

/**
 * Local seam between Mission Foundry presentation and the qualified bridge.
 * Phase 12B additive symbols can be wired here without allowing widgets to
 * depend on an ABI header or parse canonical mission records.
 */
class KSA64OPERATIONS_API IKsa64OperationsBridgeAdapter
{
public:
    virtual ~IKsa64OperationsBridgeAdapter() = default;

    static TUniquePtr<IKsa64OperationsBridgeAdapter> Create();

    virtual bool IsReady() const = 0;
    virtual FString GetDiagnostic() const = 0;
    virtual FKsa64OperationsBridgeCapabilities GetCapabilities() const = 0;

    virtual bool StartGuidedOperations() = 0;
    virtual void Close() = 0;
    virtual EKsa64OperationsAdapterResult AdvanceOneRelease() = 0;
    virtual EKsa64OperationsAdapterResult AdvanceReleases(uint32 MaximumReleases) = 0;
    virtual EKsa64OperationsAdapterResult Poll(FKsa64OperationsViewModel& OutView) = 0;
    virtual void DrainTimeline(TArray<FKsa64OperationsTimelineItem>& OutItems) = 0;
    virtual void DrainReleaseSamples(TArray<FKsa64OperationsReleasePoint>& OutSamples) = 0;
    virtual void ReadPredictionPath(TArray<FKsa64OperationsPredictionPoint>& OutPoints) = 0;
    virtual void ReadTrajectoryPath(
        EKsa64OperationsTrajectorySource Source,
        TArray<FKsa64OperationsPredictionPoint>& OutPoints) = 0;

    virtual EKsa64OperationsAdapterResult ReviewAction() = 0;
    virtual EKsa64OperationsAdapterResult StageAction() = 0;
    virtual EKsa64OperationsAdapterResult CommitAction() = 0;
    virtual EKsa64OperationsAdapterResult CancelAction() = 0;
    virtual EKsa64OperationsAdapterResult RequestShutdown() = 0;
    virtual EKsa64OperationsAdapterResult GetCompletedEvidence(TArray<uint8>& OutBytes) const = 0;

    static FKsa64OperationsViewModel MapLegacySnapshot(
        const Ksa64ViewerSnapshot& Snapshot,
        const FString& BridgeDiagnostic);
};

