#pragma once

#include "CoreMinimal.h"
#include "Containers/Ticker.h"
#include "Subsystems/GameInstanceSubsystem.h"
#include "Ksa64OperationsTypes.h"
#include "Ksa64OperationsBridgeAdapter.h"
#include "Ksa64OperationsPolicy.h"
#include "Ksa64LiveMissionSubsystem.generated.h"

class IKsa64OperationsBridgeAdapter;
class SWidget;

/**
 * Sole Mission Foundry consumer of Ksa64Bridge. It controls presentation
 * pacing and publishes immutable view models; it owns no mission authority.
 */
UCLASS()
class KSA64OPERATIONS_API UKsa64LiveMissionSubsystem final : public UGameInstanceSubsystem
{
    GENERATED_BODY()

public:
    virtual void Initialize(FSubsystemCollectionBase& Collection) override;
    virtual void Deinitialize() override;

    UFUNCTION(BlueprintCallable, Category = "KSA64|Operations")
    bool StartGuidedOperations();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Operations")
    void PausePresentation();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Operations")
    void ResumeRealtime();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Operations")
    void StepOneRelease();

    void SetPace(EKsa64OperationsPace Pace);
    void ReviewAction();
    void StageAction();
    void CommitAction();
    void CancelAction();
    bool RequestShutdown();
    bool SaveCompletedEvidence();

    const FKsa64OperationsViewModel& GetViewModel() const { return ViewModel; }
    const TArray<FKsa64OperationsReleasePoint>& GetReleaseHistory() const { return ReleaseHistory; }
    const TArray<FKsa64OperationsTimelineItem>& GetTimeline() const { return Timeline; }
    const TArray<FKsa64OperationsPredictionPoint>& GetPredictionPath() const { return GroundPredictionPath; }
    const TArray<FKsa64OperationsPredictionPoint>& GetPlannedReferencePath() const { return PlannedReferencePath; }
    const TArray<FKsa64OperationsPredictionPoint>& GetOnboardPredictionPath() const { return OnboardPredictionPath; }
    const TArray<FKsa64OperationsPredictionPoint>& GetGroundPredictionPath() const { return GroundPredictionPath; }
    EKsa64OperationsDisplayMode GetDisplayMode() const;
    bool GetVisualObservedPoint(FKsa64OperationsReleasePoint& OutPoint) const;
    const FKsa64OperationsAccessibilitySettings& GetAccessibility() const
    {
        return Accessibility;
    }

    void ToggleReducedMotion();
    void ToggleHighContrast();
    void ToggleSoundCues();
    void CycleTextScale();
    void ToggleDisplayMode();

#if WITH_DEV_AUTOMATION_TESTS
    bool InitializeForAutomation();
    bool AdvanceToReleaseForAutomation(uint32 TargetRelease, double TimeoutSeconds);
    bool WaitForActionReceiptForAutomation(uint32 ExpectedState, double TimeoutSeconds);
    bool WaitForCompletionForAutomation(double TimeoutSeconds);
    bool CopyCompletedEvidenceForAutomation(TArray<uint8>& OutBytes) const;
    void CloseForAutomation();
#endif

    FString ExportSemanticStateJson() const;
    FText GetPaceLabel() const;
    FText GetMissionElapsedLabel() const;
    FText GetReleaseLabel() const;

private:
    bool Tick(float DeltaSeconds);
    void InstallDashboardIfPossible();
    void PollBridge();
    void ObserveSnapshot(const FKsa64OperationsViewModel& Previous);
    void AppendTimeline(
        const FString& Category,
        const FString& Summary,
        bool bAttention = false);
    void HandleAdapterResult(EKsa64OperationsAdapterResult Result, const TCHAR* Operation);
    void EmitProceduralCue(const TCHAR* CueName);
    void ObserveCompletionAndShutdown();
    void TickAcceptance();
    bool QueueAcceptanceAdvance(uint32 TargetRelease);
    void FailAcceptance(const FString& Reason);
    void ExitAcceptanceFailure();
    void TickPresentationEvidence(float DeltaSeconds);
    bool QueuePresentationEvidenceAdvance(uint32 TargetRelease);
    bool WritePresentationSemanticAndRequestScreenshot();
    void OnPresentationScreenshotProcessed();
    bool ValidatePresentationScreenshot(int32& OutWidth, int32& OutHeight);
    bool WritePresentationManifest(bool bPassed, const FString& FailureReason = FString());
    void FailPresentationEvidence(const FString& Reason);
    void ExitPresentationEvidenceFailure();

    TUniquePtr<IKsa64OperationsBridgeAdapter> Bridge;
    FKsa64OperationsViewModel ViewModel;
    TArray<FKsa64OperationsReleasePoint> ReleaseHistory;
    TArray<FKsa64OperationsTimelineItem> Timeline;
    TArray<FKsa64OperationsPredictionPoint> PlannedReferencePath;
    TArray<FKsa64OperationsPredictionPoint> OnboardPredictionPath;
    TArray<FKsa64OperationsPredictionPoint> GroundPredictionPath;
    FKsa64OperationsAccessibilitySettings Accessibility;
    EKsa64OperationsDisplayMode DisplayMode = EKsa64OperationsDisplayMode::Smooth;
    double LastVisualSampleSeconds = 0.0;
    double VisualSnapUntilSeconds = 0.0;
    TSharedPtr<SWidget> Dashboard;
    FTSTicker::FDelegateHandle TickerHandle;
    Ksa64OperationsPolicy::FKsa64OperationsPacingController PacingController;
    Ksa64OperationsPolicy::FKsa64OperationsAdvanceTracker AdvanceTracker;
    uint32 LastObservedRelease = 0;
    uint64 LastObservedCommandSequence = 0;
    bool bDashboardInstalled = false;
    bool bEvidenceSaved = false;
    bool bAcceptanceMode = false;
    bool bAcceptanceVerified = false;
    bool bAcceptanceExitRequested = false;
    bool bAcceptanceFailed = false;
    FString AcceptanceFailureReason;
    uint8 AcceptancePhase = 0;
    uint64 AcceptanceReceiptSequenceBeforeCommand = 0;
    uint32 AcceptanceExpectedProposalIdentity = 0;
    double AcceptanceStartedSeconds = 0.0;
    bool bAcceptanceSlowWarningEmitted = false;
    bool bPresentationEvidenceMode = false;
    bool bPresentationEvidenceFailed = false;
    bool bPresentationEvidenceExitRequested = false;
    bool bPresentationEvidenceSlowWarningEmitted = false;
    uint8 PresentationEvidencePhase = 0;
    uint32 PresentationEvidenceWarmupFrames = 0;
    uint32 PresentationEvidenceMeasuredFrames = 0;
    bool bPresentationMeasurementFramesComplete = false;
    uint32 PresentationDashboardWaitFrames = 0;
    uint32 PresentationScreenshotWaitFrames = 0;
    uint32 PresentationPerformanceStartRelease = 0;
    uint32 PresentationPerformanceEndRelease = 0;
    uint64 PresentationPerformanceStartPublication = 0;
    uint64 PresentationPerformanceEndPublication = 0;
    int64 PresentationEvidenceP99Nanoseconds = -1;
    int64 PresentationEvidenceMaximumNanoseconds = -1;
    int32 PresentationScreenshotSampledPixels = 0;
    int32 PresentationScreenshotDistinctColorBuckets = 0;
    int32 PresentationScreenshotLuminanceRange = 0;
    int32 PresentationScreenshotNonDarkSamples = 0;
    double PresentationEvidenceStartedSeconds = 0.0;
    FString PresentationEvidenceFailureReason;
    FString PresentationEvidenceDirectory;
    FString PresentationScreenshotPath;
    FString PresentationSemanticPath;
    FString PresentationManifestPath;
    FString PresentationRhiName;
    FDelegateHandle PresentationScreenshotProcessedHandle;
    bool bPresentationScreenshotProcessed = false;
    TArray<int64> PresentationEvidenceServiceNanoseconds;
};

