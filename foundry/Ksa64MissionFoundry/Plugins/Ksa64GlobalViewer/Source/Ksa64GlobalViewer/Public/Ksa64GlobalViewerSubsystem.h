#pragma once

#include "CoreMinimal.h"
#include "Containers/Ticker.h"
#include "Subsystems/GameInstanceSubsystem.h"
#include "Ksa64GlobalViewerTypes.h"
#include "Ksa64GlobalViewerSubsystem.generated.h"

class AActor;
class ACameraActor;
class SWidget;
class UStaticMeshComponent;
class UKsa64GlobalLineComponent;
class UKsa64LiveMissionSubsystem;

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
    void CycleLayout();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void CycleCamera();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void ResumeAutomaticDirector();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void ToggleOperationsDesk();

    UFUNCTION(BlueprintCallable, Category = "KSA64|Global Viewer")
    void ToggleTruth();

    void SetLayout(EKsa64GlobalViewerLayout Layout);
    void SetCamera(EKsa64GlobalCameraMode Camera);
    const FKsa64GlobalSemanticState& GetSemanticState() const { return SemanticState; }
    FString ExportSemanticStateJson() const;
    FText GetStatusText() const;
    FText GetCameraText() const;
    FText GetLayoutText() const;
    FText GetSourceLegendText() const;
    bool CanShowTruth() const { return SemanticState.bTruthPermitted; }

#if WITH_DEV_AUTOMATION_TESTS
    void ApplySampleForAutomation(
        const FKsa64GlobalSceneSample& Sample,
        bool bTruthPermitted);
    void SetSceneReadyForAutomation(bool bReady);
#endif

private:
    bool Tick(float DeltaSeconds);
    UKsa64LiveMissionSubsystem* GetOperations() const;
    void InstallOverlayIfPossible();
    bool EnsureScene();
    void DestroyScene();
    void ObserveOperations(float DeltaSeconds);
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
    TWeakObjectPtr<UKsa64GlobalLineComponent> EarthGridLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> LocalGridLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> AxisXLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> AxisYLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> AxisZLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> ObservedPathLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> PlannedPathLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> OnboardPathLines;
    TWeakObjectPtr<UKsa64GlobalLineComponent> GroundPathLines;
    TSharedPtr<SWidget> Overlay;
    FTSTicker::FDelegateHandle TickerHandle;
    FKsa64GlobalSemanticState SemanticState;
    FKsa64GlobalSceneSample PreviousSample;
    FKsa64GlobalSceneSample CurrentSample;
    FVector3d LastCameraLocation = FVector3d::ZeroVector;
    bool bHasPreviousSample = false;
    bool bOverlayInstalled = false;
    bool bSceneAttempted = false;
    bool bOperationsDeskVisible = false;
    bool bTruthRequested = false;
    double LastSceneSampleWallSeconds = 0.0;
};
