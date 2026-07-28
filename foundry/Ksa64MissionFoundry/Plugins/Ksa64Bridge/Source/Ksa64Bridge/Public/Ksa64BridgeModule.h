#pragma once

#include "CoreMinimal.h"
#include "Containers/Ticker.h"
#include "Modules/ModuleInterface.h"
#include "ksa64_viewer_bridge.h"

enum class EKsa64BridgeStatus : uint8
{
    Unavailable,
    Ready,
    SessionOpen,
    Faulted
};

struct FKsa64BridgeValidation
{
    FString ManifestPath;
    FString DllPath;
    FString DllSha256;
    FString CatalogSha256;
    FString SourceCommit;
    FString TargetTriple;
    uint32 AbiVersion = 0;
    uint32 BuildIdentity = 0;
    uint32 CatalogCount = 0;
    bool bSourceTreeClean = false;
};

/**
 * Presentation-only Phase 12A boundary. All calls enqueue work or inspect
 * immutable, role-filtered data owned by the Rust bridge.
 */
class KSA64BRIDGE_API FKsa64BridgeModule final : public IModuleInterface
{
public:
    FKsa64BridgeModule();
    virtual ~FKsa64BridgeModule() override;

    static FKsa64BridgeModule& Get();
    static bool IsAvailable();

    virtual void StartupModule() override;
    virtual void ShutdownModule() override;

    EKsa64BridgeStatus GetStatus() const { return Status; }
    const FString& GetDiagnostic() const { return Diagnostic; }
    const FString& GetCatalogJson() const { return CatalogJson; }
    const FKsa64BridgeValidation& GetValidation() const { return Validation; }
    uint32 GetFeatureFlags() const { return FeatureFlags; }
    bool SupportsFeature(uint32 Feature) const { return (FeatureFlags & Feature) == Feature; }

    bool StartGuidedGnssLoss();
    bool StartGuidedOperationsV1(
        uint32 ScenarioIdentity = KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS);
    int32 AdvanceOneRelease();
    int32 AdvanceReleases(uint32 MaximumReleases);
    int32 PollSnapshot(Ksa64ViewerSnapshot& OutSnapshot) const;
    int32 PollEvent(Ksa64ViewerEvent& OutEvent) const;
    int32 PollOperationalV1(Ksa64ViewerOperationalViewV1& OutView) const;
    int32 ProcedureV1(Ksa64ViewerProcedureViewV1& OutView) const;
    int32 DispositionV1(Ksa64ViewerDispositionV1& OutView) const;
    int32 PollTimelineV1(Ksa64ViewerTimelineEventV1& OutEvent) const;
    int32 PollReleaseSampleV1(Ksa64ViewerReleaseSampleV1& OutSample) const;
    int32 PredictionPathHeaderV1(Ksa64ViewerPredictionPathHeaderV1& OutHeader) const;
    int32 PredictionPathPointV1(uint32 PointIndex, Ksa64ViewerPredictionPathPointV1& OutPoint) const;
    int32 TrajectoryPathHeaderV1(
        uint32 Source,
        Ksa64ViewerPredictionPathHeaderV1& OutHeader) const;
    int32 TrajectoryPathPointV1(
        uint32 Source,
        uint32 PointIndex,
        Ksa64ViewerPredictionPathPointV1& OutPoint) const;
    int32 ActionProposalV1(Ksa64ViewerActionProposalV1& OutProposal) const;
    int32 SubmitActionProposalV1(uint32 ProposalIdentity, uint32 CompletedEventMask);
    int32 CommitActionV1(uint32 ProposalIdentity);
    int32 CancelActionV1(uint32 ProposalIdentity);
    int32 PollActionReceiptV1(Ksa64ViewerActionReceiptV1& OutReceipt) const;
    int32 TransportStatusV1(Ksa64ViewerTransportStatusV1& OutStatus) const;
    int32 FinishStatusV1(Ksa64ViewerFinishStatusV1& OutStatus) const;
    int32 RequestShutdownV1();
    bool RequestAsyncClose();
    bool IsAsyncClosePending() const { return bAsyncClosePending; }
    int32 GetCompletedKsb11(TArray<uint8>& OutBytes) const;
    void CloseSession();

    /** Validation-only helper used by automation before any DLL is loaded. */
    static bool ValidateArtifactManifest(
        const FString& ManifestPath,
        FKsa64BridgeValidation& OutValidation,
        FString& OutDiagnostic);

private:
    struct FApi;

    bool LoadBridge();
    bool LoadAndCheckCatalog();
    FString ReadLibraryDiagnostic() const;
    void UnloadBridge();
    void SetFault(const FString& Message);
    bool TickAsyncClose(float DeltaSeconds);

    TUniquePtr<FApi> Api;
    void* DllHandle = nullptr;
    Ksa64ViewerHandle* Session = nullptr;
    EKsa64BridgeStatus Status = EKsa64BridgeStatus::Unavailable;
    FString Diagnostic;
    FString CatalogJson;
    FKsa64BridgeValidation Validation;
    uint32 FeatureFlags = 0;
    uint32 ActiveTypedScenarioIdentity = 0;
    uint32 ActiveTypedAdapterIdentity = 0;
    mutable uint32 ValidatedPredictionPathIdentity = 0;
    mutable uint32 ValidatedPredictionPointCount = 0;
    mutable uint32 ValidatedTrajectoryPathIdentities[4] = {};
    mutable uint32 ValidatedTrajectoryPointCounts[4] = {};
    FTSTicker::FDelegateHandle AsyncCloseTickerHandle;
    bool bAsyncClosePending = false;
};
