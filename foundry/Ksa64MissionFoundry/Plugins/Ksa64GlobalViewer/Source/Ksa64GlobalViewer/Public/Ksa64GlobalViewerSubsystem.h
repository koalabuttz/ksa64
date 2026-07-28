#pragma once

#include "CoreMinimal.h"
#include "Containers/Ticker.h"
#include "Subsystems/GameInstanceSubsystem.h"
#include "Ksa64GlobalViewerTypes.h"
#include "Ksa64GlobalDisplayCodec.h"
#include "Ksa64GlobalViewerSubsystem.generated.h"

class AActor;
class ACameraActor;
class SWidget;
class UStaticMeshComponent;
class UKsa64GlobalLineComponent;
class UKsa64LiveMissionSubsystem;

struct FKsa64GlobalEvidenceCapture
{
    FString Label;
    FString ScreenshotPath;
    FString SemanticPath;
    uint32 ReleaseEpoch = 0;
    uint32 FrameIdentity = 0;
    uint32 SegmentIdentity = 0;
    uint32 SourceMask = 0;
    uint32 TransitionMarkers = 0;
    uint32 PlannedPathPoints = 0;
    uint32 OnboardPathPoints = 0;
    uint32 ObservedPathPoints = 0;
    int32 Width = 0;
    int32 Height = 0;
    int32 SampledPixels = 0;
    int32 DistinctColorBuckets = 0;
    int32 LuminanceRange = 0;
    int32 NonDarkSamples = 0;
};

struct FKsa64GlobalGuidedEvidenceRecord
{
    FString Label;
    FString SemanticPath;
    uint32 ReleaseEpoch = 0;
    uint32 FrameIdentity = 0;
    uint32 SegmentIdentity = 0;
    uint32 SourceMask = 0;
    bool bTruthPermitted = false;
    bool bTruthVisible = false;
    uint32 GnssState = 0;
    uint64 ActionReceiptSequence = 0;
    uint32 ActionReceiptState = 0;
    uint32 ActionReceiptAccepted = 0;
    uint32 ActionProposalIdentity = 0;
    uint32 OverallDisposition = 0;
    uint32 ObjectiveDisposition = 0;
    uint32 VehicleDisposition = 0;
    uint32 ProcedureDisposition = 0;
    uint32 OperatorDisposition = 0;
    uint32 AvionicsDisposition = 0;
    uint32 EvidenceDisposition = 0;
};

/**
 * Passive Unreal consumer of Ksa64Operations. It creates renderer-local scene
 * state and never opens the bridge, parses evidence, or advances authority.
 */
UCLASS()
class KSA64GLOBALVIEWER_API UKsa64GlobalViewerSubsystem final
    : public UGameInstanceSubsystem
{
    GENERATED_BODY()

public:
    virtual void Initialize(FSubsystemCollectionBase& Collection) override;
    virtual void Deinitialize() override;

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    bool StartGuidedOperations();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    bool StartNominalReplay();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void CycleLayout();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void CycleCamera();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void ResumeAutomaticDirector();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void ToggleOperationsDesk();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void ToggleTruth();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void TogglePause();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void StepOneRelease();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void CycleReplayPace();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void JumpToPreviousBookmark();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void JumpToNextBookmark();

    void SetLayout(EKsa64GlobalViewerLayout Layout);
    void SetCamera(EKsa64GlobalCameraMode Camera);
    const FKsa64GlobalSemanticState& GetSemanticState() const { return SemanticState; }
    FString ExportSemanticStateJson() const;
    FText GetStatusText() const;
    FText GetCameraText() const;
    FText GetLayoutText() const;
    FText GetSourceLegendText() const;
    FText GetPaceText() const;
    bool IsNominalReplay() const
    {
        return SemanticState.ExperienceMode == EKsa64GlobalExperienceMode::NominalReplay;
    }
    bool CanShowTruth() const { return SemanticState.bTruthPermitted; }

#if WITH_DEV_AUTOMATION_TESTS
    void ApplySampleForAutomation(
        const FKsa64GlobalSceneSample& Sample,
        bool bTruthPermitted);
    void SetSceneReadyForAutomation(bool bReady);
    void ApplyReplayIndexForAutomation(const FKsa64GlobalReplayIndexProduct& Replay);
    bool OpenNominalReleaseForAutomation(
        UKsa64LiveMissionSubsystem& Operations,
        uint32 ReleaseEpoch);
    void SetGlobalAvailabilityForAutomation(
        bool bDefinitionValid,
        bool bAcceptedExact,
        uint32 SourceMask);
#endif

private:
    bool Tick(float DeltaSeconds);
    UKsa64LiveMissionSubsystem* GetOperations() const;
    void InstallOverlayIfPossible();
    bool EnsureScene();
    void DestroyScene();
    void ObserveOperations(float DeltaSeconds);
    bool ObserveGlobalDisplay(
        UKsa64LiveMissionSubsystem& Operations,
        float DeltaSeconds);
    void ResetGlobalDisplayState();
    void AdvanceReplayPresentation(float DeltaSeconds);
    bool ReadReplaySample(
        UKsa64LiveMissionSubsystem& Operations,
        float DeltaSeconds);
    void SeekReplayRelease(uint32 ReleaseEpoch);
    bool InitializeGlobalDisplay(UKsa64LiveMissionSubsystem& Operations);
    void ApplyGlobalSample(
        const FKsa64GlobalDisplaySampleProduct& Product,
        const UKsa64LiveMissionSubsystem& Operations,
        float DeltaSeconds);
    void RefreshGlobalPaths(UKsa64LiveMissionSubsystem& Operations);
    void ApplyAcceptedEarthDefinition();
    const FKsa64GlobalSourcePoseProduct* FindGlobalSource(uint8 Source) const;
    const FKsa64GlobalResolvedPoseProduct* ResolvePoseForCamera(
        const FKsa64GlobalSourcePoseProduct& Source) const;
    void ApplyReplayDisposition();
    void RefreshSemanticState(
        const FKsa64GlobalSceneSample& Sample,
        const UKsa64LiveMissionSubsystem& Operations);
    void UpdateDisplayOrigin(const FKsa64GlobalSceneSample& Sample);
    void UpdateScene(const FKsa64GlobalSceneSample& Sample);
    void UpdateEarthAndLocalDomain(uint32 FrameIdentity);
    void UpdateVehicle(const FKsa64GlobalSceneSample& Sample);
    void UpdatePaths(uint32 ActiveFrame);
    void UpdateCamera(const FKsa64GlobalSceneSample& Sample, float DeltaSeconds);
    void BuildEarthGrid();
    void BuildLocalGrid();
    void ApplyOriginToStaticDomain();
    void TickGlobalEvidence(float DeltaSeconds);
    bool PrepareGlobalEvidence();
    bool ValidateGlobalEvidenceState(
        uint32 ReleaseEpoch,
        uint32 FrameIdentity,
        uint32 SegmentIdentity,
        FString& OutReason) const;
    bool WriteGlobalEvidenceSemanticAndRequestScreenshot();
    bool QueueGlobalEvidenceGuidedAdvance(
        UKsa64LiveMissionSubsystem& Operations,
        uint32 TargetRelease);
    bool WriteGlobalEvidenceGuidedRecord(
        const FString& Label,
        uint32 ExpectedGnssState,
        uint32 ExpectedReceiptState);
    void OnGlobalEvidenceScreenshotProcessed();
    bool ValidateGlobalEvidenceScreenshot(FKsa64GlobalEvidenceCapture& Capture);
    bool WriteGlobalEvidenceManifest(
        bool bPassed,
        const FString& FailureReason = FString());
    void FailGlobalEvidence(const FString& Reason);
    void ExitGlobalEvidenceFailure();

    UStaticMeshComponent* CreateMeshComponent(
        AActor& Owner,
        const TCHAR* Name,
        const TCHAR* MeshPath,
        const FLinearColor& Color);
    UKsa64GlobalLineComponent* CreateLineComponent(
        AActor& Owner,
        const TCHAR* Name,
        const FLinearColor& Color,
        float Thickness);

    TWeakObjectPtr<AActor> SceneRootActor;
    TWeakObjectPtr<ACameraActor> ViewerCamera;
    TWeakObjectPtr<UStaticMeshComponent> EarthMesh;
    TWeakObjectPtr<UStaticMeshComponent> AtmosphereMesh;
    TWeakObjectPtr<UStaticMeshComponent> VehicleBodyMesh;
    TWeakObjectPtr<UStaticMeshComponent> VehicleNoseMesh;
    TWeakObjectPtr<UStaticMeshComponent> LocatorMesh;
    TWeakObjectPtr<UStaticMeshComponent> GroundLocatorMesh;
    TWeakObjectPtr<UStaticMeshComponent> TruthLocatorMesh;
    TWeakObjectPtr<UKsa64GlobalLineComponent> EarthGridLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> LocalGridLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> AxisXLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> AxisYLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> AxisZLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> ObservedPathLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> PlannedPathLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> OnboardPathLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> GroundPathLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> TransitionMarkerLines;
    TSharedPtr<SWidget> Overlay;
    FTSTicker::FDelegateHandle TickerHandle;
    FKsa64GlobalSemanticState SemanticState;
    FKsa64GlobalSceneSample PreviousSample;
    FKsa64GlobalSceneSample CurrentSample;
    FKsa64GlobalDisplayDefinitionProduct GlobalDefinition;
    FKsa64GlobalDisplaySampleProduct CurrentGlobalProduct;
    FKsa64GlobalReplayIndexProduct GlobalReplayIndex;
    TArray<FKsa64GlobalTransitionProduct> GlobalTransitions;
    TArray<FKsa64GlobalPathPointProduct> GlobalPaths[4];
    uint32 PermittedGlobalSourceMask = 0;
    uint8 GlobalPathDisplayFrame = 0;
    uint32 LastGlobalPathRefreshRelease = 0;
    uint32 ReplayOldestRelease = 0;
    uint32 ReplayNewestRelease = 0;
    uint32 ReplaySelectedRelease = 0;
    uint32 ReplayLastReadRelease = MAX_uint32;
    EKsa64GlobalReplayPace ReplayPace = EKsa64GlobalReplayPace::Paused;
    double ReplayReleaseAccumulator = 0.0;
    bool bReplaySeekSnapPending = false;
    double EarthSemiMajorCentimetres = 6378.137 * 100000.0;
    double EarthSemiMinorCentimetres = 6356.752314245 * 100000.0;
    FVector3d LastCameraLocation = FVector3d::ZeroVector;
    bool bHasPreviousSample = false;
    bool bGlobalDefinitionValid = false;
    bool bGlobalAcceptedExact = false;
    bool bOverlayInstalled = false;
    bool bSceneAttempted = false;
    bool bOperationsDeskVisible = false;
    bool bTruthRequested = false;
    double LastSceneSampleWallSeconds = 0.0;
    bool bGlobalEvidenceMode = false;
    bool bGlobalEvidenceFailed = false;
    bool bGlobalEvidenceExitRequested = false;
    bool bGlobalEvidenceSlowWarningEmitted = false;
    bool bGlobalEvidenceScreenshotProcessed = false;
    bool bGlobalEvidencePrepared = false;
    uint8 GlobalEvidencePhase = 0;
    uint32 GlobalEvidenceMilestoneIndex = 0;
    uint32 GlobalEvidenceGuidedIndex = 0;
    uint32 GlobalEvidenceAcceptedActions = 0;
    uint64 GlobalEvidenceReceiptSequenceBeforeAction = 0;
    uint32 GlobalEvidenceExpectedProposalIdentity = 0;
    bool bGlobalEvidenceGuidedActionOutstanding = false;
    uint32 GlobalEvidenceWarmupFrames = 0;
    uint32 GlobalEvidenceMeasuredFrames = 0;
    uint32 GlobalEvidenceScreenshotWaitFrames = 0;
    uint32 GlobalEvidenceReadyWaitFrames = 0;
    uint32 GlobalEvidencePerformanceStartRelease = 0;
    uint32 GlobalEvidencePerformanceEndRelease = 0;
    int64 GlobalEvidenceP99Nanoseconds = -1;
    int64 GlobalEvidenceMaximumNanoseconds = -1;
    double GlobalEvidenceStartedSeconds = 0.0;
    double GlobalEvidenceMeasurementStartedSeconds = 0.0;
    double GlobalEvidenceMeasurementEndedSeconds = 0.0;
    double GlobalEvidenceActualRenderFramesPerSecond = 0.0;
    FString GlobalEvidenceSourceCommit;
    FString GlobalEvidenceExecutableRelativePath;
    uint64 GlobalEvidenceExecutableBytes = 0;
    FString GlobalEvidenceExecutableSha256;
    FString GlobalEvidencePackageAuditSha256;
    FString GlobalEvidenceFailureReason;
    FString GlobalEvidenceDirectory;
    FString GlobalEvidenceManifestPath;
    FString GlobalEvidenceRhiName;
    FDelegateHandle GlobalEvidenceScreenshotProcessedHandle;
    TArray<int64> GlobalEvidenceServiceNanoseconds;
    TArray<FKsa64GlobalEvidenceCapture> GlobalEvidenceCaptures;
    TArray<FKsa64GlobalGuidedEvidenceRecord> GlobalEvidenceGuidedRecords;
};
