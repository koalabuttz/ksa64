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
    void RequestShutdown();
    bool SaveCompletedEvidence();

    const FKsa64OperationsViewModel& GetViewModel() const { return ViewModel; }
    const TArray<FKsa64OperationsReleasePoint>& GetReleaseHistory() const { return ReleaseHistory; }
    const TArray<FKsa64OperationsTimelineItem>& GetTimeline() const { return Timeline; }
    const TArray<FKsa64OperationsPredictionPoint>& GetPredictionPath() const { return PredictionPath; }
    const FKsa64OperationsAccessibilitySettings& GetAccessibility() const
    {
        return Accessibility;
    }

    void ToggleReducedMotion();
    void ToggleHighContrast();
    void ToggleSoundCues();
    void CycleTextScale();

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

    TUniquePtr<IKsa64OperationsBridgeAdapter> Bridge;
    FKsa64OperationsViewModel ViewModel;
    TArray<FKsa64OperationsReleasePoint> ReleaseHistory;
    TArray<FKsa64OperationsTimelineItem> Timeline;
    TArray<FKsa64OperationsPredictionPoint> PredictionPath;
    FKsa64OperationsAccessibilitySettings Accessibility;
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
    double AcceptanceStartedSeconds = 0.0;
};

