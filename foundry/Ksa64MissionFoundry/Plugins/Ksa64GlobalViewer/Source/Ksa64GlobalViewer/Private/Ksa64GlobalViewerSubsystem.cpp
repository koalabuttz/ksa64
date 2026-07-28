#include "Ksa64GlobalViewerSubsystem.h"

#include "Ksa64GlobalLineComponent.h"
#include "Ksa64GlobalViewerOverlay.h"
#include "Ksa64GlobalViewerPolicy.h"
#include "Ksa64LiveMissionSubsystem.h"
#include "Ksa64OperationsPolicy.h"

#include "Camera/CameraActor.h"
#include "Camera/CameraComponent.h"
#include "Components/DirectionalLightComponent.h"
#include "Components/SceneComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/Engine.h"
#include "Engine/GameInstance.h"
#include "Engine/GameViewportClient.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "DynamicRHI.h"
#include "ImageUtils.h"
#include "HAL/PlatformFileManager.h"
#include "HAL/PlatformMisc.h"
#include "HAL/PlatformProcess.h"
#include "Framework/Application/SlateApplication.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformTime.h"
#include "Materials/Material.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Misc/App.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Misc/Parse.h"
#include "Serialization/JsonWriter.h"
#include "UnrealClient.h"

DEFINE_LOG_CATEGORY_STATIC(LogKsa64GlobalViewer, Log, All);

namespace
{
constexpr double FallbackSemiMajorCentimetres = 6378.137 * 100'000.0;
constexpr double FallbackSemiMinorCentimetres = 6356.752314245 * 100'000.0;
constexpr int32 EarthGridLatitudeSteps = 12;
constexpr int32 EarthGridLongitudeSteps = 24;
constexpr int32 EarthGridCurveSteps = 96;
constexpr int32 MaximumPathSegments = 32'768;
constexpr uint32 GlobalEvidenceWarmupFrameCount = 120;
constexpr uint32 GlobalEvidenceMeasuredFrameCount = 600;
constexpr uint32 GlobalEvidencePerformanceStart = 6'000;
constexpr uint32 GlobalEvidenceExpectedReleaseDelta = 320;
constexpr int64 GlobalEvidenceP99LimitNanoseconds = 1'000'000;
constexpr int64 GlobalEvidenceMaximumLimitNanoseconds = 2'000'000;
constexpr int32 GlobalEvidenceWidth = 1'920;
constexpr int32 GlobalEvidenceHeight = 1'080;
constexpr uint32 GlobalEvidenceReadyFrameLimit = 600;
constexpr int32 GlobalEvidenceMinimumLuminanceRange = 24;
constexpr int32 GlobalEvidenceMinimumColorBuckets = 8;

struct FKsa64GlobalEvidenceMilestone
{
    const TCHAR* Label;
    uint32 ReleaseEpoch;
    uint32 FrameIdentity;
    uint32 SegmentIdentity;
};

constexpr FKsa64GlobalEvidenceMilestone GlobalEvidenceMilestones[] = {
    {TEXT("enu-to-ecef"), 29, 2, 2},
    {TEXT("burnout"), 1'920, 2, 2},
    {TEXT("ecef-to-gcrf"), 3'579, 3, 3},
    {TEXT("apogee"), 8'124, 3, 3},
    {TEXT("gcrf-to-ecef"), 12'669, 2, 4},
    {TEXT("recovery-enu"), 15'255, 1, 5},
    {TEXT("drogue"), 15'257, 1, 5},
    {TEXT("main"), 20'929, 1, 5},
    {TEXT("landing"), 22'014, 1, 5},
};

enum class EKsa64GlobalGuidedEvidenceAction : uint8
{
    None = 0,
    Stage = 1,
    Commit = 2,
};

struct FKsa64GlobalGuidedEvidenceMilestone
{
    const TCHAR* Label;
    uint32 ReleaseEpoch;
    uint32 GnssState;
    uint32 ReceiptState;
    EKsa64GlobalGuidedEvidenceAction Action;
};

constexpr FKsa64GlobalGuidedEvidenceMilestone GlobalGuidedEvidenceMilestones[] = {
    {TEXT("gnss-fault-begins"), 5'760, 2, 0, EKsa64GlobalGuidedEvidenceAction::None},
    {TEXT("gnss-fault-qualified"), 5'824, 3, 0, EKsa64GlobalGuidedEvidenceAction::None},
    {TEXT("ground-update-stage"), 6'080, 3, 1, EKsa64GlobalGuidedEvidenceAction::Stage},
    {TEXT("ground-update-commit"), 6'240, 3, 2, EKsa64GlobalGuidedEvidenceAction::Commit},
    {TEXT("branch-stage"), 6'560, 3, 1, EKsa64GlobalGuidedEvidenceAction::Stage},
    {TEXT("branch-commit"), 6'720, 3, 2, EKsa64GlobalGuidedEvidenceAction::Commit},
};

bool IsQualifiedHexIdentity(const FString& Value, int32 ExpectedLength)
{
    if (Value.Len() != ExpectedLength) return false;
    for (const TCHAR Character : Value)
    {
        if (!FChar::IsHexDigit(Character)) return false;
    }
    return true;
}

void AddSegment(
    TArray<FVector3d>& Points,
    const FVector3d& Start,
    const FVector3d& End)
{
    Points.Add(Start);
    Points.Add(End);
}

FVector3d EarthPoint(
    double LatitudeRadians,
    double LongitudeRadians,
    double SemiMajorCentimetres,
    double SemiMinorCentimetres)
{
    const double CosLatitude = FMath::Cos(LatitudeRadians);
    const double X = SemiMajorCentimetres
        * CosLatitude
        * FMath::Cos(LongitudeRadians);
    const double Y = -SemiMajorCentimetres
        * CosLatitude
        * FMath::Sin(LongitudeRadians);
    const double Z = SemiMinorCentimetres * FMath::Sin(LatitudeRadians);
    return FVector3d(X, Y, Z);
}
}

void UKsa64GlobalViewerSubsystem::Initialize(FSubsystemCollectionBase& Collection)
{
    Collection.InitializeDependency<UKsa64LiveMissionSubsystem>();
    Super::Initialize(Collection);
    EarthSemiMajorCentimetres = FallbackSemiMajorCentimetres;
    EarthSemiMinorCentimetres = FallbackSemiMinorCentimetres;
    SemanticState.Layout = EKsa64GlobalViewerLayout::HybridMissionDirector;
    SemanticState.RequestedCamera = EKsa64GlobalCameraMode::AutomaticDirector;
    SemanticState.ResolvedCamera = EKsa64GlobalCameraMode::LaunchLocalEnu;
    LastSceneSampleWallSeconds = FPlatformTime::Seconds();
    bGlobalEvidenceMode = FParse::Param(
        FCommandLine::Get(),
        TEXT("Ksa64Phase12cGlobalEvidence"));
    if (bGlobalEvidenceMode)
    {
        GlobalEvidenceStartedSeconds = FPlatformTime::Seconds();
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64SourceCommit="),
            GlobalEvidenceSourceCommit);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64ExecutableRelativePath="),
            GlobalEvidenceExecutableRelativePath);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64ExecutableBytes="),
            GlobalEvidenceExecutableBytes);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64ExecutableSha256="),
            GlobalEvidenceExecutableSha256);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64PackageAuditSha256="),
            GlobalEvidencePackageAuditSha256);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64PackagedDirectoryBytes="),
            GlobalEvidencePackagedDirectoryBytes);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64PackagedDirectoryFiles="),
            GlobalEvidencePackagedDirectoryFiles);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64PackagedDirectoryTreeSha256="),
            GlobalEvidencePackagedDirectoryTreeSha256);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64PackagedDirectoryInventoryFile="),
            GlobalEvidencePackagedDirectoryInventoryFile);
        FParse::Value(
            FCommandLine::Get(),
            TEXT("Ksa64PackagedDirectoryInventorySha256="),
            GlobalEvidencePackagedDirectoryInventorySha256);
        GlobalEvidenceDirectory = FPaths::Combine(
            FPaths::ProjectSavedDir(),
            TEXT("KSA64"),
            TEXT("GlobalViewerEvidence"));
        GlobalEvidenceManifestPath = FPaths::Combine(
            GlobalEvidenceDirectory,
            TEXT("phase12c-global-viewer-evidence.json"));
        GlobalEvidenceServiceNanoseconds.Reserve(GlobalEvidenceMeasuredFrameCount);
        GlobalEvidenceCaptures.Reserve(UE_ARRAY_COUNT(GlobalEvidenceMilestones));
        GlobalEvidenceGuidedRecords.Reserve(
            UE_ARRAY_COUNT(GlobalGuidedEvidenceMilestones));
        UE_LOG(
            LogKsa64GlobalViewer,
            Display,
            TEXT("KSA64_PHASE12C_GLOBAL_EVIDENCE_BEGIN milestones=%d frames=%u"),
            UE_ARRAY_COUNT(GlobalEvidenceMilestones),
            GlobalEvidenceMeasuredFrameCount);
    }

    if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
    {
        Operations->SetDashboardVisible(false);
    }
    TickerHandle = FTSTicker::GetCoreTicker().AddTicker(
        FTickerDelegate::CreateUObject(this, &UKsa64GlobalViewerSubsystem::Tick));
}

void UKsa64GlobalViewerSubsystem::Deinitialize()
{
    if (GlobalEvidenceScreenshotProcessedHandle.IsValid())
    {
        FScreenshotRequest::OnScreenshotRequestProcessed().Remove(
            GlobalEvidenceScreenshotProcessedHandle);
        GlobalEvidenceScreenshotProcessedHandle.Reset();
    }
    if (TickerHandle.IsValid())
    {
        FTSTicker::GetCoreTicker().RemoveTicker(TickerHandle);
        TickerHandle.Reset();
    }
    if (GEngine != nullptr
        && GEngine->GameViewport != nullptr
        && Overlay.IsValid()
        && bOverlayInstalled)
    {
        GEngine->GameViewport->RemoveViewportWidgetContent(Overlay.ToSharedRef());
    }
    Overlay.Reset();
    bOverlayInstalled = false;
    if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
    {
        Operations->SetDashboardVisible(true);
    }
    DestroyScene();
    Super::Deinitialize();
}

UKsa64LiveMissionSubsystem* UKsa64GlobalViewerSubsystem::GetOperations() const
{
    return GetGameInstance() != nullptr
        ? GetGameInstance()->GetSubsystem<UKsa64LiveMissionSubsystem>()
        : nullptr;
}

void UKsa64GlobalViewerSubsystem::ResetGlobalDisplayState()
{
    bGlobalDefinitionValid = false;
    bGlobalAcceptedExact = false;
    PermittedGlobalSourceMask = 0;
    GlobalPathDisplayFrame = 0;
    LastGlobalPathRefreshRelease = 0;
    GlobalDefinition = {};
    CurrentGlobalProduct = {};
    GlobalReplayIndex = {};
    GlobalTransitions.Reset();
    for (TArray<FKsa64GlobalPathPointProduct>& Path : GlobalPaths) Path.Reset();
    PreviousSample = {};
    CurrentSample = {};
    bHasPreviousSample = false;
    ReplayOldestRelease = 0;
    ReplayNewestRelease = 0;
    ReplaySelectedRelease = 0;
    ReplayLastReadRelease = MAX_uint32;
    ReplayPace = EKsa64GlobalReplayPace::Paused;
    ReplayReleaseAccumulator = 0.0;
    bReplaySeekSnapPending = true;
    bTruthRequested = false;
    SemanticState.DisplayAvailability = EKsa64GlobalDisplayAvailability::Unavailable;
    SemanticState.ReplayPace = ReplayPace;
    SemanticState.ReplayOldestRelease = 0;
    SemanticState.ReplayNewestRelease = 0;
    SemanticState.ReplaySelectedRelease = 0;
    SemanticState.ReplayBookmarkCount = 0;
    SemanticState.SourceMask = 0;
    SemanticState.ObservedPathPoints = 0;
    SemanticState.PlannedPathPoints = 0;
    SemanticState.OnboardPathPoints = 0;
    SemanticState.GroundPathPoints = 0;
    SemanticState.TransitionMarkers = 0;
    SemanticState.bAcceptanceEligible = false;
    SemanticState.bTruthVisible = false;
}

bool UKsa64GlobalViewerSubsystem::StartGuidedOperations()
{
    UKsa64LiveMissionSubsystem* Operations = GetOperations();
    if (Operations == nullptr || !Operations->StartGuidedOperations()) return false;
    ResetGlobalDisplayState();
    SemanticState.ExperienceMode = EKsa64GlobalExperienceMode::GuidedOperations;
    SemanticState.RoleLabel = TEXT("GUIDED OPERATOR");
    SemanticState.StatusLabel = TEXT("GUIDED GNSS-LOSS OPERATIONS OPENING");
    return true;
}

bool UKsa64GlobalViewerSubsystem::StartNominalReplay()
{
    UKsa64LiveMissionSubsystem* Operations = GetOperations();
    if (Operations == nullptr || !Operations->StartNominalGlobalReplay()) return false;
    ResetGlobalDisplayState();
    SemanticState.ExperienceMode = EKsa64GlobalExperienceMode::NominalReplay;
    SemanticState.RoleLabel = TEXT("SIM DIRECTOR · READ ONLY");
    SemanticState.StatusLabel = TEXT("VERIFYING FROZEN PHASE 10 REPLAY");
    return true;
}

bool UKsa64GlobalViewerSubsystem::Tick(float DeltaSeconds)
{
    InstallOverlayIfPossible();
    EnsureScene();
    if (bGlobalEvidenceMode)
    {
        TickGlobalEvidence(DeltaSeconds);
    }
    else
    {
        ObserveOperations(DeltaSeconds);
    }
    return true;
}

void UKsa64GlobalViewerSubsystem::InstallOverlayIfPossible()
{
    if (bOverlayInstalled || GEngine == nullptr || GEngine->GameViewport == nullptr)
    {
        return;
    }
    Overlay = SNew(SKsa64GlobalViewerOverlay).Subsystem(this);
    GEngine->GameViewport->AddViewportWidgetContent(Overlay.ToSharedRef(), 200);
    FSlateApplication::Get().SetKeyboardFocus(Overlay, EFocusCause::SetDirectly);
    bOverlayInstalled = true;
}

bool UKsa64GlobalViewerSubsystem::EnsureScene()
{
    if (SceneRootActor.IsValid())
    {
        return true;
    }
    UWorld* World = GetWorld();
    if (World == nullptr
        || !World->IsGameWorld()
        || !World->HasBegunPlay()
        || GEngine == nullptr
        || GEngine->GameViewport == nullptr
        || GEngine->GameViewport->GetWorld() != World)
    {
        return false;
    }
    if (bSceneAttempted)
    {
        return false;
    }
    bSceneAttempted = true;

    FActorSpawnParameters SpawnParameters;
    SpawnParameters.Name = TEXT("Ksa64GlobalViewerScene");
    SpawnParameters.SpawnCollisionHandlingOverride =
        ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    AActor* SceneActor = World->SpawnActor<AActor>(
        AActor::StaticClass(),
        FTransform::Identity,
        SpawnParameters);
    if (SceneActor == nullptr)
    {
        UE_LOG(LogKsa64GlobalViewer, Error, TEXT("could not create global scene actor"));
        return false;
    }
    USceneComponent* Root = NewObject<USceneComponent>(SceneActor, TEXT("GlobalSceneRoot"));
    SceneActor->SetRootComponent(Root);
    SceneActor->AddInstanceComponent(Root);
    Root->RegisterComponent();
    SceneRootActor = SceneActor;

    EarthMesh = CreateMeshComponent(
        *SceneActor,
        TEXT("Wgs84Earth"),
        TEXT("/Engine/BasicShapes/Sphere.Sphere"),
        FLinearColor(0.018f, 0.10f, 0.18f, 1.0f));
    AtmosphereMesh = CreateMeshComponent(
        *SceneActor,
        TEXT("AtmosphereShell"),
        TEXT("/Engine/BasicShapes/Sphere.Sphere"),
        FLinearColor(0.04f, 0.30f, 0.55f, 0.18f));
    LocatorMesh = CreateMeshComponent(
        *SceneActor,
        TEXT("VehicleLocator"),
        TEXT("/Engine/BasicShapes/Sphere.Sphere"),
        FLinearColor(1.0f, 0.62f, 0.12f, 1.0f));
    GroundLocatorMesh = CreateMeshComponent(
        *SceneActor,
        TEXT("GroundEstimateLocator"),
        TEXT("/Engine/BasicShapes/Cube.Cube"),
        FLinearColor(0.24f, 0.58f, 1.0f, 0.62f));
    TruthLocatorMesh = CreateMeshComponent(
        *SceneActor,
        TEXT("SimTruthLocator"),
        TEXT("/Engine/BasicShapes/Cone.Cone"),
        FLinearColor(0.82f, 0.32f, 1.0f, 0.68f));

    EarthGridLines = CreateLineComponent(
        *SceneActor, TEXT("EarthGrid"), FLinearColor(0.10f, 0.55f, 0.68f, 0.55f), 0.75f);
    LocalGridLines = CreateLineComponent(
        *SceneActor, TEXT("LocalGrid"), FLinearColor(0.12f, 0.42f, 0.50f, 0.55f), 0.75f);
    AxisXLines = CreateLineComponent(
        *SceneActor, TEXT("AxisX"), FLinearColor(0.95f, 0.28f, 0.25f, 0.9f), 1.5f);
    AxisYLines = CreateLineComponent(
        *SceneActor, TEXT("AxisY"), FLinearColor(0.25f, 0.92f, 0.45f, 0.9f), 1.5f);
    AxisZLines = CreateLineComponent(
        *SceneActor, TEXT("AxisZ"), FLinearColor(0.25f, 0.55f, 1.0f, 0.9f), 1.5f);
    ObservedPathLines = CreateLineComponent(
        *SceneActor, TEXT("ObservedPath"), FLinearColor(0.31f, 0.93f, 0.57f, 1.0f), 2.5f);
    PlannedPathLines = CreateLineComponent(
        *SceneActor, TEXT("PlannedPath"), FLinearColor(0.90f, 0.92f, 0.95f, 0.75f), 1.0f);
    OnboardPathLines = CreateLineComponent(
        *SceneActor, TEXT("OnboardPath"), FLinearColor(0.14f, 0.83f, 0.95f, 0.9f), 1.75f);
    GroundPathLines = CreateLineComponent(
        *SceneActor, TEXT("GroundPath"), FLinearColor(1.0f, 0.66f, 0.18f, 0.9f), 1.75f);
    TransitionMarkerLines = CreateLineComponent(
        *SceneActor, TEXT("TransitionMarkers"), FLinearColor(0.95f, 0.40f, 0.82f, 0.95f), 2.0f);

    AActor* VehicleActor = World->SpawnActor<AActor>(
        AActor::StaticClass(),
        FTransform::Identity);
    if (VehicleActor != nullptr)
    {
        #if WITH_EDITOR
        VehicleActor->SetActorLabel(TEXT("KSA-G10R Schematic Vehicle"));
        #endif
        USceneComponent* VehicleRoot =
            NewObject<USceneComponent>(VehicleActor, TEXT("VehicleRoot"));
        VehicleActor->SetRootComponent(VehicleRoot);
        VehicleActor->AddInstanceComponent(VehicleRoot);
        VehicleRoot->RegisterComponent();
        VehicleBodyMesh = CreateMeshComponent(
            *VehicleActor,
            TEXT("VehicleBody"),
            TEXT("/Engine/BasicShapes/Cylinder.Cylinder"),
            FLinearColor(0.78f, 0.82f, 0.86f, 1.0f));
        VehicleNoseMesh = CreateMeshComponent(
            *VehicleActor,
            TEXT("VehicleNose"),
            TEXT("/Engine/BasicShapes/Cone.Cone"),
            FLinearColor(0.95f, 0.36f, 0.20f, 1.0f));
        if (VehicleBodyMesh.IsValid())
        {
            VehicleBodyMesh->AttachToComponent(
                VehicleRoot,
                FAttachmentTransformRules::KeepRelativeTransform);
            VehicleBodyMesh->SetRelativeRotation(FRotator(0.0, 90.0, 0.0));
            VehicleBodyMesh->SetRelativeScale3D(FVector(0.40, 0.40, 7.20));
        }
        if (VehicleNoseMesh.IsValid())
        {
            VehicleNoseMesh->AttachToComponent(
                VehicleRoot,
                FAttachmentTransformRules::KeepRelativeTransform);
            VehicleNoseMesh->SetRelativeRotation(FRotator(0.0, 90.0, 0.0));
            VehicleNoseMesh->SetRelativeLocation(FVector(400.0, 0.0, 0.0));
            VehicleNoseMesh->SetRelativeScale3D(FVector(0.40, 0.40, 0.80));
        }
    }

    ACameraActor* Camera = World->SpawnActor<ACameraActor>(
        ACameraActor::StaticClass(),
        FTransform::Identity);
    if (Camera != nullptr)
    {
        #if WITH_EDITOR
        Camera->SetActorLabel(TEXT("KSA64 Global Viewer Camera"));
        #endif
        Camera->GetCameraComponent()->FieldOfView = 50.0f;
        ViewerCamera = Camera;
        if (APlayerController* Controller = World->GetFirstPlayerController())
        {
            Controller->SetViewTarget(Camera);
        }
    }

    UDirectionalLightComponent* Sun =
        NewObject<UDirectionalLightComponent>(SceneActor, TEXT("ProceduralSun"));
    SceneActor->AddInstanceComponent(Sun);
    Sun->SetupAttachment(Root);
    Sun->SetRelativeRotation(FRotator(-32.0, -25.0, 0.0));
    Sun->SetIntensity(4.0f);
    Sun->RegisterComponent();

    if (EarthMesh.IsValid())
    {
        EarthMesh->SetRelativeScale3D(FVector(
            EarthSemiMajorCentimetres / 50.0,
            EarthSemiMajorCentimetres / 50.0,
            EarthSemiMinorCentimetres / 50.0));
    }
    if (AtmosphereMesh.IsValid())
    {
        AtmosphereMesh->SetRelativeScale3D(FVector(
            EarthSemiMajorCentimetres * 1.012 / 50.0,
            EarthSemiMajorCentimetres * 1.012 / 50.0,
            EarthSemiMinorCentimetres * 1.012 / 50.0));
    }
    if (LocatorMesh.IsValid()) LocatorMesh->SetRelativeScale3D(FVector(0.08));
    if (GroundLocatorMesh.IsValid())
    {
        GroundLocatorMesh->SetRelativeScale3D(FVector(0.07));
        GroundLocatorMesh->SetVisibility(false);
    }
    if (TruthLocatorMesh.IsValid())
    {
        TruthLocatorMesh->SetRelativeScale3D(FVector(0.07));
        TruthLocatorMesh->SetVisibility(false);
    }

    BuildEarthGrid();
    BuildLocalGrid();
    SemanticState.bSceneReady = true;
    return true;
}

void UKsa64GlobalViewerSubsystem::DestroyScene()
{
    if (ViewerCamera.IsValid()) ViewerCamera->Destroy();
    if (VehicleBodyMesh.IsValid() && VehicleBodyMesh->GetOwner() != nullptr)
    {
        VehicleBodyMesh->GetOwner()->Destroy();
    }
    if (SceneRootActor.IsValid()) SceneRootActor->Destroy();
    ViewerCamera.Reset();
    GroundLocatorMesh.Reset();
    TruthLocatorMesh.Reset();
    SceneRootActor.Reset();
    SemanticState.bSceneReady = false;
}

UStaticMeshComponent* UKsa64GlobalViewerSubsystem::CreateMeshComponent(
    AActor& Owner,
    const TCHAR* Name,
    const TCHAR* MeshPath,
    const FLinearColor& Color)
{
    UStaticMesh* Mesh = LoadObject<UStaticMesh>(nullptr, MeshPath);
    UStaticMeshComponent* Component =
        NewObject<UStaticMeshComponent>(&Owner, FName(Name));
    Owner.AddInstanceComponent(Component);
    if (Owner.GetRootComponent() != nullptr)
    {
        Component->SetupAttachment(Owner.GetRootComponent());
    }
    Component->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Component->SetCastShadow(false);
    Component->SetStaticMesh(Mesh);
    UMaterialInterface* BasicMaterial = LoadObject<UMaterialInterface>(
        nullptr,
        TEXT("/Engine/BasicShapes/BasicShapeMaterial.BasicShapeMaterial"));
    if (BasicMaterial != nullptr)
    {
        UMaterialInstanceDynamic* Material =
            UMaterialInstanceDynamic::Create(BasicMaterial, Component);
        Material->SetVectorParameterValue(TEXT("Color"), Color);
        Component->SetMaterial(0, Material);
    }
    Component->RegisterComponent();
    return Component;
}

UKsa64GlobalLineComponent* UKsa64GlobalViewerSubsystem::CreateLineComponent(
    AActor& Owner,
    const TCHAR* Name,
    const FLinearColor& Color,
    float Thickness)
{
    UKsa64GlobalLineComponent* Component =
        NewObject<UKsa64GlobalLineComponent>(&Owner, FName(Name));
    Owner.AddInstanceComponent(Component);
    if (Owner.GetRootComponent() != nullptr)
    {
        Component->SetupAttachment(Owner.GetRootComponent());
    }
    Component->SetSegments({}, Color, Thickness);
    Component->RegisterComponent();
    return Component;
}

void UKsa64GlobalViewerSubsystem::BuildEarthGrid()
{
    if (!EarthGridLines.IsValid())
    {
        return;
    }
    TArray<FVector3d> Segments;
    Segments.Reserve(
        (EarthGridLatitudeSteps + EarthGridLongitudeSteps)
        * EarthGridCurveSteps
        * 2);
    for (int32 Latitude = -EarthGridLatitudeSteps / 2 + 1;
         Latitude < EarthGridLatitudeSteps / 2;
         ++Latitude)
    {
        const double LatRadians = Latitude * UE_PI / EarthGridLatitudeSteps;
        FVector3d Previous = EarthPoint(
            LatRadians, -UE_PI, EarthSemiMajorCentimetres, EarthSemiMinorCentimetres);
        for (int32 Step = 1; Step <= EarthGridCurveSteps; ++Step)
        {
            const double Longitude =
                -UE_PI + 2.0 * UE_PI * Step / EarthGridCurveSteps;
            const FVector3d Current = EarthPoint(
                LatRadians, Longitude, EarthSemiMajorCentimetres, EarthSemiMinorCentimetres);
            AddSegment(Segments, Previous, Current);
            Previous = Current;
        }
    }
    for (int32 Longitude = 0; Longitude < EarthGridLongitudeSteps; ++Longitude)
    {
        const double LonRadians =
            2.0 * UE_PI * Longitude / EarthGridLongitudeSteps;
        FVector3d Previous = EarthPoint(
            -UE_PI * 0.5, LonRadians, EarthSemiMajorCentimetres, EarthSemiMinorCentimetres);
        for (int32 Step = 1; Step <= EarthGridCurveSteps / 2; ++Step)
        {
            const double Latitude =
                -UE_PI * 0.5 + UE_PI * Step / (EarthGridCurveSteps / 2);
            const FVector3d Current = EarthPoint(
                Latitude, LonRadians, EarthSemiMajorCentimetres, EarthSemiMinorCentimetres);
            AddSegment(Segments, Previous, Current);
            Previous = Current;
        }
    }
    EarthGridLines->SetSegments(
        Segments,
        FLinearColor(0.10f, 0.55f, 0.68f, 0.55f),
        0.75f);
}

void UKsa64GlobalViewerSubsystem::BuildLocalGrid()
{
    if (!LocalGridLines.IsValid())
    {
        return;
    }
    TArray<FVector3d> Segments;
    constexpr double Extent = 1'000'000.0;
    constexpr double Spacing = 100'000.0;
    for (double Offset = -Extent; Offset <= Extent; Offset += Spacing)
    {
        AddSegment(
            Segments,
            FVector3d(-Extent, Offset, 0.0),
            FVector3d(Extent, Offset, 0.0));
        AddSegment(
            Segments,
            FVector3d(Offset, -Extent, 0.0),
            FVector3d(Offset, Extent, 0.0));
    }
    LocalGridLines->SetSegments(
        Segments,
        FLinearColor(0.12f, 0.42f, 0.50f, 0.55f),
        0.75f);

    constexpr double AxisLength = 2'000'000.0;
    TArray<FVector3d> Axis;
    AddSegment(Axis, FVector3d::ZeroVector, FVector3d(AxisLength, 0.0, 0.0));
    AxisXLines->SetSegments(Axis, FLinearColor(0.95f, 0.28f, 0.25f), 1.5f);
    Axis.Reset();
    AddSegment(Axis, FVector3d::ZeroVector, FVector3d(0.0, -AxisLength, 0.0));
    AxisYLines->SetSegments(Axis, FLinearColor(0.25f, 0.92f, 0.45f), 1.5f);
    Axis.Reset();
    AddSegment(Axis, FVector3d::ZeroVector, FVector3d(0.0, 0.0, AxisLength));
    AxisZLines->SetSegments(Axis, FLinearColor(0.25f, 0.55f, 1.0f), 1.5f);
}

void UKsa64GlobalViewerSubsystem::ObserveOperations(float DeltaSeconds)
{
    UKsa64LiveMissionSubsystem* Operations = GetOperations();
    if (Operations == nullptr)
    {
        SemanticState.StatusLabel = TEXT("OPERATIONS SUBSYSTEM UNAVAILABLE");
        return;
    }
    const FKsa64OperationsViewModel& View = Operations->GetViewModel();
    SemanticState.bOperationsDeskVisible = Operations->IsDashboardVisible();
    SemanticState.bSessionOpen = View.bSessionOpen;
    SemanticState.RoleLabel = View.RoleLabel;
    SemanticState.FrameLabel = View.FrameLabel;
    SemanticState.DispositionLabel = View.DispositionLabel;
    SemanticState.StatusLabel = View.SessionStatus;
    SemanticState.OverallDisposition = View.OverallDisposition;
    SemanticState.ObjectiveDisposition = View.ObjectiveDisposition;
    SemanticState.VehicleDisposition = View.VehicleDisposition;
    SemanticState.ProcedureDisposition = View.ProcedureDisposition;
    SemanticState.OperatorDisposition = View.OperatorDisposition;
    SemanticState.AvionicsDisposition = View.AvionicsDisposition;
    SemanticState.EvidenceDisposition = View.EvidenceDisposition;
    SemanticState.bObservationComplete = View.bObservationComplete;
    SemanticState.bTruthPermitted = !View.bTruthFiltered;
    if (!SemanticState.bTruthPermitted)
    {
        bTruthRequested = false;
    }
    SemanticState.bTruthVisible =
        SemanticState.bTruthPermitted && bTruthRequested;

    if (Operations->SupportsGlobalDisplayV1())
    {
        ObserveGlobalDisplay(*Operations, DeltaSeconds);
        return;
    }

    // Compatibility-only preview for an archived ABI-v1 bridge. This path is
    // intentionally ineligible for Phase 12C parity or exact-display evidence.
    FKsa64OperationsReleasePoint VisualPoint;
    if (!Operations->GetVisualObservedPoint(VisualPoint))
    {
        return;
    }
    FKsa64GlobalSceneSample Sample;
    Sample.ReleaseEpoch = VisualPoint.ReleaseEpoch;
    Sample.MissionTimeQ16 = VisualPoint.MissionTimeQ16;
    Sample.FrameIdentity = VisualPoint.FrameIdentity != 0
        ? VisualPoint.FrameIdentity
        : View.FrameIdentity;
    Sample.SegmentIdentity = 0;
    Sample.ContinuityIdentity = Sample.FrameIdentity;
    Sample.bPositionValid = VisualPoint.bHasPosition;
    Sample.bGroundPositionValid = VisualPoint.bHasGroundEstimate;
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        Sample.PositionQ12Km[Axis] = VisualPoint.PositionQ12[Axis];
        Sample.GroundPositionQ12Km[Axis] = VisualPoint.GroundPositionQ12[Axis];
    }
    Sample.bAttitudeValid = false;
    Sample.bExactSnap = !bHasPreviousSample
        || Operations->GetDisplayMode() == EKsa64OperationsDisplayMode::Exact
        || Ksa64GlobalViewerPolicy::ShouldSnap(PreviousSample, Sample);

    if (!bHasPreviousSample || Sample.ReleaseEpoch != CurrentSample.ReleaseEpoch)
    {
        PreviousSample = CurrentSample;
        CurrentSample = Sample;
        bHasPreviousSample = true;
        LastSceneSampleWallSeconds = FPlatformTime::Seconds();
        RefreshSemanticState(Sample, *Operations);
        UpdateDisplayOrigin(Sample);
        UpdateScene(Sample);
    }
    else
    {
        UpdateCamera(Sample, DeltaSeconds);
    }
}

bool UKsa64GlobalViewerSubsystem::InitializeGlobalDisplay(
    UKsa64LiveMissionSubsystem& Operations)
{
    Ksa64GlobalDisplayAvailabilityV1 Availability = {};
    if (Operations.GetGlobalDisplayAvailability(Availability)
        != EKsa64OperationsAdapterResult::Ok)
    {
        return false;
    }
    TArray<uint8> DefinitionPayload;
    if (Operations.GetGlobalDisplayDefinition(DefinitionPayload)
        != EKsa64OperationsAdapterResult::Ok)
    {
        return false;
    }
    FKsa64GlobalDisplayDefinitionProduct Candidate;
    FString Error;
    if (!FKsa64GlobalDisplayCodec::DecodeDefinition(
            DefinitionPayload, Candidate, Error)
        || Candidate.DisplayIdentity != Availability.display_identity
        || Candidate.AvailableSourceMask != Availability.available_source_mask
        || Candidate.AvailableFrameMask != Availability.available_frame_mask
        || (Availability.role != 5u
            && (Availability.available_source_mask & (1u << 3)) != 0)
        || (Operations.IsGlobalReplayMode() && Availability.role != 5u)
        || (!Operations.IsGlobalReplayMode() && Availability.role != 2u))
    {
        SemanticState.StatusLabel = FString::Printf(
            TEXT("GLOBAL DISPLAY REJECTED · %s"),
            Error.IsEmpty() ? TEXT("identity mismatch") : *Error);
        return false;
    }
    GlobalDefinition = Candidate;
    PermittedGlobalSourceMask = Candidate.AvailableSourceMask;
    bGlobalAcceptedExact =
        (Availability.flags & KSA64_GLOBAL_DISPLAY_AVAILABILITY_ACCEPTED_EXACT) != 0;
    bGlobalDefinitionValid = true;
    ReplayOldestRelease = Availability.oldest_sample_release;
    ReplayNewestRelease = Availability.newest_sample_release;
    if (Operations.IsGlobalReplayMode())
    {
        ReplaySelectedRelease = ReplayOldestRelease;
        ReplayLastReadRelease = MAX_uint32;
        ReplayPace = EKsa64GlobalReplayPace::Paused;
        bReplaySeekSnapPending = true;
    }
    SemanticState.ReplayPace = ReplayPace;
    SemanticState.ReplayOldestRelease = ReplayOldestRelease;
    SemanticState.ReplayNewestRelease = ReplayNewestRelease;
    SemanticState.ReplaySelectedRelease = ReplaySelectedRelease;
    SemanticState.bTruthPermitted = Availability.role == 5u
        && (Availability.available_source_mask & (1u << 3)) != 0;
    if (!SemanticState.bTruthPermitted) bTruthRequested = false;
    SemanticState.bTruthVisible = SemanticState.bTruthPermitted && bTruthRequested;
    ApplyAcceptedEarthDefinition();

    TArray<uint8> ReplayPayload;
    if (Operations.GetGlobalReplayIndex(ReplayPayload)
        == EKsa64OperationsAdapterResult::Ok)
    {
        FKsa64GlobalReplayIndexProduct CandidateReplay;
        if (FKsa64GlobalDisplayCodec::DecodeReplayIndex(
                ReplayPayload, CandidateReplay, Error))
        {
            GlobalReplayIndex = MoveTemp(CandidateReplay);
            SemanticState.ReplayBookmarkCount = GlobalReplayIndex.Entries.Num();
        }
    }
    if (CurrentGlobalProduct.ActiveFrame != 0) RefreshGlobalPaths(Operations);
    return true;
}

bool UKsa64GlobalViewerSubsystem::ObserveGlobalDisplay(
    UKsa64LiveMissionSubsystem& Operations,
    float DeltaSeconds)
{
    if (!bGlobalDefinitionValid && !InitializeGlobalDisplay(Operations))
    {
        SemanticState.bAcceptanceEligible = false;
        return false;
    }

    for (int32 Count = 0; Count < 256; ++Count)
    {
        TArray<uint8> TransitionPayload;
        const EKsa64OperationsAdapterResult Result =
            Operations.PollGlobalDisplayTransition(TransitionPayload);
        if (Result == EKsa64OperationsAdapterResult::NoData
            || Result == EKsa64OperationsAdapterResult::Unchanged)
        {
            break;
        }
        if (Result != EKsa64OperationsAdapterResult::Ok)
        {
            SemanticState.StatusLabel = TEXT("GLOBAL TRANSITION STREAM FAILED CLOSED");
            SemanticState.bAcceptanceEligible = false;
            return false;
        }
        FKsa64GlobalTransitionProduct Transition;
        FString Error;
        if (!FKsa64GlobalDisplayCodec::DecodeTransition(
                TransitionPayload, Transition, Error))
        {
            SemanticState.StatusLabel = FString::Printf(
                TEXT("GLOBAL TRANSITION REJECTED · %s"), *Error);
            SemanticState.bAcceptanceEligible = false;
            return false;
        }
        GlobalTransitions.Add(Transition);
    }

    bool bObservedSample = false;
    if (Operations.IsGlobalReplayMode())
    {
        AdvanceReplayPresentation(DeltaSeconds);
        bObservedSample = ReadReplaySample(Operations, DeltaSeconds);
    }
    else
    {
        for (int32 Count = 0; Count < 256; ++Count)
        {
            TArray<uint8> SamplePayload;
            const EKsa64OperationsAdapterResult Result =
                Operations.PollGlobalDisplaySample(SamplePayload);
            if (Result == EKsa64OperationsAdapterResult::NoData
                || Result == EKsa64OperationsAdapterResult::Unchanged)
            {
                break;
            }
            if (Result != EKsa64OperationsAdapterResult::Ok)
            {
                SemanticState.StatusLabel = TEXT("GLOBAL SAMPLE STREAM FAILED CLOSED");
                SemanticState.bAcceptanceEligible = false;
                return false;
            }
            TArray<FKsa64GlobalDisplaySampleProduct> Samples;
            FString Error;
            if (!FKsa64GlobalDisplayCodec::DecodeSamples(
                    SamplePayload, PermittedGlobalSourceMask, Samples, Error))
            {
                SemanticState.StatusLabel = FString::Printf(
                    TEXT("GLOBAL SAMPLE REJECTED · %s"), *Error);
                SemanticState.bAcceptanceEligible = false;
                return false;
            }
            for (const FKsa64GlobalDisplaySampleProduct& Sample : Samples)
            {
                ApplyGlobalSample(Sample, Operations, DeltaSeconds);
                bObservedSample = true;
            }
        }
    }

    if (bObservedSample
        && (LastGlobalPathRefreshRelease == 0
            || CurrentSample.ReleaseEpoch - LastGlobalPathRefreshRelease >= 32
            || CurrentSample.bExactSnap))
    {
        RefreshGlobalPaths(Operations);
    }
    else if (!bObservedSample && bHasPreviousSample)
    {
        UpdateCamera(CurrentSample, DeltaSeconds);
    }
    return true;
}

void UKsa64GlobalViewerSubsystem::AdvanceReplayPresentation(float DeltaSeconds)
{
    if (!IsNominalReplay()
        || ReplayPace == EKsa64GlobalReplayPace::Paused
        || ReplaySelectedRelease >= ReplayNewestRelease)
    {
        return;
    }
    double Rate = 0.0;
    switch (ReplayPace)
    {
    case EKsa64GlobalReplayPace::Quarter: Rate = 0.25; break;
    case EKsa64GlobalReplayPace::Half: Rate = 0.5; break;
    case EKsa64GlobalReplayPace::One: Rate = 1.0; break;
    case EKsa64GlobalReplayPace::Two: Rate = 2.0; break;
    case EKsa64GlobalReplayPace::Four: Rate = 4.0; break;
    case EKsa64GlobalReplayPace::Eight: Rate = 8.0; break;
    case EKsa64GlobalReplayPace::Sixteen: Rate = 16.0; break;
    case EKsa64GlobalReplayPace::Unpaced: Rate = 0.0; break;
    default: return;
    }
    uint32 Releases = 0;
    if (ReplayPace == EKsa64GlobalReplayPace::Unpaced)
    {
        Releases = 1'024;
    }
    else
    {
        ReplayReleaseAccumulator += FMath::Max(0.0f, DeltaSeconds) * 32.0 * Rate;
        Releases = static_cast<uint32>(FMath::FloorToDouble(ReplayReleaseAccumulator));
        ReplayReleaseAccumulator -= Releases;
    }
    if (Releases == 0) return;
    const uint64 Candidate = static_cast<uint64>(ReplaySelectedRelease) + Releases;
    ReplaySelectedRelease = static_cast<uint32>(FMath::Min<uint64>(
        Candidate, ReplayNewestRelease));
    SemanticState.ReplaySelectedRelease = ReplaySelectedRelease;
    if (ReplaySelectedRelease >= ReplayNewestRelease)
    {
        ReplayPace = EKsa64GlobalReplayPace::Paused;
        SemanticState.ReplayPace = ReplayPace;
    }
}

bool UKsa64GlobalViewerSubsystem::ReadReplaySample(
    UKsa64LiveMissionSubsystem& Operations,
    float DeltaSeconds)
{
    if (ReplaySelectedRelease == ReplayLastReadRelease) return false;
    Ksa64GlobalDisplaySampleRangeRequestV1 Request = {};
    Request.api_version = KSA64_GLOBAL_DISPLAY_API_VERSION;
    Request.struct_size = sizeof(Request);
    Request.start_release = ReplaySelectedRelease;
    Request.max_count = 1;
    TArray<uint8> Payload;
    const EKsa64OperationsAdapterResult Result =
        Operations.GetGlobalDisplaySampleRange(Request, Payload);
    if (Result == EKsa64OperationsAdapterResult::NoData
        || Result == EKsa64OperationsAdapterResult::Unchanged)
    {
        return false;
    }
    if (Result != EKsa64OperationsAdapterResult::Ok)
    {
        SemanticState.StatusLabel = TEXT("GLOBAL REPLAY RANGE FAILED CLOSED");
        SemanticState.bAcceptanceEligible = false;
        return false;
    }
    TArray<FKsa64GlobalDisplaySampleProduct> Samples;
    FString Error;
    if (!FKsa64GlobalDisplayCodec::DecodeSamples(
            Payload, PermittedGlobalSourceMask, Samples, Error)
        || Samples.Num() != 1
        || Samples[0].ReleaseEpoch != ReplaySelectedRelease)
    {
        SemanticState.StatusLabel = FString::Printf(
            TEXT("GLOBAL REPLAY SAMPLE REJECTED · %s"),
            Error.IsEmpty() ? TEXT("release mismatch") : *Error);
        SemanticState.bAcceptanceEligible = false;
        return false;
    }
    ApplyGlobalSample(Samples[0], Operations, DeltaSeconds);
    ReplayLastReadRelease = ReplaySelectedRelease;
    bReplaySeekSnapPending = false;
    return true;
}

void UKsa64GlobalViewerSubsystem::SeekReplayRelease(uint32 ReleaseEpoch)
{
    if (!IsNominalReplay() || !bGlobalDefinitionValid) return;
    ReplaySelectedRelease = FMath::Clamp(
        ReleaseEpoch, ReplayOldestRelease, ReplayNewestRelease);
    SemanticState.ReplaySelectedRelease = ReplaySelectedRelease;
    ReplayReleaseAccumulator = 0.0;
    ReplayLastReadRelease = MAX_uint32;
    bReplaySeekSnapPending = true;
}

void UKsa64GlobalViewerSubsystem::ApplyGlobalSample(
    const FKsa64GlobalDisplaySampleProduct& Product,
    const UKsa64LiveMissionSubsystem& Operations,
    float DeltaSeconds)
{
    CurrentGlobalProduct = Product;
    const FKsa64GlobalSourcePoseProduct* Onboard = FindGlobalSource(2);
    if (Onboard == nullptr) Onboard = FindGlobalSource(1);
    if (Onboard == nullptr) return;

    FKsa64GlobalSceneSample Sample;
    Sample.ReleaseEpoch = Product.ReleaseEpoch;
    Sample.MissionTimeQ16 = Product.MissionTimeQ16;
    Sample.FrameIdentity = Product.ActiveFrame;
    Sample.SegmentIdentity = Product.Segment;
    Sample.EventMask = Product.EventMask;
    Sample.DiscontinuityMask = Product.DiscontinuityMask;
    Sample.ContinuityIdentity = Product.ContinuityIdentity;
    Sample.bPositionValid = (Onboard->ValidityMask & (1u << 0)) != 0;
    Sample.bAttitudeValid = (Onboard->ValidityMask & (1u << 2)) != 0;
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        Sample.PositionQ12Km[Axis] = Onboard->Active.PositionQ12Km[Axis];
    }
    for (int32 Component = 0; Component < 4; ++Component)
    {
        Sample.AttitudeQ30[Component] = Onboard->Active.AttitudeQ30[Component];
    }
    if (const FKsa64GlobalSourcePoseProduct* Ground = FindGlobalSource(3))
    {
        Sample.bGroundPositionValid = (Ground->ValidityMask & (1u << 0)) != 0;
        for (int32 Axis = 0; Axis < 3; ++Axis)
        {
            Sample.GroundPositionQ12Km[Axis] = Ground->Active.PositionQ12Km[Axis];
        }
    }
    Sample.bExactSnap = !bHasPreviousSample
        || bReplaySeekSnapPending
        || Product.EventMask != 0
        || Product.DiscontinuityMask != 0
        || Operations.GetDisplayMode() == EKsa64OperationsDisplayMode::Exact
        || Ksa64GlobalViewerPolicy::ShouldSnap(PreviousSample, Sample);

    PreviousSample = CurrentSample;
    CurrentSample = Sample;
    bHasPreviousSample = true;
    LastSceneSampleWallSeconds = FPlatformTime::Seconds();
    RefreshSemanticState(Sample, Operations);
    UpdateDisplayOrigin(Sample);
    UpdateScene(Sample);
    if (!Sample.bExactSnap) UpdateCamera(Sample, DeltaSeconds);
}

void UKsa64GlobalViewerSubsystem::RefreshGlobalPaths(
    UKsa64LiveMissionSubsystem& Operations)
{
    uint32 RequestedFrame = CurrentGlobalProduct.ActiveFrame;
    switch (SemanticState.ResolvedCamera)
    {
    case EKsa64GlobalCameraMode::LaunchLocalEnu:
    case EKsa64GlobalCameraMode::RecoveryLocalEnu:
        RequestedFrame = 1;
        break;
    case EKsa64GlobalCameraMode::EarthFixed:
        RequestedFrame = 2;
        break;
    case EKsa64GlobalCameraMode::EarthInertial:
    case EKsa64GlobalCameraMode::FreeOrbit:
        RequestedFrame = 3;
        break;
    default:
        break;
    }
    uint32 RequestedLod = 2;
    switch (SemanticState.ResolvedCamera)
    {
    case EKsa64GlobalCameraMode::LaunchLocalEnu:
    case EKsa64GlobalCameraMode::RecoveryLocalEnu:
    case EKsa64GlobalCameraMode::VehicleChase:
    case EKsa64GlobalCameraMode::TrueScaleInspection:
        RequestedLod = 1;
        break;
    case EKsa64GlobalCameraMode::EarthFixed:
    case EKsa64GlobalCameraMode::EarthInertial:
    case EKsa64GlobalCameraMode::FreeOrbit:
        RequestedLod = 3;
        break;
    default:
        break;
    }
    for (uint32 Source = 1; Source <= 4; ++Source)
    {
        const uint32 SourceBit = 1u << (Source - 1u);
        if ((PermittedGlobalSourceMask & SourceBit) == 0)
        {
            GlobalPaths[Source - 1].Reset();
            continue;
        }
        TArray<FKsa64GlobalPathPointProduct> Candidate;
        uint32 ChunkIndex = 0;
        uint32 ChunkCount = 1;
        while (ChunkIndex < ChunkCount && ChunkIndex < 64)
        {
            Ksa64GlobalDisplayPathRequestV1 Request = {};
            Request.api_version = KSA64_GLOBAL_DISPLAY_API_VERSION;
            Request.struct_size = sizeof(Request);
            Request.source = Source;
            Request.display_frame = RequestedFrame;
            Request.lod = RequestedLod;
            Request.chunk_index = ChunkIndex;
            TArray<uint8> Payload;
            const EKsa64OperationsAdapterResult Result =
                Operations.GetGlobalPathChunk(Request, Payload);
            if (Result == EKsa64OperationsAdapterResult::NoData) break;
            if (Result != EKsa64OperationsAdapterResult::Ok)
            {
                Candidate.Reset();
                break;
            }
            FKsa64GlobalPathChunkProduct Chunk;
            FString Error;
            if (!FKsa64GlobalDisplayCodec::DecodePath(
                    Payload, PermittedGlobalSourceMask, Chunk, Error)
                || Chunk.Source != Source
                || Chunk.DisplayFrame != RequestedFrame
                || Chunk.Lod != RequestedLod
                || Chunk.ChunkIndex != ChunkIndex)
            {
                Candidate.Reset();
                SemanticState.StatusLabel = FString::Printf(
                    TEXT("GLOBAL PATH REJECTED · %s"), *Error);
                SemanticState.bAcceptanceEligible = false;
                break;
            }
            ChunkCount = Chunk.ChunkCount;
            Candidate.Append(Chunk.Points);
            ++ChunkIndex;
        }
        if (!Candidate.IsEmpty()) GlobalPaths[Source - 1] = MoveTemp(Candidate);
    }
    GlobalPathDisplayFrame = static_cast<uint8>(RequestedFrame);
    LastGlobalPathRefreshRelease = CurrentSample.ReleaseEpoch;
}

void UKsa64GlobalViewerSubsystem::ApplyAcceptedEarthDefinition()
{
    if (!bGlobalDefinitionValid) return;
    EarthSemiMajorCentimetres =
        static_cast<double>(GlobalDefinition.SemiMajorQ12Km) * 100'000.0 / 4'096.0;
    EarthSemiMinorCentimetres =
        static_cast<double>(GlobalDefinition.SemiMinorQ12Km) * 100'000.0 / 4'096.0;
    if (EarthMesh.IsValid())
    {
        EarthMesh->SetRelativeScale3D(FVector(
            EarthSemiMajorCentimetres / 50.0,
            EarthSemiMajorCentimetres / 50.0,
            EarthSemiMinorCentimetres / 50.0));
    }
    if (AtmosphereMesh.IsValid())
    {
        AtmosphereMesh->SetRelativeScale3D(FVector(
            EarthSemiMajorCentimetres * 1.012 / 50.0,
            EarthSemiMajorCentimetres * 1.012 / 50.0,
            EarthSemiMinorCentimetres * 1.012 / 50.0));
    }
    BuildEarthGrid();
}

const FKsa64GlobalSourcePoseProduct*
UKsa64GlobalViewerSubsystem::FindGlobalSource(uint8 Source) const
{
    return CurrentGlobalProduct.Sources.FindByPredicate(
        [Source](const FKsa64GlobalSourcePoseProduct& Value)
        {
            return Value.Source == Source;
        });
}

const FKsa64GlobalResolvedPoseProduct*
UKsa64GlobalViewerSubsystem::ResolvePoseForCamera(
    const FKsa64GlobalSourcePoseProduct& Source) const
{
    switch (SemanticState.ResolvedCamera)
    {
    case EKsa64GlobalCameraMode::LaunchLocalEnu:
        return (Source.ValidityMask & (1u << 10)) != 0 ? &Source.LaunchEnu : nullptr;
    case EKsa64GlobalCameraMode::RecoveryLocalEnu:
        return (Source.ValidityMask & (1u << 13)) != 0 ? &Source.RecoveryEnu : nullptr;
    case EKsa64GlobalCameraMode::EarthFixed:
        return (Source.ValidityMask & (1u << 4)) != 0 ? &Source.Ecef : nullptr;
    case EKsa64GlobalCameraMode::EarthInertial:
    case EKsa64GlobalCameraMode::FreeOrbit:
        return (Source.ValidityMask & (1u << 7)) != 0 ? &Source.Gcrf : nullptr;
    default:
        return (Source.ValidityMask & (1u << 0)) != 0 ? &Source.Active : nullptr;
    }
}

void UKsa64GlobalViewerSubsystem::ApplyReplayDisposition()
{
    if (GlobalReplayIndex.IndexIdentity == 0) return;
    SemanticState.OverallDisposition = GlobalReplayIndex.TerminalDisposition;
    SemanticState.ObjectiveDisposition = GlobalReplayIndex.DispositionAxes[0];
    SemanticState.VehicleDisposition = GlobalReplayIndex.DispositionAxes[1];
    SemanticState.ProcedureDisposition = GlobalReplayIndex.DispositionAxes[2];
    SemanticState.OperatorDisposition = GlobalReplayIndex.DispositionAxes[3];
    SemanticState.AvionicsDisposition = GlobalReplayIndex.DispositionAxes[4];
    SemanticState.EvidenceDisposition = GlobalReplayIndex.DispositionAxes[5];
    SemanticState.DispositionLabel = Ksa64OperationsPolicy::OverallLabel(
        GlobalReplayIndex.TerminalDisposition);
    SemanticState.StatusLabel = bGlobalAcceptedExact
        ? TEXT("VERIFIED PHASE 10 NOMINAL REPLAY")
        : TEXT("NOMINAL REPLAY · NOT ACCEPTANCE EVIDENCE");
}

void UKsa64GlobalViewerSubsystem::RefreshSemanticState(
    const FKsa64GlobalSceneSample& Sample,
    const UKsa64LiveMissionSubsystem& Operations)
{
    const FKsa64OperationsViewModel& View = Operations.GetViewModel();
    SemanticState.DisplayAvailability = bGlobalDefinitionValid
        ? EKsa64GlobalDisplayAvailability::GlobalDisplayV1
        : EKsa64GlobalDisplayAvailability::ActiveFrameFallback;
    SemanticState.bAcceptanceEligible =
        bGlobalDefinitionValid && bGlobalAcceptedExact;
    SemanticState.ReleaseEpoch = Sample.ReleaseEpoch;
    SemanticState.MissionTimeQ16 = Sample.MissionTimeQ16;
    SemanticState.FrameIdentity = Sample.FrameIdentity;
    SemanticState.SegmentIdentity = Sample.SegmentIdentity;
    SemanticState.EventMask = Sample.EventMask;
    SemanticState.DiscontinuityMask = Sample.DiscontinuityMask;
    SemanticState.ContinuityIdentity = Sample.ContinuityIdentity;
    SemanticState.bExactSnap = Sample.bExactSnap;
    SemanticState.bAttitudeAvailable = Sample.bAttitudeValid;
    SemanticState.ReplayPace = ReplayPace;
    SemanticState.ReplayOldestRelease = ReplayOldestRelease;
    SemanticState.ReplayNewestRelease = ReplayNewestRelease;
    SemanticState.ReplaySelectedRelease = ReplaySelectedRelease;
    if (Operations.IsGlobalReplayMode()) ApplyReplayDisposition();
    SemanticState.FrameLabel =
        Ksa64GlobalViewerPolicy::FrameLabel(Sample.FrameIdentity);
    if (bGlobalDefinitionValid)
    {
        SemanticState.SourceMask = 0;
        for (const FKsa64GlobalSourcePoseProduct& Source : CurrentGlobalProduct.Sources)
        {
            SemanticState.SourceMask |= 1u << (Source.Source - 1u);
        }
        SemanticState.ObservedPathPoints = GlobalPaths[3].Num();
        SemanticState.PlannedPathPoints = GlobalPaths[0].Num();
        SemanticState.OnboardPathPoints = GlobalPaths[1].Num();
        SemanticState.GroundPathPoints = GlobalPaths[2].Num();
        SemanticState.TransitionMarkers = GlobalTransitions.Num();
        SemanticState.SourceLabel = SemanticState.bTruthVisible
            ? TEXT("SIM TRUTH · DIRECTOR ONLY")
            : TEXT("ONBOARD ESTIMATE");
    }
    else
    {
        SemanticState.SourceMask = (1u << 0) | (1u << 1);
        if (Sample.bGroundPositionValid) SemanticState.SourceMask |= 1u << 2;
        if (SemanticState.bTruthPermitted) SemanticState.SourceMask |= 1u << 3;
        SemanticState.ObservedPathPoints = Operations.GetReleaseHistory().Num();
        SemanticState.PlannedPathPoints = Operations.GetPlannedReferencePath().Num();
        SemanticState.OnboardPathPoints = Operations.GetOnboardPredictionPath().Num();
        SemanticState.GroundPathPoints = Operations.GetGroundPredictionPath().Num();
        uint32 Transitions = 0;
        uint32 PreviousFrame = 0;
        for (const FKsa64OperationsReleasePoint& Point : Operations.GetReleaseHistory())
        {
            if (PreviousFrame != 0 && Point.FrameIdentity != PreviousFrame) ++Transitions;
            if (Point.FrameIdentity != 0) PreviousFrame = Point.FrameIdentity;
        }
        SemanticState.TransitionMarkers = Transitions;
    }
    SemanticState.ResolvedCamera =
        SemanticState.RequestedCamera == EKsa64GlobalCameraMode::AutomaticDirector
        && !SemanticState.bAutoDirectorSuspended
            ? Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(
                Sample.FrameIdentity,
                Sample.SegmentIdentity,
                Sample.ReleaseEpoch)
            : SemanticState.RequestedCamera;
}

void UKsa64GlobalViewerSubsystem::UpdateDisplayOrigin(
    const FKsa64GlobalSceneSample& Sample)
{
    const int64 PreviousOrigin[3] = {
        SemanticState.DisplayOriginQ12Km[0],
        SemanticState.DisplayOriginQ12Km[1],
        SemanticState.DisplayOriginQ12Km[2]};
    const bool bEarthCentred =
        SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::EarthFixed
        || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::EarthInertial
        || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::FreeOrbit;
    const int32* Position = Sample.PositionQ12Km;
    if (bGlobalDefinitionValid)
    {
        const FKsa64GlobalSourcePoseProduct* Source = FindGlobalSource(2);
        if (Source == nullptr) Source = FindGlobalSource(1);
        if (Source != nullptr)
        {
            if (const FKsa64GlobalResolvedPoseProduct* Pose =
                    ResolvePoseForCamera(*Source))
            {
                Position = Pose->PositionQ12Km;
            }
        }
    }
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        SemanticState.DisplayOriginQ12Km[Axis] =
            bEarthCentred || !Sample.bPositionValid
                ? 0
                : Ksa64GlobalViewerPolicy::QuantizeOriginQ12(Position[Axis]);
    }
    if (bGlobalEvidenceMode
        && (PreviousOrigin[0] != SemanticState.DisplayOriginQ12Km[0]
            || PreviousOrigin[1] != SemanticState.DisplayOriginQ12Km[1]
            || PreviousOrigin[2] != SemanticState.DisplayOriginQ12Km[2]))
    {
        ++GlobalEvidenceOriginChanges;
    }
    ApplyOriginToStaticDomain();
}

uint64 UKsa64GlobalViewerSubsystem::ComputeOriginInvariant() const
{
    uint64 Hash = 1469598103934665603ull;
    const auto Mix = [&Hash](uint64 Value)
    {
        for (uint32 Shift = 0; Shift < 64; Shift += 8)
        {
            Hash ^= (Value >> Shift) & 0xffull;
            Hash *= 1099511628211ull;
        }
    };
    Mix(CurrentSample.ReleaseEpoch);
    Mix(CurrentSample.MissionTimeQ16);
    Mix(CurrentSample.FrameIdentity);
    Mix(CurrentSample.SegmentIdentity);
    Mix(CurrentSample.EventMask);
    Mix(CurrentSample.DiscontinuityMask);
    Mix(CurrentSample.ContinuityIdentity);
    for (const int32 Value : CurrentSample.PositionQ12Km) Mix(static_cast<uint32>(Value));
    for (const int32 Value : CurrentSample.GroundPositionQ12Km) Mix(static_cast<uint32>(Value));
    for (const int32 Value : CurrentSample.AttitudeQ30) Mix(static_cast<uint32>(Value));
    Mix(SemanticState.SourceMask);
    Mix(SemanticState.PlannedPathPoints);
    Mix(SemanticState.OnboardPathPoints);
    Mix(SemanticState.GroundPathPoints);
    Mix(SemanticState.ObservedPathPoints);
    Mix(SemanticState.TransitionMarkers);
    Mix(CurrentGlobalProduct.Sequence);
    Mix(CurrentGlobalProduct.ContinuityIdentity);
    for (const FKsa64GlobalSourcePoseProduct& Source : CurrentGlobalProduct.Sources)
    {
        Mix(Source.Source);
        Mix(Source.ActiveFrame);
        Mix(Source.ValidityMask);
        Mix(Source.ModelIdentity);
        Mix(Source.EstimateIdentity);
        Mix(Source.Checksum);
        for (const int32 Value : Source.Active.PositionQ12Km) Mix(static_cast<uint32>(Value));
        for (const int32 Value : Source.Ecef.PositionQ12Km) Mix(static_cast<uint32>(Value));
        for (const int32 Value : Source.Gcrf.PositionQ12Km) Mix(static_cast<uint32>(Value));
    }
    for (const TArray<FKsa64GlobalPathPointProduct>& Path : GlobalPaths)
    {
        Mix(Path.Num());
        for (const FKsa64GlobalPathPointProduct& Point : Path)
        {
            Mix(Point.ReleaseEpoch);
            Mix(Point.MissionTimeQ16);
            Mix(Point.Segment);
            Mix(Point.EventMask);
            Mix(Point.AnchorIdentity);
            for (const int32 Value : Point.PositionQ12Km) Mix(static_cast<uint32>(Value));
        }
    }
    return Hash;
}

void UKsa64GlobalViewerSubsystem::CollectRenderedAbsolutePoints(
    TArray<FVector3d>& OutPoints) const
{
    const int64 Zero[3] = {0, 0, 0};
    const FVector3d AbsoluteOrigin =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            SemanticState.DisplayOriginQ12Km,
            Zero);
    const auto AppendComponent = [&OutPoints, &AbsoluteOrigin](
        const auto& Component)
    {
        if (Component.IsValid() && Component->IsVisible())
        {
            OutPoints.Add(FVector3d(Component->GetComponentLocation()) + AbsoluteOrigin);
        }
    };
    AppendComponent(EarthMesh);
    AppendComponent(LocatorMesh);
    AppendComponent(GroundLocatorMesh);
    AppendComponent(TruthLocatorMesh);
    const auto AppendLines = [&OutPoints, &AbsoluteOrigin](
        const TWeakObjectPtr<UKsa64GlobalLineComponent>& Lines,
        int32 MaximumSamples)
    {
        if (!Lines.IsValid() || !Lines->IsVisible())
        {
            return;
        }
        TArray<FVector3d> Samples;
        Lines->AppendWorldSamplePoints(Samples, MaximumSamples);
        for (const FVector3d& Sample : Samples)
        {
            OutPoints.Add(Sample + AbsoluteOrigin);
        }
    };
    AppendLines(PlannedPathLines, 16);
    AppendLines(OnboardPathLines, 16);
    AppendLines(GroundPathLines, 16);
    AppendLines(ObservedPathLines, 16);
    AppendLines(TransitionMarkerLines, 32);
}

bool UKsa64GlobalViewerSubsystem::ValidateGlobalEvidenceOriginContinuity()
{
    const EKsa64GlobalCameraMode Requested = SemanticState.RequestedCamera;
    const EKsa64GlobalCameraMode Resolved = SemanticState.ResolvedCamera;
    const bool bSuspended = SemanticState.bAutoDirectorSuspended;
    const uint64 BeforeInvariant = ComputeOriginInvariant();

    SemanticState.ResolvedCamera = EKsa64GlobalCameraMode::EarthFixed;
    UpdateDisplayOrigin(CurrentSample);
    UpdateVehicle(CurrentSample);
    UpdatePaths(CurrentSample.FrameIdentity);
    const int64 EarthOrigin[3] = {
        SemanticState.DisplayOriginQ12Km[0],
        SemanticState.DisplayOriginQ12Km[1],
        SemanticState.DisplayOriginQ12Km[2]};
    TArray<FVector3d> EarthPoints;
    CollectRenderedAbsolutePoints(EarthPoints);

    SemanticState.ResolvedCamera = EKsa64GlobalCameraMode::VehicleChase;
    UpdateDisplayOrigin(CurrentSample);
    UpdateVehicle(CurrentSample);
    UpdatePaths(CurrentSample.FrameIdentity);
    const bool bChanged = EarthOrigin[0] != SemanticState.DisplayOriginQ12Km[0]
        || EarthOrigin[1] != SemanticState.DisplayOriginQ12Km[1]
        || EarthOrigin[2] != SemanticState.DisplayOriginQ12Km[2];
    TArray<FVector3d> ChasePoints;
    CollectRenderedAbsolutePoints(ChasePoints);

    const bool bSemanticUnchanged = BeforeInvariant == ComputeOriginInvariant();
    double MaximumDeltaCm = 0.0;
    bool bRenderedContinuous = EarthPoints.Num() >= 8
        && EarthPoints.Num() == ChasePoints.Num();
    if (bRenderedContinuous)
    {
        for (int32 Index = 0; Index < EarthPoints.Num(); ++Index)
        {
            MaximumDeltaCm = FMath::Max(
                MaximumDeltaCm,
                FVector3d::Distance(EarthPoints[Index], ChasePoints[Index]));
        }
        bRenderedContinuous = MaximumDeltaCm <= 100.0;
    }

    ++GlobalEvidenceOriginContinuityChecks;
    GlobalEvidenceOriginRenderedSamples = EarthPoints.Num();
    GlobalEvidenceOriginMaximumDeltaCm = MaximumDeltaCm;
    bGlobalEvidenceOriginSemanticUnchanged &= bSemanticUnchanged;
    bGlobalEvidenceOriginRenderedContinuity &= bRenderedContinuous;
    bGlobalEvidenceOriginContinuityValid &=
        bChanged && bSemanticUnchanged && bRenderedContinuous;

    SemanticState.RequestedCamera = Requested;
    SemanticState.ResolvedCamera = Resolved;
    SemanticState.bAutoDirectorSuspended = bSuspended;
    UpdateDisplayOrigin(CurrentSample);
    UpdateVehicle(CurrentSample);
    UpdatePaths(CurrentSample.FrameIdentity);
    return bChanged && bSemanticUnchanged && bRenderedContinuous;
}

void UKsa64GlobalViewerSubsystem::ApplyOriginToStaticDomain()
{
    const int64 EarthCentre[3] = {0, 0, 0};
    const FVector3d EarthRelative =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            EarthCentre,
            SemanticState.DisplayOriginQ12Km);
    if (EarthMesh.IsValid()) EarthMesh->SetRelativeLocation(EarthRelative);
    if (AtmosphereMesh.IsValid()) AtmosphereMesh->SetRelativeLocation(EarthRelative);
    if (EarthGridLines.IsValid()) EarthGridLines->SetRelativeLocation(EarthRelative);
}

void UKsa64GlobalViewerSubsystem::UpdateScene(
    const FKsa64GlobalSceneSample& Sample)
{
    UpdateEarthAndLocalDomain(Sample.FrameIdentity);
    UpdateVehicle(Sample);
    UpdatePaths(Sample.FrameIdentity);
    UpdateCamera(Sample, 0.0f);
}

void UKsa64GlobalViewerSubsystem::UpdateEarthAndLocalDomain(uint32 FrameIdentity)
{
    const bool bExplicitLocal =
        SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::LaunchLocalEnu
        || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::RecoveryLocalEnu;
    const bool bExplicitEarth =
        SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::EarthFixed
        || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::EarthInertial
        || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::FreeOrbit;
    const bool bLocal = bExplicitLocal || (!bExplicitEarth && FrameIdentity == 1);
    if (EarthMesh.IsValid()) EarthMesh->SetVisibility(!bLocal);
    if (AtmosphereMesh.IsValid()) AtmosphereMesh->SetVisibility(!bLocal);
    if (EarthGridLines.IsValid()) EarthGridLines->SetVisibility(!bLocal);
    if (LocalGridLines.IsValid()) LocalGridLines->SetVisibility(bLocal);
    if (AxisXLines.IsValid()) AxisXLines->SetVisibility(true);
    if (AxisYLines.IsValid()) AxisYLines->SetVisibility(true);
    if (AxisZLines.IsValid()) AxisZLines->SetVisibility(true);
}

void UKsa64GlobalViewerSubsystem::UpdateVehicle(
    const FKsa64GlobalSceneSample& Sample)
{
    const int32* Position = Sample.PositionQ12Km;
    const int32* Attitude = Sample.AttitudeQ30;
    bool bPositionValid = Sample.bPositionValid;
    bool bAttitudeValid = Sample.bAttitudeValid;
    if (bGlobalDefinitionValid)
    {
        const FKsa64GlobalSourcePoseProduct* Source = FindGlobalSource(2);
        if (Source == nullptr) Source = FindGlobalSource(1);
        const FKsa64GlobalResolvedPoseProduct* Pose =
            Source != nullptr ? ResolvePoseForCamera(*Source) : nullptr;
        bPositionValid = Pose != nullptr;
        if (Pose != nullptr)
        {
            Position = Pose->PositionQ12Km;
            Attitude = Pose->AttitudeQ30;
            bAttitudeValid = Attitude[0] != 0 || Attitude[1] != 0
                || Attitude[2] != 0 || Attitude[3] != 0;
        }
    }
    SemanticState.bAttitudeAvailable = bAttitudeValid;
    if (!bPositionValid || !VehicleBodyMesh.IsValid())
    {
        if (VehicleBodyMesh.IsValid())
            VehicleBodyMesh->GetOwner()->SetActorHiddenInGame(true);
        if (LocatorMesh.IsValid()) LocatorMesh->SetVisibility(false);
        if (GroundLocatorMesh.IsValid()) GroundLocatorMesh->SetVisibility(false);
        if (TruthLocatorMesh.IsValid()) TruthLocatorMesh->SetVisibility(false);
        return;
    }
    const FVector3d Relative =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            Position, SemanticState.DisplayOriginQ12Km);
    AActor* Vehicle = VehicleBodyMesh->GetOwner();
    Vehicle->SetActorHiddenInGame(false);
    // Reflecting KSA's +Y axis once maps the right-handed body-to-frame
    // quaternion into Unreal without giving Unreal ownership of attitude.
    if (bAttitudeValid)
    {
        Vehicle->SetActorRotation(
            Ksa64GlobalViewerPolicy::Ksa64BodyToFrameQuaternionToUnreal(
                Attitude));
    }
    else
    {
        Vehicle->SetActorRotation(FQuat::Identity);
    }
    if (LocatorMesh.IsValid())
    {
        LocatorMesh->SetVisibility(true);
        LocatorMesh->SetRelativeLocation(Relative);
    }
    const auto UpdateGhost = [this](
        TWeakObjectPtr<UStaticMeshComponent>& Marker,
        uint8 SourceIdentity,
        bool bVisible)
    {
        if (!Marker.IsValid()) return;
        const FKsa64GlobalSourcePoseProduct* Source = FindGlobalSource(SourceIdentity);
        const FKsa64GlobalResolvedPoseProduct* Pose =
            Source != nullptr ? ResolvePoseForCamera(*Source) : nullptr;
        Marker->SetVisibility(bVisible && Pose != nullptr);
        if (bVisible && Pose != nullptr)
        {
            Marker->SetRelativeLocation(
                Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                    Pose->PositionQ12Km,
                    SemanticState.DisplayOriginQ12Km));
        }
    };
    UpdateGhost(GroundLocatorMesh, 3, true);
    UpdateGhost(TruthLocatorMesh, 4, SemanticState.bTruthVisible);
}

void UKsa64GlobalViewerSubsystem::UpdatePaths(uint32 ActiveFrame)
{
    const UKsa64LiveMissionSubsystem* Operations = GetOperations();
    if (Operations == nullptr)
    {
        return;
    }
    if (bGlobalDefinitionValid)
    {
        const auto ConvertGlobal = [this](
            const TArray<FKsa64GlobalPathPointProduct>& Points,
            bool bDashed = false)
        {
            TArray<FVector3d> Segments;
            Segments.Reserve(FMath::Min(Points.Num(), MaximumPathSegments) * 2);
            for (int32 Index = 1; Index < Points.Num(); ++Index)
            {
                if (GlobalPathDisplayFrame == 1
                    && Points[Index - 1].AnchorIdentity != Points[Index].AnchorIdentity)
                {
                    continue;
                }
                if (bDashed && (Index % 2) == 0) continue;
                AddSegment(
                    Segments,
                    Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                        Points[Index - 1].PositionQ12Km,
                        SemanticState.DisplayOriginQ12Km),
                    Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                        Points[Index].PositionQ12Km,
                        SemanticState.DisplayOriginQ12Km));
                if (Segments.Num() / 2 >= MaximumPathSegments) break;
            }
            return Segments;
        };
        if (ObservedPathLines.IsValid())
        {
            ObservedPathLines->SetSegments(
                SemanticState.bTruthVisible
                    ? ConvertGlobal(GlobalPaths[3])
                    : TArray<FVector3d>{},
                FLinearColor(0.31f, 0.93f, 0.57f, 1.0f),
                2.5f);
        }
        if (PlannedPathLines.IsValid())
            PlannedPathLines->SetSegments(
                ConvertGlobal(GlobalPaths[0], true),
                FLinearColor(0.90f, 0.92f, 0.95f, 0.75f), 1.0f);
        if (OnboardPathLines.IsValid())
            OnboardPathLines->SetSegments(
                ConvertGlobal(GlobalPaths[1]),
                FLinearColor(0.14f, 0.83f, 0.95f, 0.9f), 1.75f);
        if (GroundPathLines.IsValid())
            GroundPathLines->SetSegments(
                ConvertGlobal(GlobalPaths[2]),
                FLinearColor(1.0f, 0.66f, 0.18f, 0.9f), 1.75f);
        if (TransitionMarkerLines.IsValid())
        {
            TArray<FVector3d> Markers;
            const bool bLocalDetail =
                SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::LaunchLocalEnu
                || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::RecoveryLocalEnu
                || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::VehicleChase
                || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::TrueScaleInspection;
            const double Radius = bLocalDetail ? 5'000.0 : 2'000'000.0;
            for (const FKsa64GlobalTransitionProduct& Transition : GlobalTransitions)
            {
                const FKsa64GlobalPathPointProduct* Point =
                    GlobalPaths[1].FindByPredicate([&Transition](
                        const FKsa64GlobalPathPointProduct& Candidate)
                    {
                        return Candidate.ReleaseEpoch == Transition.ReleaseEpoch;
                    });
                if (Point == nullptr) continue;
                const FVector3d Centre =
                    Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                        Point->PositionQ12Km,
                        SemanticState.DisplayOriginQ12Km);
                AddSegment(Markers, Centre - FVector3d(Radius, 0, 0), Centre + FVector3d(Radius, 0, 0));
                AddSegment(Markers, Centre - FVector3d(0, Radius, 0), Centre + FVector3d(0, Radius, 0));
                AddSegment(Markers, Centre - FVector3d(0, 0, Radius), Centre + FVector3d(0, 0, Radius));
            }
            TransitionMarkerLines->SetSegments(
                Markers,
                FLinearColor(0.95f, 0.40f, 0.82f, 0.95f),
                2.0f);
        }
        return;
    }
    const auto ConvertHistory = [this, ActiveFrame](
        const TArray<FKsa64OperationsReleasePoint>& Points,
        bool bGround)
    {
        TArray<FVector3d> Segments;
        Segments.Reserve(FMath::Min(Points.Num(), MaximumPathSegments) * 2);
        const FKsa64OperationsReleasePoint* Previous = nullptr;
        for (const FKsa64OperationsReleasePoint& Point : Points)
        {
            const bool bValid = bGround
                ? Point.bHasGroundEstimate
                : Point.bHasPosition;
            if (!bValid || Point.FrameIdentity != ActiveFrame)
            {
                Previous = nullptr;
                continue;
            }
            if (Previous != nullptr && Previous->FrameIdentity == Point.FrameIdentity)
            {
                const int32* PreviousPosition = bGround
                    ? Previous->GroundPositionQ12
                    : Previous->PositionQ12;
                const int32* CurrentPosition = bGround
                    ? Point.GroundPositionQ12
                    : Point.PositionQ12;
                AddSegment(
                    Segments,
                    Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                        PreviousPosition,
                        SemanticState.DisplayOriginQ12Km),
                    Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                        CurrentPosition,
                        SemanticState.DisplayOriginQ12Km));
            }
            Previous = &Point;
            if (Segments.Num() / 2 >= MaximumPathSegments)
            {
                break;
            }
        }
        return Segments;
    };
    const auto ConvertPrediction = [this, ActiveFrame](
        const TArray<FKsa64OperationsPredictionPoint>& Points)
    {
        TArray<FVector3d> Segments;
        Segments.Reserve(FMath::Min(Points.Num(), MaximumPathSegments) * 2);
        const FKsa64OperationsPredictionPoint* Previous = nullptr;
        for (const FKsa64OperationsPredictionPoint& Point : Points)
        {
            if (Point.FrameIdentity != ActiveFrame)
            {
                Previous = nullptr;
                continue;
            }
            if (Previous != nullptr
                && Previous->FrameIdentity == Point.FrameIdentity
                && Previous->PathIdentity == Point.PathIdentity)
            {
                AddSegment(
                    Segments,
                    Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                        Previous->PositionQ12Km,
                        SemanticState.DisplayOriginQ12Km),
                    Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
                        Point.PositionQ12Km,
                        SemanticState.DisplayOriginQ12Km));
            }
            Previous = &Point;
        }
        return Segments;
    };
    if (ObservedPathLines.IsValid())
    {
        ObservedPathLines->SetSegments(
            ConvertHistory(Operations->GetReleaseHistory(), false),
            FLinearColor(0.31f, 0.93f, 0.57f, 1.0f),
            2.5f);
    }
    if (GroundPathLines.IsValid())
    {
        TArray<FVector3d> Segments =
            ConvertPrediction(Operations->GetGroundPredictionPath());
        Segments.Append(ConvertHistory(Operations->GetReleaseHistory(), true));
        GroundPathLines->SetSegments(
            Segments,
            FLinearColor(1.0f, 0.66f, 0.18f, 0.9f),
            1.75f);
    }
    if (PlannedPathLines.IsValid())
    {
        PlannedPathLines->SetSegments(
            ConvertPrediction(Operations->GetPlannedReferencePath()),
            FLinearColor(0.90f, 0.92f, 0.95f, 0.75f),
            1.0f);
    }
    if (OnboardPathLines.IsValid())
    {
        OnboardPathLines->SetSegments(
            ConvertPrediction(Operations->GetOnboardPredictionPath()),
            FLinearColor(0.14f, 0.83f, 0.95f, 0.9f),
            1.75f);
    }
}

void UKsa64GlobalViewerSubsystem::UpdateCamera(
    const FKsa64GlobalSceneSample& Sample,
    float DeltaSeconds)
{
    const int32* Position = Sample.PositionQ12Km;
    bool bPositionValid = Sample.bPositionValid;
    if (bGlobalDefinitionValid)
    {
        const FKsa64GlobalSourcePoseProduct* Source = FindGlobalSource(2);
        if (Source == nullptr) Source = FindGlobalSource(1);
        const FKsa64GlobalResolvedPoseProduct* Pose =
            Source != nullptr ? ResolvePoseForCamera(*Source) : nullptr;
        bPositionValid = Pose != nullptr;
        if (Pose != nullptr) Position = Pose->PositionQ12Km;
    }
    if (!ViewerCamera.IsValid() || !bPositionValid)
    {
        return;
    }
    const FVector3d Target =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            Position, SemanticState.DisplayOriginQ12Km);
    const int64 EarthCentreQ12[3] = {0, 0, 0};
    const FVector3d EarthCentre =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            EarthCentreQ12,
            SemanticState.DisplayOriginQ12Km);
    FVector3d Desired;
    switch (SemanticState.ResolvedCamera)
    {
    case EKsa64GlobalCameraMode::LaunchLocalEnu:
    case EKsa64GlobalCameraMode::RecoveryLocalEnu:
        Desired = Target + FVector3d(-250'000.0, 450'000.0, 220'000.0);
        break;
    case EKsa64GlobalCameraMode::VehicleChase:
        Desired = Target + FVector3d(-150'000.0, 220'000.0, 90'000.0);
        break;
    case EKsa64GlobalCameraMode::TrueScaleInspection:
        Desired = Target + FVector3d(-3'000.0, 2'000.0, 1'200.0);
        break;
    case EKsa64GlobalCameraMode::EarthFixed:
    case EKsa64GlobalCameraMode::EarthInertial:
    case EKsa64GlobalCameraMode::FreeOrbit:
    default:
    {
        FVector3d Direction = Target - EarthCentre;
        if (Direction.IsNearlyZero())
        {
            Direction = FVector3d(1.0, -1.0, 0.55);
        }
        Direction.Normalize();
        Desired = EarthCentre
            + Direction * (EarthSemiMajorCentimetres * 1.85)
            + FVector3d(0.0, 0.0, EarthSemiMajorCentimetres * 0.18);
        break;
    }
    }
    const bool bSnap = Sample.bExactSnap
        || SemanticState.Layout == EKsa64GlobalViewerLayout::CinematicFullscreen
        || LastCameraLocation.IsNearlyZero();
    const double Alpha = bSnap
        ? 1.0
        : 1.0 - FMath::Exp(-FMath::Max(0.0f, DeltaSeconds) * 4.0);
    LastCameraLocation = FMath::Lerp(LastCameraLocation, Desired, Alpha);
    ViewerCamera->SetActorLocation(LastCameraLocation);
    ViewerCamera->SetActorRotation((Target - LastCameraLocation).Rotation());

    // These explicitly labelled locators remain readable at planetary scale.
    // The separate schematic vehicle actor remains physically scaled.
    const double Distance = FMath::Max(1'000.0, (LastCameraLocation - Target).Length());
    const double LocatorScale = FMath::Clamp(Distance * 0.00003, 0.08, 50'000.0);
    if (LocatorMesh.IsValid()) LocatorMesh->SetRelativeScale3D(FVector(LocatorScale));
    if (GroundLocatorMesh.IsValid()) GroundLocatorMesh->SetRelativeScale3D(FVector(LocatorScale * 0.80));
    if (TruthLocatorMesh.IsValid()) TruthLocatorMesh->SetRelativeScale3D(FVector(LocatorScale * 0.84));
}

void UKsa64GlobalViewerSubsystem::CycleLayout()
{
    switch (SemanticState.Layout)
    {
    case EKsa64GlobalViewerLayout::HybridMissionDirector:
        SetLayout(EKsa64GlobalViewerLayout::EngineeringSplit);
        break;
    case EKsa64GlobalViewerLayout::EngineeringSplit:
        SetLayout(EKsa64GlobalViewerLayout::CinematicFullscreen);
        break;
    default:
        SetLayout(EKsa64GlobalViewerLayout::HybridMissionDirector);
        break;
    }
}

void UKsa64GlobalViewerSubsystem::SetLayout(EKsa64GlobalViewerLayout Layout)
{
    SemanticState.Layout = Layout;
}

void UKsa64GlobalViewerSubsystem::CycleCamera()
{
    uint8 Camera = static_cast<uint8>(SemanticState.RequestedCamera);
    Camera = Camera >= static_cast<uint8>(EKsa64GlobalCameraMode::TrueScaleInspection)
        ? static_cast<uint8>(EKsa64GlobalCameraMode::AutomaticDirector)
        : Camera + 1;
    SetCamera(static_cast<EKsa64GlobalCameraMode>(Camera));
}

void UKsa64GlobalViewerSubsystem::SetCamera(EKsa64GlobalCameraMode Camera)
{
    SemanticState.RequestedCamera = Camera;
    SemanticState.bAutoDirectorSuspended =
        Camera != EKsa64GlobalCameraMode::AutomaticDirector;
    SemanticState.ResolvedCamera =
        Camera == EKsa64GlobalCameraMode::AutomaticDirector
            ? Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(
                CurrentSample.FrameIdentity,
                CurrentSample.SegmentIdentity,
                CurrentSample.ReleaseEpoch)
            : Camera;
    UpdateDisplayOrigin(CurrentSample);
    if (bGlobalDefinitionValid)
    {
        if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
            RefreshGlobalPaths(*Operations);
    }
    UpdateScene(CurrentSample);
}

void UKsa64GlobalViewerSubsystem::ResumeAutomaticDirector()
{
    SemanticState.RequestedCamera = EKsa64GlobalCameraMode::AutomaticDirector;
    SemanticState.bAutoDirectorSuspended = false;
    SemanticState.ResolvedCamera =
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(
            CurrentSample.FrameIdentity,
            CurrentSample.SegmentIdentity,
            CurrentSample.ReleaseEpoch);
    UpdateDisplayOrigin(CurrentSample);
    if (bGlobalDefinitionValid)
    {
        if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
            RefreshGlobalPaths(*Operations);
    }
    UpdateScene(CurrentSample);
}

void UKsa64GlobalViewerSubsystem::ToggleOperationsDesk()
{
    bOperationsDeskVisible = !bOperationsDeskVisible;
    SemanticState.bOperationsDeskVisible = bOperationsDeskVisible;
    if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
    {
        Operations->SetDashboardVisible(bOperationsDeskVisible);
    }
}

void UKsa64GlobalViewerSubsystem::ToggleTruth()
{
    if (!SemanticState.bTruthPermitted)
    {
        bTruthRequested = false;
        SemanticState.bTruthVisible = false;
        return;
    }
    bTruthRequested = !bTruthRequested;
    SemanticState.bTruthVisible = bTruthRequested;
    UpdateDisplayOrigin(CurrentSample);
    if (bGlobalDefinitionValid)
    {
        if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
            RefreshGlobalPaths(*Operations);
    }
    UpdateScene(CurrentSample);
}

void UKsa64GlobalViewerSubsystem::TogglePause()
{
    if (IsNominalReplay())
    {
        ReplayPace = ReplayPace == EKsa64GlobalReplayPace::Paused
            ? EKsa64GlobalReplayPace::One
            : EKsa64GlobalReplayPace::Paused;
        ReplayReleaseAccumulator = 0.0;
        SemanticState.ReplayPace = ReplayPace;
        return;
    }
    if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
    {
        if (Operations->GetViewModel().PresentationPace == EKsa64OperationsPace::Paused)
            Operations->ResumeRealtime();
        else
            Operations->PausePresentation();
    }
}

void UKsa64GlobalViewerSubsystem::StepOneRelease()
{
    if (IsNominalReplay())
    {
        ReplayPace = EKsa64GlobalReplayPace::Paused;
        SemanticState.ReplayPace = ReplayPace;
        SeekReplayRelease(ReplaySelectedRelease < ReplayNewestRelease
            ? ReplaySelectedRelease + 1u
            : ReplayNewestRelease);
        return;
    }
    if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
        Operations->StepOneRelease();
}

void UKsa64GlobalViewerSubsystem::CycleReplayPace()
{
    if (!IsNominalReplay()) return;
    switch (ReplayPace)
    {
    case EKsa64GlobalReplayPace::Paused: ReplayPace = EKsa64GlobalReplayPace::One; break;
    case EKsa64GlobalReplayPace::Quarter: ReplayPace = EKsa64GlobalReplayPace::Half; break;
    case EKsa64GlobalReplayPace::Half: ReplayPace = EKsa64GlobalReplayPace::One; break;
    case EKsa64GlobalReplayPace::One: ReplayPace = EKsa64GlobalReplayPace::Two; break;
    case EKsa64GlobalReplayPace::Two: ReplayPace = EKsa64GlobalReplayPace::Four; break;
    case EKsa64GlobalReplayPace::Four: ReplayPace = EKsa64GlobalReplayPace::Eight; break;
    case EKsa64GlobalReplayPace::Eight: ReplayPace = EKsa64GlobalReplayPace::Sixteen; break;
    case EKsa64GlobalReplayPace::Sixteen: ReplayPace = EKsa64GlobalReplayPace::Unpaced; break;
    case EKsa64GlobalReplayPace::Unpaced: ReplayPace = EKsa64GlobalReplayPace::Quarter; break;
    }
    ReplayReleaseAccumulator = 0.0;
    SemanticState.ReplayPace = ReplayPace;
}

void UKsa64GlobalViewerSubsystem::JumpToPreviousBookmark()
{
    if (!IsNominalReplay()) return;
    uint32 Target = ReplayOldestRelease;
    for (const FKsa64GlobalReplayEntryProduct& Entry : GlobalReplayIndex.Entries)
    {
        if (Entry.ReleaseEpoch >= ReplaySelectedRelease) break;
        Target = Entry.ReleaseEpoch;
    }
    SeekReplayRelease(Target);
}

void UKsa64GlobalViewerSubsystem::JumpToNextBookmark()
{
    if (!IsNominalReplay()) return;
    uint32 Target = ReplayNewestRelease;
    for (const FKsa64GlobalReplayEntryProduct& Entry : GlobalReplayIndex.Entries)
    {
        if (Entry.ReleaseEpoch > ReplaySelectedRelease)
        {
            Target = Entry.ReleaseEpoch;
            break;
        }
    }
    SeekReplayRelease(Target);
}

FString UKsa64GlobalViewerSubsystem::ExportSemanticStateJson() const
{
    return SemanticState.ToDeterministicJson();
}

FText UKsa64GlobalViewerSubsystem::GetStatusText() const
{
    return FText::FromString(FString::Printf(
        TEXT("KSA-G10R  ·  RELEASE %u  ·  %s  ·  %s"),
        SemanticState.ReleaseEpoch,
        *SemanticState.FrameLabel,
        *SemanticState.StatusLabel));
}

FText UKsa64GlobalViewerSubsystem::GetCameraText() const
{
    return FText::FromString(
        Ksa64GlobalViewerPolicy::CameraLabel(SemanticState.ResolvedCamera));
}

FText UKsa64GlobalViewerSubsystem::GetLayoutText() const
{
    return FText::FromString(
        Ksa64GlobalViewerPolicy::LayoutLabel(SemanticState.Layout));
}

FText UKsa64GlobalViewerSubsystem::GetSourceLegendText() const
{
    return FText::FromString(FString::Printf(
        TEXT("SOLID  ONBOARD ESTIMATE\n"
             "GHOST  GROUND ESTIMATE\n"
             "DASHED  PLANNED REFERENCE\n"
             "%s\n\n"
             "ATTITUDE  %s\n"
             "DISPLAY PRODUCT  %s\n\n"
             "Unreal owns cameras, interpolation, and renderer origins only."),
        SemanticState.bTruthVisible
            ? TEXT("SIM TRUTH  ENABLED / DIRECTOR ONLY")
            : TEXT("SIM TRUTH  HIDDEN OR STRUCTURALLY ABSENT"),
        SemanticState.bAttitudeAvailable
            ? TEXT("ROLE-PERMITTED")
            : TEXT("UNAVAILABLE / NOT SYNTHESIZED"),
        SemanticState.DisplayAvailability
                == EKsa64GlobalDisplayAvailability::GlobalDisplayV1
            ? (SemanticState.bAcceptanceEligible
                ? TEXT("GLOBAL DISPLAY V1 · ACCEPTED EXACT")
                : TEXT("GLOBAL DISPLAY V1 · UNQUALIFIED"))
            : TEXT("ACTIVE-FRAME FALLBACK · NOT ACCEPTANCE EVIDENCE")));
}

FText UKsa64GlobalViewerSubsystem::GetPaceText() const
{
    if (!IsNominalReplay())
    {
        if (const UKsa64LiveMissionSubsystem* Operations = GetOperations())
            return Operations->GetPaceLabel();
        return FText::FromString(TEXT("PAUSED"));
    }
    const TCHAR* Label = TEXT("PAUSED");
    switch (ReplayPace)
    {
    case EKsa64GlobalReplayPace::Quarter: Label = TEXT("0.25x"); break;
    case EKsa64GlobalReplayPace::Half: Label = TEXT("0.5x"); break;
    case EKsa64GlobalReplayPace::One: Label = TEXT("1x"); break;
    case EKsa64GlobalReplayPace::Two: Label = TEXT("2x"); break;
    case EKsa64GlobalReplayPace::Four: Label = TEXT("4x"); break;
    case EKsa64GlobalReplayPace::Eight: Label = TEXT("8x"); break;
    case EKsa64GlobalReplayPace::Sixteen: Label = TEXT("16x"); break;
    case EKsa64GlobalReplayPace::Unpaced: Label = TEXT("UNPACED"); break;
    default: break;
    }
    return FText::FromString(Label);
}


bool UKsa64GlobalViewerSubsystem::PrepareGlobalEvidence()
{
    if (!IsQualifiedHexIdentity(GlobalEvidenceSourceCommit, 40)
        || GlobalEvidenceExecutableRelativePath.IsEmpty()
        || !FPaths::IsRelative(GlobalEvidenceExecutableRelativePath)
        || GlobalEvidenceExecutableRelativePath.Contains(TEXT(".."))
        || GlobalEvidenceExecutableBytes == 0
        || GlobalEvidencePackagedDirectoryBytes <= GlobalEvidenceExecutableBytes
        || GlobalEvidencePackagedDirectoryFiles < 2
        || !IsQualifiedHexIdentity(GlobalEvidencePackagedDirectoryTreeSha256, 64)
        || GlobalEvidencePackagedDirectoryInventoryFile.IsEmpty()
        || FPaths::IsRelative(GlobalEvidencePackagedDirectoryInventoryFile) == false
        || !IsQualifiedHexIdentity(GlobalEvidencePackagedDirectoryInventorySha256, 64)
        || !IsQualifiedHexIdentity(GlobalEvidenceExecutableSha256, 64)
        || !IsQualifiedHexIdentity(GlobalEvidencePackageAuditSha256, 64))
    {
        FailGlobalEvidence(TEXT("qualified source, executable, and package-audit identities are required"));
        return false;
    }
    if (FParse::Param(FCommandLine::Get(), TEXT("Ksa64Phase12bAcceptance"))
        || FParse::Param(
            FCommandLine::Get(),
            TEXT("Ksa64Phase12bPresentationEvidence")))
    {
        FailGlobalEvidence(TEXT("Phase 12B and Phase 12C packaged evidence modes are mutually exclusive"));
        return false;
    }

    IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
    if (!PlatformFile.CreateDirectoryTree(*GlobalEvidenceDirectory))
    {
        FailGlobalEvidence(TEXT("global-viewer evidence directory creation failed"));
        return false;
    }
    if (PlatformFile.FileExists(*GlobalEvidenceManifestPath))
    {
        FailGlobalEvidence(TEXT("global-viewer evidence manifest path is not fresh"));
        return false;
    }
    for (const FKsa64GlobalEvidenceMilestone& Milestone : GlobalEvidenceMilestones)
    {
        const FString Base = FString::Printf(
            TEXT("phase12c-%s-%u"),
            Milestone.Label,
            Milestone.ReleaseEpoch);
        if (PlatformFile.FileExists(
                *FPaths::Combine(GlobalEvidenceDirectory, Base + TEXT("-semantic.json")))
            || PlatformFile.FileExists(
                *FPaths::Combine(GlobalEvidenceDirectory, Base + TEXT("-1920x1080.png"))))
        {
            FailGlobalEvidence(TEXT("global-viewer capture paths are not fresh"));
            return false;
        }
    }
    if (!StartNominalReplay())
    {
        FailGlobalEvidence(TEXT("verified nominal global replay could not start"));
        return false;
    }
    SetLayout(EKsa64GlobalViewerLayout::HybridMissionDirector);
    ResumeAutomaticDirector();
    ReplayPace = EKsa64GlobalReplayPace::Paused;
    SemanticState.ReplayPace = ReplayPace;
    bGlobalEvidencePrepared = true;
    GlobalEvidencePhase = 1;
    return true;
}

bool UKsa64GlobalViewerSubsystem::ValidateGlobalEvidenceState(
    uint32 ReleaseEpoch,
    uint32 FrameIdentity,
    uint32 SegmentIdentity,
    FString& OutReason) const
{
    if (SemanticState.ReleaseEpoch != ReleaseEpoch
        || SemanticState.ReplaySelectedRelease != ReleaseEpoch)
    {
        OutReason = FString::Printf(
            TEXT("release mismatch: selected=%u observed=%u expected=%u"),
            SemanticState.ReplaySelectedRelease,
            SemanticState.ReleaseEpoch,
            ReleaseEpoch);
        return false;
    }
    if (SemanticState.FrameIdentity != FrameIdentity
        || SemanticState.SegmentIdentity != SegmentIdentity)
    {
        OutReason = FString::Printf(
            TEXT("frame/segment mismatch at release %u: %u/%u expected %u/%u"),
            ReleaseEpoch,
            SemanticState.FrameIdentity,
            SemanticState.SegmentIdentity,
            FrameIdentity,
            SegmentIdentity);
        return false;
    }
    if (!SemanticState.bSceneReady
        || !SemanticState.bAcceptanceEligible
        || !bGlobalAcceptedExact
        || !SemanticState.bExactSnap
        || SemanticState.ExperienceMode != EKsa64GlobalExperienceMode::NominalReplay
        || SemanticState.DisplayAvailability
            != EKsa64GlobalDisplayAvailability::GlobalDisplayV1
        || SemanticState.ReplayOldestRelease != 0
        || SemanticState.ReplayNewestRelease != 22'014)
    {
        OutReason = TEXT("global replay is not exact, accepted, scene-ready, and fully indexed");
        return false;
    }
    if (SemanticState.SourceMask != 0x0bu
        || !SemanticState.bTruthPermitted
        || SemanticState.bTruthVisible
        || bTruthRequested
        || !SemanticState.RoleLabel.Contains(TEXT("SIM DIRECTOR")))
    {
        OutReason = FString::Printf(
            TEXT("SIM Director source policy failed: source_mask=%08X permitted=%u visible=%u requested=%u"),
            SemanticState.SourceMask,
            SemanticState.bTruthPermitted ? 1u : 0u,
            SemanticState.bTruthVisible ? 1u : 0u,
            bTruthRequested ? 1u : 0u);
        return false;
    }
    if (SemanticState.PlannedPathPoints == 0
        || SemanticState.OnboardPathPoints == 0
        || SemanticState.ObservedPathPoints == 0
        || SemanticState.TransitionMarkers < 4
        || SemanticState.OverallDisposition != 1
        || SemanticState.EvidenceDisposition != 1)
    {
        OutReason = FString::Printf(
            TEXT("derived display evidence is incomplete: paths=%u/%u/%u transitions=%u disposition=%u/%u"),
            SemanticState.PlannedPathPoints,
            SemanticState.OnboardPathPoints,
            SemanticState.ObservedPathPoints,
            SemanticState.TransitionMarkers,
            SemanticState.OverallDisposition,
            SemanticState.EvidenceDisposition);
        return false;
    }
    return true;
}

bool UKsa64GlobalViewerSubsystem::WriteGlobalEvidenceSemanticAndRequestScreenshot()
{
    if (GlobalEvidenceMilestoneIndex >= UE_ARRAY_COUNT(GlobalEvidenceMilestones))
    {
        FailGlobalEvidence(TEXT("global-viewer milestone index overflow"));
        return false;
    }
    const FKsa64GlobalEvidenceMilestone& Milestone =
        GlobalEvidenceMilestones[GlobalEvidenceMilestoneIndex];
    FString Reason;
    if (!ValidateGlobalEvidenceState(
            Milestone.ReleaseEpoch,
            Milestone.FrameIdentity,
            Milestone.SegmentIdentity,
            Reason))
    {
        FailGlobalEvidence(Reason);
        return false;
    }

    if (Milestone.ReleaseEpoch == 8'124
        && GlobalEvidenceOriginContinuityChecks == 0
        && !ValidateGlobalEvidenceOriginContinuity())
    {
        FailGlobalEvidence(TEXT("renderer-origin change altered absolute semantic state"));
        return false;
    }

    FKsa64GlobalEvidenceCapture Capture;
    Capture.Label = Milestone.Label;
    Capture.ReleaseEpoch = Milestone.ReleaseEpoch;
    Capture.FrameIdentity = SemanticState.FrameIdentity;
    Capture.SegmentIdentity = SemanticState.SegmentIdentity;
    Capture.SourceMask = SemanticState.SourceMask;
    Capture.TransitionMarkers = SemanticState.TransitionMarkers;
    Capture.PlannedPathPoints = SemanticState.PlannedPathPoints;
    Capture.OnboardPathPoints = SemanticState.OnboardPathPoints;
    Capture.ObservedPathPoints = SemanticState.ObservedPathPoints;
    const FString Base = FString::Printf(
        TEXT("phase12c-%s-%u"),
        Milestone.Label,
        Milestone.ReleaseEpoch);
    Capture.SemanticPath = FPaths::Combine(
        GlobalEvidenceDirectory,
        Base + TEXT("-semantic.json"));
    Capture.ScreenshotPath = FPaths::Combine(
        GlobalEvidenceDirectory,
        Base + TEXT("-1920x1080.png"));

    IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
    const FString TemporaryPath = Capture.SemanticPath + TEXT(".tmp");
    PlatformFile.DeleteFile(*TemporaryPath);
    if (!FFileHelper::SaveStringToFile(
            ExportSemanticStateJson(),
            *TemporaryPath,
            FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM)
        || !PlatformFile.MoveFile(*Capture.SemanticPath, *TemporaryPath))
    {
        PlatformFile.DeleteFile(*TemporaryPath);
        FailGlobalEvidence(TEXT("global-viewer semantic snapshot atomic write failed"));
        return false;
    }
    if (FScreenshotRequest::IsScreenshotRequested())
    {
        FailGlobalEvidence(TEXT("another screenshot request is already active"));
        return false;
    }
    bGlobalEvidenceScreenshotProcessed = false;
    GlobalEvidenceScreenshotWaitFrames = 0;
    if (GlobalEvidenceScreenshotProcessedHandle.IsValid())
    {
        FScreenshotRequest::OnScreenshotRequestProcessed().Remove(
            GlobalEvidenceScreenshotProcessedHandle);
    }
    GlobalEvidenceScreenshotProcessedHandle =
        FScreenshotRequest::OnScreenshotRequestProcessed().AddUObject(
            this,
            &UKsa64GlobalViewerSubsystem::OnGlobalEvidenceScreenshotProcessed);
    GlobalEvidenceCaptures.Add(MoveTemp(Capture));
    FScreenshotRequest::RequestScreenshot(
        GlobalEvidenceCaptures.Last().ScreenshotPath,
        true,
        false,
        false,
        FIntRect(),
        true);
    UE_LOG(
        LogKsa64GlobalViewer,
        Display,
        TEXT("KSA64_PHASE12C_GLOBAL_CAPTURE_REQUESTED label=%s release=%u path=%s"),
        Milestone.Label,
        Milestone.ReleaseEpoch,
        *GlobalEvidenceCaptures.Last().ScreenshotPath);
    return true;
}


bool UKsa64GlobalViewerSubsystem::QueueGlobalEvidenceGuidedAdvance(
    UKsa64LiveMissionSubsystem& Operations,
    uint32 TargetRelease)
{
    const uint32 CurrentRelease = Operations.GetViewModel().ReleaseEpoch;
    if (CurrentRelease > TargetRelease)
    {
        FailGlobalEvidence(FString::Printf(
            TEXT("guided evidence crossed release %u and reached %u"),
            TargetRelease,
            CurrentRelease));
        return false;
    }
    return Operations.QueueBoundedAdvanceToRelease(TargetRelease, 64);
}

bool UKsa64GlobalViewerSubsystem::WriteGlobalEvidenceGuidedRecord(
    const FString& Label,
    uint32 ExpectedGnssState,
    uint32 ExpectedReceiptState)
{
    UKsa64LiveMissionSubsystem* Operations = GetOperations();
    if (Operations == nullptr) return false;
    const FKsa64OperationsViewModel& View = Operations->GetViewModel();
    if (SemanticState.ReleaseEpoch != View.ReleaseEpoch
        || SemanticState.FrameIdentity != 3
        || SemanticState.SegmentIdentity != 3
        || SemanticState.SourceMask != 0x03u
        || SemanticState.bTruthPermitted
        || SemanticState.bTruthVisible
        || !SemanticState.bAcceptanceEligible
        || View.bTruthFiltered != true
        || View.GnssState != ExpectedGnssState
        || (ExpectedReceiptState != 0
            && (View.ActionReceiptState != ExpectedReceiptState
                || View.ActionReceiptAccepted == 0
                || View.ActionProposalIdentity == 0)))
    {
        FailGlobalEvidence(FString::Printf(
            TEXT("guided semantic state failed at release %u: frame=%u segment=%u sources=%08X truth=%u/%u gnss=%u receipt=%u/%u"),
            View.ReleaseEpoch,
            SemanticState.FrameIdentity,
            SemanticState.SegmentIdentity,
            SemanticState.SourceMask,
            SemanticState.bTruthPermitted ? 1u : 0u,
            SemanticState.bTruthVisible ? 1u : 0u,
            View.GnssState,
            View.ActionReceiptState,
            View.ActionReceiptAccepted));
        return false;
    }

    FKsa64GlobalGuidedEvidenceRecord Record;
    Record.Label = Label;
    Record.ReleaseEpoch = View.ReleaseEpoch;
    Record.FrameIdentity = SemanticState.FrameIdentity;
    Record.SegmentIdentity = SemanticState.SegmentIdentity;
    Record.SourceMask = SemanticState.SourceMask;
    Record.bTruthPermitted = SemanticState.bTruthPermitted;
    Record.bTruthVisible = SemanticState.bTruthVisible;
    Record.GnssState = View.GnssState;
    Record.ActionReceiptSequence = View.ActionReceiptSequence;
    Record.ActionReceiptState = View.ActionReceiptState;
    Record.ActionReceiptAccepted = View.ActionReceiptAccepted;
    Record.ActionProposalIdentity = View.ActionProposalIdentity;
    Record.OverallDisposition = SemanticState.OverallDisposition;
    Record.ObjectiveDisposition = SemanticState.ObjectiveDisposition;
    Record.VehicleDisposition = SemanticState.VehicleDisposition;
    Record.ProcedureDisposition = SemanticState.ProcedureDisposition;
    Record.OperatorDisposition = SemanticState.OperatorDisposition;
    Record.AvionicsDisposition = SemanticState.AvionicsDisposition;
    Record.EvidenceDisposition = SemanticState.EvidenceDisposition;
    Record.SemanticPath = FPaths::Combine(
        GlobalEvidenceDirectory,
        FString::Printf(
            TEXT("phase12c-guided-%s-%u-semantic.json"),
            *Label,
            View.ReleaseEpoch));

    FString Output;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Output);
    Writer->WriteObjectStart();
    Writer->WriteValue(TEXT("schema"), TEXT("ksa64.phase12c.unreal-guided-semantic.v1"));
    Writer->WriteValue(TEXT("label"), Record.Label);
    Writer->WriteValue(TEXT("release_epoch"), Record.ReleaseEpoch);
    Writer->WriteValue(TEXT("frame_identity"), Record.FrameIdentity);
    Writer->WriteValue(TEXT("segment_identity"), Record.SegmentIdentity);
    Writer->WriteValue(TEXT("source_mask"), Record.SourceMask);
    Writer->WriteValue(TEXT("truth_permitted"), Record.bTruthPermitted);
    Writer->WriteValue(TEXT("truth_visible"), Record.bTruthVisible);
    Writer->WriteValue(TEXT("gnss_state"), Record.GnssState);
    Writer->WriteValue(TEXT("gnss_reacquired"), false);
    Writer->WriteValue(TEXT("action_receipt_sequence"), FString::Printf(TEXT("%llu"), static_cast<unsigned long long>(Record.ActionReceiptSequence)));
    Writer->WriteValue(TEXT("action_receipt_state"), Record.ActionReceiptState);
    Writer->WriteValue(TEXT("action_receipt_accepted"), Record.ActionReceiptAccepted);
    Writer->WriteValue(TEXT("action_proposal_identity"), Record.ActionProposalIdentity);
    Writer->WriteValue(TEXT("overall_disposition"), Record.OverallDisposition);
    Writer->WriteValue(TEXT("objective_disposition"), Record.ObjectiveDisposition);
    Writer->WriteValue(TEXT("vehicle_disposition"), Record.VehicleDisposition);
    Writer->WriteValue(TEXT("procedure_disposition"), Record.ProcedureDisposition);
    Writer->WriteValue(TEXT("operator_disposition"), Record.OperatorDisposition);
    Writer->WriteValue(TEXT("avionics_disposition"), Record.AvionicsDisposition);
    Writer->WriteValue(TEXT("evidence_disposition"), Record.EvidenceDisposition);
    Writer->WriteValue(TEXT("viewer_semantic_json"), ExportSemanticStateJson());
    Writer->WriteObjectEnd();
    Writer->Close();
    IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
    if (PlatformFile.FileExists(*Record.SemanticPath))
    {
        FailGlobalEvidence(TEXT("guided semantic output path is not fresh"));
        return false;
    }
    const FString TemporaryPath = Record.SemanticPath + TEXT(".tmp");
    PlatformFile.DeleteFile(*TemporaryPath);
    if (!FFileHelper::SaveStringToFile(
            Output,
            *TemporaryPath,
            FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM)
        || !PlatformFile.MoveFile(*Record.SemanticPath, *TemporaryPath))
    {
        PlatformFile.DeleteFile(*TemporaryPath);
        FailGlobalEvidence(TEXT("guided semantic snapshot atomic write failed"));
        return false;
    }
    GlobalEvidenceGuidedRecords.Add(MoveTemp(Record));
    return true;
}

void UKsa64GlobalViewerSubsystem::OnGlobalEvidenceScreenshotProcessed()
{
    bGlobalEvidenceScreenshotProcessed = true;
    if (GlobalEvidenceScreenshotProcessedHandle.IsValid())
    {
        FScreenshotRequest::OnScreenshotRequestProcessed().Remove(
            GlobalEvidenceScreenshotProcessedHandle);
        GlobalEvidenceScreenshotProcessedHandle.Reset();
    }
}

bool UKsa64GlobalViewerSubsystem::ValidateGlobalEvidenceScreenshot(
    FKsa64GlobalEvidenceCapture& Capture)
{
    FImage Decoded;
    if (!FImageUtils::LoadImage(*Capture.ScreenshotPath, Decoded)
        || Decoded.SizeX <= 0
        || Decoded.SizeY <= 0
        || Decoded.NumSlices != 1)
    {
        return false;
    }
    Capture.Width = Decoded.SizeX;
    Capture.Height = Decoded.SizeY;
    Decoded.ChangeFormat(ERawImageFormat::BGRA8, EGammaSpace::sRGB);
    const TArrayView64<const FColor> Pixels = Decoded.AsBGRA8();
    if (Pixels.Num()
        != static_cast<int64>(Capture.Width) * static_cast<int64>(Capture.Height))
    {
        return false;
    }
    const int32 StepX = FMath::Max(1, Capture.Width / 64);
    const int32 StepY = FMath::Max(1, Capture.Height / 36);
    int32 MinimumLuminance = 255;
    int32 MaximumLuminance = 0;
    TSet<uint16> ColorBuckets;
    for (int32 Y = 0; Y < Capture.Height; Y += StepY)
    {
        for (int32 X = 0; X < Capture.Width; X += StepX)
        {
            const FColor Pixel = Pixels[static_cast<int64>(Y) * Capture.Width + X];
            const int32 Luminance =
                (54 * static_cast<int32>(Pixel.R)
                    + 183 * static_cast<int32>(Pixel.G)
                    + 19 * static_cast<int32>(Pixel.B))
                >> 8;
            MinimumLuminance = FMath::Min(MinimumLuminance, Luminance);
            MaximumLuminance = FMath::Max(MaximumLuminance, Luminance);
            if (Luminance > 16) ++Capture.NonDarkSamples;
            ColorBuckets.Add(static_cast<uint16>(
                (static_cast<uint16>(Pixel.R >> 5) << 6)
                | (static_cast<uint16>(Pixel.G >> 5) << 3)
                | static_cast<uint16>(Pixel.B >> 5)));
            ++Capture.SampledPixels;
        }
    }
    Capture.DistinctColorBuckets = ColorBuckets.Num();
    Capture.LuminanceRange = MaximumLuminance - MinimumLuminance;
    return Capture.Width == GlobalEvidenceWidth
        && Capture.Height == GlobalEvidenceHeight
        && Capture.SampledPixels > 0
        && Capture.DistinctColorBuckets >= GlobalEvidenceMinimumColorBuckets
        && Capture.LuminanceRange >= GlobalEvidenceMinimumLuminanceRange
        && Capture.NonDarkSamples >= FMath::Max(1, Capture.SampledPixels / 100);
}

bool UKsa64GlobalViewerSubsystem::WriteGlobalEvidenceManifest(
    bool bPassed,
    const FString& FailureReason)
{
    IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
    if (!PlatformFile.CreateDirectoryTree(*GlobalEvidenceDirectory)) return false;
    FString Output;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Output);
    Writer->WriteObjectStart();
    Writer->WriteValue(TEXT("schema"), TEXT("ksa64.phase12c.unreal-global-evidence.v1"));
    Writer->WriteValue(TEXT("pass"), bPassed);
    Writer->WriteValue(TEXT("failure_reason"), FailureReason);
    Writer->WriteValue(TEXT("source_commit"), GlobalEvidenceSourceCommit.ToLower());
    Writer->WriteValue(TEXT("scenario"), TEXT("ksa-g10r.global/nominal"));
    Writer->WriteValue(TEXT("role"), TEXT("sim-director-read-only"));
    Writer->WriteValue(TEXT("guided_scenario"), TEXT("ksa-g10r.operations/gnss-loss"));
    Writer->WriteValue(TEXT("guided_role"), TEXT("guided-operator"));
    Writer->WriteValue(TEXT("accepted_exact"), bGlobalAcceptedExact);
    Writer->WriteValue(TEXT("nominal_truth_permitted"), GlobalEvidenceCaptures.Num() == UE_ARRAY_COUNT(GlobalEvidenceMilestones));
    Writer->WriteValue(TEXT("nominal_truth_visible"), false);
    Writer->WriteValue(TEXT("guided_truth_permitted"), SemanticState.bTruthPermitted);
    Writer->WriteValue(TEXT("guided_truth_visible"), SemanticState.bTruthVisible);
    Writer->WriteValue(TEXT("nominal_terminal_release_epoch"), 22'014);
    Writer->WriteValue(TEXT("nominal_terminal_disposition"), 1);
    Writer->WriteValue(
        TEXT("guided_terminal_release_epoch"),
        SemanticState.ReleaseEpoch);
    Writer->WriteValue(
        TEXT("guided_terminal_disposition"),
        SemanticState.OverallDisposition);
    Writer->WriteObjectStart(TEXT("package"));
    Writer->WriteValue(TEXT("path"), GlobalEvidenceExecutableRelativePath.Replace(TEXT("\\"), TEXT("/")));
    Writer->WriteValue(
        TEXT("bytes"),
        static_cast<int64>(GlobalEvidenceExecutableBytes));
    Writer->WriteValue(TEXT("sha256"), GlobalEvidenceExecutableSha256.ToLower());
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("executable"));
    Writer->WriteValue(TEXT("path"), GlobalEvidenceExecutableRelativePath.Replace(TEXT("\\"), TEXT("/")));
    Writer->WriteValue(TEXT("bytes"), static_cast<int64>(GlobalEvidenceExecutableBytes));
    Writer->WriteValue(TEXT("sha256"), GlobalEvidenceExecutableSha256.ToLower());
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("packaged_directory"));
    Writer->WriteValue(TEXT("measurement"), TEXT("immutable packaged application payload excluding Saved"));
    Writer->WriteValue(TEXT("bytes"), static_cast<int64>(GlobalEvidencePackagedDirectoryBytes));
    Writer->WriteValue(TEXT("file_count"), GlobalEvidencePackagedDirectoryFiles);
    Writer->WriteValue(TEXT("tree_sha256"), GlobalEvidencePackagedDirectoryTreeSha256.ToLower());
    Writer->WriteValue(TEXT("inventory_file"), GlobalEvidencePackagedDirectoryInventoryFile);
    Writer->WriteValue(TEXT("inventory_sha256"), GlobalEvidencePackagedDirectoryInventorySha256.ToLower());
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("package_binding"));
    Writer->WriteValue(TEXT("package_audit_sha256"), GlobalEvidencePackageAuditSha256.ToLower());
    Writer->WriteValue(TEXT("binding_method"), TEXT("source-qualified-launcher-verification"));
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("frozen_reference"));
    Writer->WriteValue(TEXT("releases"), 22'015);
    Writer->WriteValue(TEXT("elapsed_seconds"), TEXT("687.9375"));
    Writer->WriteValue(TEXT("ktt10_sha256"), TEXT("a50b4b32b1c0feb44a54fc9041c40833717b9032ce127af67a9d34c3488e824a"));
    Writer->WriteValue(TEXT("kph10_sha256"), TEXT("cd664e8b72eff7aff1e3c4a5b7fb6859bb9d5178d3b6b6d4c2c06f2c61ed9cf2"));
    Writer->WriteValue(TEXT("ksr10_sha256"), TEXT("9e8691933789ce6d870d561218d6888f65acb04ef24e02796be33a704c8678aa"));
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("renderer"));
    Writer->WriteValue(TEXT("rhi_name"), GlobalEvidenceRhiName);
    Writer->WriteValue(
        TEXT("d3d12"),
        GlobalEvidenceRhiName.Contains(TEXT("D3D12"), ESearchCase::IgnoreCase));
    Writer->WriteValue(TEXT("width"), GlobalEvidenceWidth);
    Writer->WriteValue(TEXT("height"), GlobalEvidenceHeight);
    Writer->WriteValue(TEXT("fixed_timestep"), FApp::UseFixedTimeStep());
    Writer->WriteValue(TEXT("fixed_delta_seconds"), TEXT("0.016666666666666667"));
    Writer->WriteValue(TEXT("refresh_hz"), 60);
    Writer->WriteValue(
        TEXT("frames_per_second"),
        GlobalEvidenceActualRenderFramesPerSecond);
    Writer->WriteValue(TEXT("packaged_runtime"), !GIsEditor);
    Writer->WriteValue(TEXT("editor_required"), false);
    Writer->WriteValue(TEXT("mcp_required"), false);
    Writer->WriteValue(TEXT("python_required"), false);
    Writer->WriteObjectEnd();
    Writer->WriteArrayStart(TEXT("captures"));
    for (const FKsa64GlobalEvidenceCapture& Capture : GlobalEvidenceCaptures)
    {
        Writer->WriteObjectStart();
        Writer->WriteValue(TEXT("label"), Capture.Label);
        Writer->WriteValue(TEXT("release_epoch"), Capture.ReleaseEpoch);
        Writer->WriteValue(TEXT("frame_identity"), Capture.FrameIdentity);
        Writer->WriteValue(TEXT("segment_identity"), Capture.SegmentIdentity);
        Writer->WriteValue(TEXT("source_mask"), Capture.SourceMask);
        Writer->WriteValue(TEXT("transition_markers"), Capture.TransitionMarkers);
        Writer->WriteValue(TEXT("planned_path_points"), Capture.PlannedPathPoints);
        Writer->WriteValue(TEXT("onboard_path_points"), Capture.OnboardPathPoints);
        Writer->WriteValue(TEXT("observed_path_points"), Capture.ObservedPathPoints);
        Writer->WriteValue(TEXT("semantic_file"), FPaths::GetCleanFilename(Capture.SemanticPath));
        Writer->WriteValue(TEXT("screenshot_file"), FPaths::GetCleanFilename(Capture.ScreenshotPath));
        Writer->WriteValue(TEXT("width"), Capture.Width);
        Writer->WriteValue(TEXT("height"), Capture.Height);
        Writer->WriteValue(TEXT("sampled_pixels"), Capture.SampledPixels);
        Writer->WriteValue(TEXT("distinct_color_buckets"), Capture.DistinctColorBuckets);
        Writer->WriteValue(TEXT("luminance_range"), Capture.LuminanceRange);
        Writer->WriteValue(TEXT("non_dark_samples"), Capture.NonDarkSamples);
        Writer->WriteObjectEnd();
    }
    Writer->WriteArrayEnd();
    Writer->WriteArrayStart(TEXT("operational_milestones"));
    for (const FKsa64GlobalGuidedEvidenceRecord& Record : GlobalEvidenceGuidedRecords)
    {
        Writer->WriteObjectStart();
        Writer->WriteValue(TEXT("kind"), Record.Label);
        Writer->WriteValue(TEXT("label"), Record.Label);
        Writer->WriteValue(TEXT("release_epoch"), Record.ReleaseEpoch);
        Writer->WriteValue(TEXT("selected_release_epoch"), Record.ReleaseEpoch);
        Writer->WriteValue(TEXT("frame_identity"), Record.FrameIdentity);
        Writer->WriteValue(TEXT("segment_identity"), Record.SegmentIdentity);
        Writer->WriteValue(TEXT("source_mask"), Record.SourceMask);
        Writer->WriteValue(TEXT("truth_permitted"), Record.bTruthPermitted);
        Writer->WriteValue(TEXT("truth_visible"), Record.bTruthVisible);
        Writer->WriteValue(TEXT("gnss_state"), Record.GnssState);
        Writer->WriteValue(TEXT("gnss_reacquired"), false);
        Writer->WriteValue(TEXT("action_receipt_sequence"), FString::Printf(TEXT("%llu"), static_cast<unsigned long long>(Record.ActionReceiptSequence)));
        Writer->WriteValue(TEXT("action_receipt_state"), Record.ActionReceiptState);
        Writer->WriteValue(TEXT("action_receipt_accepted"), Record.ActionReceiptAccepted);
        Writer->WriteValue(TEXT("action_proposal_identity"), Record.ActionProposalIdentity);
        Writer->WriteValue(TEXT("overall_disposition"), Record.OverallDisposition);
        Writer->WriteValue(TEXT("objective_disposition"), Record.ObjectiveDisposition);
        Writer->WriteValue(TEXT("vehicle_disposition"), Record.VehicleDisposition);
        Writer->WriteValue(TEXT("procedure_disposition"), Record.ProcedureDisposition);
        Writer->WriteValue(TEXT("operator_disposition"), Record.OperatorDisposition);
        Writer->WriteValue(TEXT("avionics_disposition"), Record.AvionicsDisposition);
        Writer->WriteValue(TEXT("evidence_disposition"), Record.EvidenceDisposition);
        Writer->WriteValue(TEXT("semantic_file"), FPaths::GetCleanFilename(Record.SemanticPath));
        Writer->WriteObjectEnd();
    }
    Writer->WriteArrayEnd();
    if (const UKsa64LiveMissionSubsystem* Operations = GetOperations())
    {
        const FKsa64OperationsViewModel& View = Operations->GetViewModel();
        Writer->WriteObjectStart(TEXT("guided_completed_evidence"));
        Writer->WriteValue(TEXT("actions"), GlobalEvidenceAcceptedActions);
        Writer->WriteValue(TEXT("bytes"), FString::Printf(TEXT("%llu"), static_cast<unsigned long long>(View.EvidenceLength)));
        Writer->WriteValue(TEXT("sha256"), View.EvidenceSha256);
        Writer->WriteValue(TEXT("observation_complete"), View.bObservationComplete);
        Writer->WriteValue(TEXT("gnss_reacquired"), false);
        Writer->WriteObjectEnd();
    }
    Writer->WriteObjectStart(TEXT("renderer_origin"));
    Writer->WriteValue(TEXT("change_count"), GlobalEvidenceOriginChanges);
    Writer->WriteValue(TEXT("continuity_checks"), GlobalEvidenceOriginContinuityChecks);
    Writer->WriteValue(TEXT("semantic_unchanged"), bGlobalEvidenceOriginSemanticUnchanged);
    Writer->WriteValue(TEXT("rendered_sample_count"), GlobalEvidenceOriginRenderedSamples);
    Writer->WriteValue(TEXT("max_reconstructed_delta_cm"), GlobalEvidenceOriginMaximumDeltaCm);
    Writer->WriteValue(TEXT("rendered_continuity"), bGlobalEvidenceOriginRenderedContinuity);
    Writer->WriteValue(TEXT("semantic_continuity"), bGlobalEvidenceOriginContinuityValid);
    Writer->WriteObjectEnd();
    const bool bPerformancePassed =
        GlobalEvidenceOriginChanges > 0
        && GlobalEvidenceOriginContinuityChecks == 1
        && bGlobalEvidenceOriginContinuityValid
        && GlobalEvidenceServiceNanoseconds.Num() == GlobalEvidenceMeasuredFrameCount
        && GlobalEvidenceMeasuredFrames == GlobalEvidenceMeasuredFrameCount
        && GlobalEvidencePerformanceEndRelease
            == GlobalEvidencePerformanceStartRelease
                + GlobalEvidenceExpectedReleaseDelta
        && GlobalEvidenceP99Nanoseconds >= 0
        && GlobalEvidenceP99Nanoseconds < GlobalEvidenceP99LimitNanoseconds
        && GlobalEvidenceMaximumNanoseconds >= 0
        && GlobalEvidenceMaximumNanoseconds < GlobalEvidenceMaximumLimitNanoseconds
        && GlobalEvidenceActualRenderFramesPerSecond >= 60.0;
    Writer->WriteObjectStart(TEXT("performance"));
    Writer->WriteValue(
        TEXT("scope"),
        TEXT("GlobalDisplayV1 poll+decode+semantic+origin+procedural-scene"));
    Writer->WriteValue(TEXT("cadence"), TEXT("simulated-fixed-step"));
    Writer->WriteValue(TEXT("warmup_frames"), GlobalEvidenceWarmupFrameCount);
    Writer->WriteValue(TEXT("measured_frames"), GlobalEvidenceMeasuredFrames);
    Writer->WriteValue(TEXT("start_release"), GlobalEvidencePerformanceStartRelease);
    Writer->WriteValue(TEXT("end_release"), GlobalEvidencePerformanceEndRelease);
    Writer->WriteValue(
        TEXT("release_delta"),
        GlobalEvidencePerformanceEndRelease - GlobalEvidencePerformanceStartRelease);
    Writer->WriteValue(TEXT("expected_release_delta"), GlobalEvidenceExpectedReleaseDelta);
    Writer->WriteValue(TEXT("percentile_method"), TEXT("nearest-rank"));
    Writer->WriteValue(TEXT("p99_ns"), GlobalEvidenceP99Nanoseconds);
    Writer->WriteValue(TEXT("max_ns"), GlobalEvidenceMaximumNanoseconds);
    Writer->WriteValue(TEXT("p99_limit_ns_exclusive"), GlobalEvidenceP99LimitNanoseconds);
    Writer->WriteValue(TEXT("max_limit_ns_exclusive"), GlobalEvidenceMaximumLimitNanoseconds);
    Writer->WriteValue(
        TEXT("wall_seconds"),
        GlobalEvidenceMeasurementEndedSeconds - GlobalEvidenceMeasurementStartedSeconds);
    Writer->WriteValue(
        TEXT("actual_render_frames_per_second"),
        GlobalEvidenceActualRenderFramesPerSecond);
    Writer->WriteValue(TEXT("pass"), bPerformancePassed);
    Writer->WriteObjectEnd();
    Writer->WriteObjectEnd();
    Writer->Close();

    const FString TemporaryPath = GlobalEvidenceManifestPath + TEXT(".tmp");
    PlatformFile.DeleteFile(*TemporaryPath);
    if (!FFileHelper::SaveStringToFile(
            Output,
            *TemporaryPath,
            FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM)
        || !PlatformFile.MoveFile(*GlobalEvidenceManifestPath, *TemporaryPath))
    {
        PlatformFile.DeleteFile(*TemporaryPath);
        return false;
    }
    return true;
}

void UKsa64GlobalViewerSubsystem::FailGlobalEvidence(const FString& Reason)
{
    if (bGlobalEvidenceFailed || bGlobalEvidenceExitRequested) return;
    bGlobalEvidenceFailed = true;
    GlobalEvidenceFailureReason = Reason;
    UE_LOG(
        LogKsa64GlobalViewer,
        Error,
        TEXT("KSA64_PHASE12C_GLOBAL_EVIDENCE_FAIL_PENDING: %s"),
        *Reason);
    if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
    {
        if (Operations->GetViewModel().bSessionOpen) Operations->RequestShutdown();
    }
    ExitGlobalEvidenceFailure();
}

void UKsa64GlobalViewerSubsystem::ExitGlobalEvidenceFailure()
{
    if (bGlobalEvidenceExitRequested) return;
    WriteGlobalEvidenceManifest(false, GlobalEvidenceFailureReason);
    bGlobalEvidenceExitRequested = true;
    UE_LOG(
        LogKsa64GlobalViewer,
        Error,
        TEXT("KSA64_PHASE12C_GLOBAL_EVIDENCE_FAIL: %s"),
        *GlobalEvidenceFailureReason);
    FPlatformMisc::RequestExitWithStatus(
        false,
        1,
        TEXT("Phase12C global-viewer evidence failure"));
}

void UKsa64GlobalViewerSubsystem::TickGlobalEvidence(float DeltaSeconds)
{
    if (bGlobalEvidenceExitRequested) return;
    if (bGlobalEvidenceFailed)
    {
        ExitGlobalEvidenceFailure();
        return;
    }
    if (FPlatformTime::Seconds() - GlobalEvidenceStartedSeconds > 180.0
        && !bGlobalEvidenceSlowWarningEmitted)
    {
        bGlobalEvidenceSlowWarningEmitted = true;
        UE_LOG(
            LogKsa64GlobalViewer,
            Warning,
            TEXT("KSA64_PHASE12C_GLOBAL_EVIDENCE_SLOW: still progressing; duration alone will not terminate the run"));
    }
    if (!bGlobalEvidencePrepared)
    {
        if (!FApp::CanEverRender()
            || GDynamicRHI == nullptr
            || GEngine == nullptr
            || GEngine->GameViewport == nullptr
            || GEngine->GameViewport->Viewport == nullptr
            || !SemanticState.bSceneReady)
        {
            ++GlobalEvidenceReadyWaitFrames;
            if (GlobalEvidenceReadyWaitFrames >= GlobalEvidenceReadyFrameLimit)
            {
                FailGlobalEvidence(TEXT("D3D12 viewport and procedural scene did not become ready in the bounded startup window"));
            }
            return;
        }
        GlobalEvidenceRhiName = GDynamicRHI->GetName();
        const FIntPoint ViewportSize = GEngine->GameViewport->Viewport->GetSizeXY();
        if (!GlobalEvidenceRhiName.Contains(TEXT("D3D12"), ESearchCase::IgnoreCase)
            || ViewportSize.X != GlobalEvidenceWidth
            || ViewportSize.Y != GlobalEvidenceHeight)
        {
            FailGlobalEvidence(FString::Printf(
                TEXT("global evidence requires D3D12 at 1920x1080; found %s at %dx%d"),
                *GlobalEvidenceRhiName,
                ViewportSize.X,
                ViewportSize.Y));
            return;
        }
        constexpr double ExpectedDeltaSeconds = 1.0 / 60.0;
        if (!FApp::UseFixedTimeStep()
            || !FMath::IsNearlyEqual(
                FApp::GetFixedDeltaTime(),
                ExpectedDeltaSeconds,
                1.0e-9)
            || !FMath::IsNearlyEqual(
                static_cast<double>(DeltaSeconds),
                ExpectedDeltaSeconds,
                1.0e-6))
        {
            FailGlobalEvidence(FString::Printf(
                TEXT("global evidence requires a verified fixed 60 Hz step: enabled=%u fixed=%.12f tick=%.12f"),
                FApp::UseFixedTimeStep() ? 1u : 0u,
                FApp::GetFixedDeltaTime(),
                static_cast<double>(DeltaSeconds)));
            return;
        }
        PrepareGlobalEvidence();
        return;
    }

    UKsa64LiveMissionSubsystem* Operations = GetOperations();
    if (Operations == nullptr)
    {
        FailGlobalEvidence(TEXT("operations subsystem disappeared"));
        return;
    }
    const FKsa64OperationsViewModel& View = Operations->GetViewModel();
    if (View.WorkerState == 3
        || View.FinalizationState == 3
        || (View.Lifecycle == 6 && GlobalEvidencePhase != 7))
    {
        FailGlobalEvidence(TEXT("nominal replay worker entered a proven failure state"));
        return;
    }

    switch (GlobalEvidencePhase)
    {
    case 1:
        ObserveOperations(0.0f);
        if (!bGlobalDefinitionValid) return;
        if (GlobalEvidenceMilestoneIndex < UE_ARRAY_COUNT(GlobalEvidenceMilestones))
        {
            SeekReplayRelease(
                GlobalEvidenceMilestones[GlobalEvidenceMilestoneIndex].ReleaseEpoch);
            GlobalEvidencePhase = 2;
        }
        else
        {
            SeekReplayRelease(GlobalEvidencePerformanceStart);
            GlobalEvidencePhase = 4;
        }
        break;
    case 2:
    {
        ObserveOperations(0.0f);
        const FKsa64GlobalEvidenceMilestone& Milestone =
            GlobalEvidenceMilestones[GlobalEvidenceMilestoneIndex];
        if (SemanticState.ReleaseEpoch != Milestone.ReleaseEpoch) return;
        if (WriteGlobalEvidenceSemanticAndRequestScreenshot())
        {
            GlobalEvidencePhase = 3;
        }
        break;
    }
    case 3:
        if (!bGlobalEvidenceScreenshotProcessed)
        {
            ++GlobalEvidenceScreenshotWaitFrames;
            if (GlobalEvidenceScreenshotWaitFrames >= GlobalEvidenceReadyFrameLimit)
            {
                FailGlobalEvidence(TEXT("milestone screenshot was not processed in the bounded render-readiness window"));
            }
            return;
        }
        if (GlobalEvidenceCaptures.IsEmpty()
            || !ValidateGlobalEvidenceScreenshot(GlobalEvidenceCaptures.Last()))
        {
            FailGlobalEvidence(TEXT("milestone screenshot did not decode as a nonblank 1920x1080 image"));
            return;
        }
        UE_LOG(
            LogKsa64GlobalViewer,
            Display,
            TEXT("KSA64_PHASE12C_GLOBAL_CAPTURE_PASS label=%s release=%u"),
            *GlobalEvidenceCaptures.Last().Label,
            GlobalEvidenceCaptures.Last().ReleaseEpoch);
        ++GlobalEvidenceMilestoneIndex;
        GlobalEvidencePhase = 1;
        break;
    case 4:
    {
        ObserveOperations(0.0f);
        if (SemanticState.ReleaseEpoch != GlobalEvidencePerformanceStart) return;
        FString Reason;
        if (!ValidateGlobalEvidenceState(
                GlobalEvidencePerformanceStart,
                3,
                3,
                Reason))
        {
            FailGlobalEvidence(Reason);
            return;
        }
        ReplayReleaseAccumulator = 0.0;
        ReplayPace = EKsa64GlobalReplayPace::One;
        SemanticState.ReplayPace = ReplayPace;
        GlobalEvidenceWarmupFrames = 0;
        GlobalEvidencePhase = 5;
        break;
    }
    case 5:
        ObserveOperations(DeltaSeconds);
        ++GlobalEvidenceWarmupFrames;
        if (GlobalEvidenceWarmupFrames >= GlobalEvidenceWarmupFrameCount)
        {
            GlobalEvidencePerformanceStartRelease = SemanticState.ReleaseEpoch;
            GlobalEvidenceServiceNanoseconds.Reset();
            GlobalEvidenceMeasuredFrames = 0;
            GlobalEvidenceMeasurementStartedSeconds = FPlatformTime::Seconds();
            GlobalEvidencePhase = 6;
        }
        break;
    case 6:
    {
        const uint64 StartCycles = FPlatformTime::Cycles64();
        ObserveOperations(DeltaSeconds);
        const uint64 EndCycles = FPlatformTime::Cycles64();
        GlobalEvidenceServiceNanoseconds.Add(FMath::Max<int64>(
            0,
            FMath::RoundToInt64(
                static_cast<double>(EndCycles - StartCycles)
                * FPlatformTime::GetSecondsPerCycle64()
                * 1'000'000'000.0)));
        GlobalEvidenceMeasuredFrames = static_cast<uint32>(
            GlobalEvidenceServiceNanoseconds.Num());
        if (GlobalEvidenceMeasuredFrames < GlobalEvidenceMeasuredFrameCount) return;

        GlobalEvidenceMeasurementEndedSeconds = FPlatformTime::Seconds();
        const double MeasurementWallSeconds = FMath::Max(
            1.0e-9,
            GlobalEvidenceMeasurementEndedSeconds
                - GlobalEvidenceMeasurementStartedSeconds);
        GlobalEvidenceActualRenderFramesPerSecond =
            static_cast<double>(GlobalEvidenceMeasuredFrames)
                / MeasurementWallSeconds;
        GlobalEvidencePerformanceEndRelease = SemanticState.ReleaseEpoch;
        TArray<int64> Sorted = GlobalEvidenceServiceNanoseconds;
        Sorted.Sort();
        const int32 P99Index = FMath::Clamp(
            FMath::CeilToInt(0.99 * static_cast<double>(Sorted.Num())) - 1,
            0,
            Sorted.Num() - 1);
        GlobalEvidenceP99Nanoseconds = Sorted[P99Index];
        GlobalEvidenceMaximumNanoseconds = Sorted.Last();
        ReplayPace = EKsa64GlobalReplayPace::Paused;
        SemanticState.ReplayPace = ReplayPace;
        if (GlobalEvidencePerformanceEndRelease
                != GlobalEvidencePerformanceStartRelease
                    + GlobalEvidenceExpectedReleaseDelta
            || GlobalEvidenceP99Nanoseconds < 0
            || GlobalEvidenceP99Nanoseconds >= GlobalEvidenceP99LimitNanoseconds
            || GlobalEvidenceMaximumNanoseconds < 0
            || GlobalEvidenceMaximumNanoseconds >= GlobalEvidenceMaximumLimitNanoseconds
            || GlobalEvidenceActualRenderFramesPerSecond < 60.0)
        {
            FailGlobalEvidence(FString::Printf(
                TEXT("global display performance failed: releases=%u expected=%u frames=%u fps=%.3f p99_ns=%lld max_ns=%lld"),
                GlobalEvidencePerformanceEndRelease
                    - GlobalEvidencePerformanceStartRelease,
                GlobalEvidenceExpectedReleaseDelta,
                GlobalEvidenceMeasuredFrames,
                GlobalEvidenceActualRenderFramesPerSecond,
                static_cast<long long>(GlobalEvidenceP99Nanoseconds),
                static_cast<long long>(GlobalEvidenceMaximumNanoseconds)));
            return;
        }
        if (!Operations->RequestShutdown())
        {
            FailGlobalEvidence(TEXT("nominal replay shutdown did not queue"));
            return;
        }
        GlobalEvidencePhase = 7;
        break;
    }
    case 7:
        ObserveOperations(0.0f);
        if (!Operations->GetViewModel().bSessionOpen)
        {
            if (!StartGuidedOperations())
            {
                FailGlobalEvidence(TEXT("guided GNSS-loss operations evidence could not start"));
                return;
            }
            Operations->PausePresentation();
            SetLayout(EKsa64GlobalViewerLayout::HybridMissionDirector);
            ResumeAutomaticDirector();
            GlobalEvidenceGuidedIndex = 0;
            GlobalEvidenceAcceptedActions = 0;
            bGlobalEvidenceGuidedActionOutstanding = false;
            GlobalEvidencePhase = 8;
        }
        break;
    case 8:
    {
        ObserveOperations(0.0f);
        if (!bGlobalDefinitionValid) return;
        if (GlobalEvidenceGuidedIndex >= UE_ARRAY_COUNT(GlobalGuidedEvidenceMilestones))
        {
            GlobalEvidencePhase = 10;
            break;
        }
        const FKsa64GlobalGuidedEvidenceMilestone& Milestone =
            GlobalGuidedEvidenceMilestones[GlobalEvidenceGuidedIndex];
        const FKsa64OperationsViewModel& GuidedView = Operations->GetViewModel();
        if (GuidedView.ReleaseEpoch < Milestone.ReleaseEpoch)
        {
            if (!QueueGlobalEvidenceGuidedAdvance(*Operations, Milestone.ReleaseEpoch))
            {
                FailGlobalEvidence(TEXT("guided evidence advance could not queue"));
            }
            return;
        }
        if (GuidedView.ReleaseEpoch != Milestone.ReleaseEpoch)
        {
            FailGlobalEvidence(TEXT("guided evidence did not stop on its exact release"));
            return;
        }
        if (Milestone.Action == EKsa64GlobalGuidedEvidenceAction::None)
        {
            if (!WriteGlobalEvidenceGuidedRecord(
                    Milestone.Label,
                    Milestone.GnssState,
                    Milestone.ReceiptState))
            {
                return;
            }
            ++GlobalEvidenceGuidedIndex;
            break;
        }
        GlobalEvidenceReceiptSequenceBeforeAction = GuidedView.ActionReceiptSequence;
        if (Milestone.Action == EKsa64GlobalGuidedEvidenceAction::Stage)
        {
            Operations->ReviewAction();
            GlobalEvidenceExpectedProposalIdentity =
                Operations->GetViewModel().ActionProposalIdentity;
            Operations->StageAction();
        }
        else
        {
            GlobalEvidenceExpectedProposalIdentity = GuidedView.ActionProposalIdentity;
            Operations->CommitAction();
        }
        if (GlobalEvidenceExpectedProposalIdentity == 0)
        {
            FailGlobalEvidence(TEXT("guided action did not expose a proposal identity"));
            return;
        }
        bGlobalEvidenceGuidedActionOutstanding = true;
        GlobalEvidencePhase = 9;
        break;
    }
    case 9:
    {
        ObserveOperations(0.0f);
        const FKsa64GlobalGuidedEvidenceMilestone& Milestone =
            GlobalGuidedEvidenceMilestones[GlobalEvidenceGuidedIndex];
        const FKsa64OperationsViewModel& GuidedView = Operations->GetViewModel();
        if (GuidedView.ReleaseEpoch != Milestone.ReleaseEpoch)
        {
            FailGlobalEvidence(TEXT("guided action receipt changed the release epoch"));
            return;
        }
        if (GuidedView.ActionReceiptSequence
            <= GlobalEvidenceReceiptSequenceBeforeAction)
        {
            return;
        }
        if (!bGlobalEvidenceGuidedActionOutstanding
            || GuidedView.ActionProposalIdentity
                != GlobalEvidenceExpectedProposalIdentity
            || GuidedView.ActionReceiptState != Milestone.ReceiptState
            || GuidedView.ActionReceiptAccepted == 0)
        {
            FailGlobalEvidence(TEXT("guided action receipt was rejected or identity-mismatched"));
            return;
        }
        if (!WriteGlobalEvidenceGuidedRecord(
                Milestone.Label,
                Milestone.GnssState,
                Milestone.ReceiptState))
        {
            return;
        }
        ++GlobalEvidenceAcceptedActions;
        ++GlobalEvidenceGuidedIndex;
        bGlobalEvidenceGuidedActionOutstanding = false;
        GlobalEvidencePhase = 8;
        break;
    }
    case 10:
    {
        ObserveOperations(0.0f);
        constexpr uint32 GuidedTerminalRelease = 21'591;
        const FKsa64OperationsViewModel& GuidedView = Operations->GetViewModel();
        if (GuidedView.ReleaseEpoch < GuidedTerminalRelease)
        {
            if (!GuidedView.bSessionOpen
                || !QueueGlobalEvidenceGuidedAdvance(
                    *Operations,
                    GuidedTerminalRelease))
            {
                FailGlobalEvidence(TEXT("guided completion advance could not queue"));
            }
            return;
        }
        if (GuidedView.ReleaseEpoch != GuidedTerminalRelease)
        {
            FailGlobalEvidence(TEXT("guided completion crossed the accepted terminal release"));
            return;
        }
        if (GuidedView.bSessionOpen) return;
        if (!GuidedView.bObservationComplete
            || GlobalEvidenceGuidedRecords.Num()
                != UE_ARRAY_COUNT(GlobalGuidedEvidenceMilestones)
            || GlobalEvidenceAcceptedActions != 4
            || GuidedView.EvidenceLength != 2'911'464
            || !GuidedView.EvidenceSha256.Equals(
                TEXT("7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"),
                ESearchCase::IgnoreCase)
            || SemanticState.bTruthPermitted
            || SemanticState.bTruthVisible
            || SemanticState.SourceMask != 0x03u
            || SemanticState.OverallDisposition != 2
            || SemanticState.EvidenceDisposition != 1)
        {
            FailGlobalEvidence(FString::Printf(
                TEXT("guided completed evidence mismatch: records=%d actions=%u bytes=%llu sha=%s source=%08X disposition=%u/%u"),
                GlobalEvidenceGuidedRecords.Num(),
                GlobalEvidenceAcceptedActions,
                static_cast<unsigned long long>(GuidedView.EvidenceLength),
                *GuidedView.EvidenceSha256,
                SemanticState.SourceMask,
                SemanticState.OverallDisposition,
                SemanticState.EvidenceDisposition));
            return;
        }
        if (!WriteGlobalEvidenceManifest(true))
        {
            FailGlobalEvidence(TEXT("global-viewer manifest atomic write failed"));
            return;
        }
        bGlobalEvidenceExitRequested = true;
        UE_LOG(
            LogKsa64GlobalViewer,
            Display,
            TEXT("KSA64_PHASE12C_GLOBAL_EVIDENCE_PASS captures=%d guided=%d actions=%u release=%u width=1920 height=1080 frames=%u fps=%.3f p99_ns=%lld max_ns=%lld"),
            GlobalEvidenceCaptures.Num(),
            GlobalEvidenceGuidedRecords.Num(),
            GlobalEvidenceAcceptedActions,
            GuidedView.ReleaseEpoch,
            GlobalEvidenceMeasuredFrames,
            GlobalEvidenceActualRenderFramesPerSecond,
            static_cast<long long>(GlobalEvidenceP99Nanoseconds),
            static_cast<long long>(GlobalEvidenceMaximumNanoseconds));
        FPlatformMisc::RequestExitWithStatus(
            false,
            0,
            TEXT("Phase12C global-viewer evidence complete"));
        break;
    }
    default:
        break;
    }
}

#if WITH_DEV_AUTOMATION_TESTS
void UKsa64GlobalViewerSubsystem::ApplySampleForAutomation(
    const FKsa64GlobalSceneSample& Sample,
    bool bTruthPermitted)
{
    PreviousSample = CurrentSample;
    CurrentSample = Sample;
    bHasPreviousSample = true;
    SemanticState.ReleaseEpoch = Sample.ReleaseEpoch;
    SemanticState.MissionTimeQ16 = Sample.MissionTimeQ16;
    SemanticState.FrameIdentity = Sample.FrameIdentity;
    SemanticState.SegmentIdentity = Sample.SegmentIdentity;
    SemanticState.EventMask = Sample.EventMask;
    SemanticState.DiscontinuityMask = Sample.DiscontinuityMask;
    SemanticState.ContinuityIdentity = Sample.ContinuityIdentity;
    SemanticState.bExactSnap = Sample.bExactSnap;
    SemanticState.bAttitudeAvailable = Sample.bAttitudeValid;
    SemanticState.bTruthPermitted = bTruthPermitted;
    if (!bTruthPermitted)
    {
        bTruthRequested = false;
        SemanticState.bTruthVisible = false;
    }
    SemanticState.ResolvedCamera =
        SemanticState.RequestedCamera == EKsa64GlobalCameraMode::AutomaticDirector
            ? Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(
                Sample.FrameIdentity,
                Sample.SegmentIdentity,
                Sample.ReleaseEpoch)
            : SemanticState.RequestedCamera;
    UpdateDisplayOrigin(Sample);
}

bool UKsa64GlobalViewerSubsystem::OpenNominalReleaseForAutomation(
    UKsa64LiveMissionSubsystem& Operations,
    uint32 ReleaseEpoch)
{
    if (!Operations.IsGlobalReplayMode()
        && !Operations.StartNominalGlobalReplay()) return false;
    if (!bGlobalDefinitionValid)
    {
        ResetGlobalDisplayState();
        SemanticState.ExperienceMode = EKsa64GlobalExperienceMode::NominalReplay;
        SemanticState.RoleLabel = TEXT("SIM DIRECTOR · READ ONLY");
        constexpr double ReadyTimeoutSeconds = 180.0;
        const double DeadlineSeconds =
            FPlatformTime::Seconds() + ReadyTimeoutSeconds;
        for (;;)
        {
            Ksa64GlobalDisplayAvailabilityV1 Availability = {};
            const EKsa64OperationsAdapterResult AvailabilityResult =
                Operations.GetGlobalDisplayAvailability(Availability);
            if (AvailabilityResult == EKsa64OperationsAdapterResult::Ok) break;
            if (AvailabilityResult != EKsa64OperationsAdapterResult::NoData
                && AvailabilityResult != EKsa64OperationsAdapterResult::Unchanged)
            {
                return false;
            }
            if (FPlatformTime::Seconds() >= DeadlineSeconds) return false;
            FPlatformProcess::SleepNoStats(0.01f);
        }
        if (!InitializeGlobalDisplay(Operations)) return false;
    }
    SeekReplayRelease(ReleaseEpoch);
    if (!ObserveGlobalDisplay(Operations, 0.0f)) return false;
    RefreshSemanticState(CurrentSample, Operations);
    return CurrentSample.ReleaseEpoch == ReleaseEpoch
        && SemanticState.bAcceptanceEligible;
}

void UKsa64GlobalViewerSubsystem::ApplyReplayIndexForAutomation(
    const FKsa64GlobalReplayIndexProduct& Replay)
{
    GlobalReplayIndex = Replay;
    ApplyReplayDisposition();
}

void UKsa64GlobalViewerSubsystem::SetSceneReadyForAutomation(bool bReady)
{
    SemanticState.bSceneReady = bReady;
}

void UKsa64GlobalViewerSubsystem::SetGlobalAvailabilityForAutomation(
    bool bDefinitionValid,
    bool bAcceptedExact,
    uint32 SourceMask)
{
    bGlobalDefinitionValid = bDefinitionValid;
    bGlobalAcceptedExact = bAcceptedExact;
    PermittedGlobalSourceMask = SourceMask;
    SemanticState.DisplayAvailability = bDefinitionValid
        ? EKsa64GlobalDisplayAvailability::GlobalDisplayV1
        : EKsa64GlobalDisplayAvailability::ActiveFrameFallback;
    SemanticState.bAcceptanceEligible = bDefinitionValid && bAcceptedExact;
    SemanticState.SourceMask = SourceMask;
}
#endif
