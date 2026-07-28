#include "Ksa64GlobalViewerSubsystem.h"

#include "Ksa64GlobalLineComponent.h"
#include "Ksa64GlobalViewerOverlay.h"
#include "Ksa64GlobalViewerPolicy.h"
#include "Ksa64LiveMissionSubsystem.h"

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
#include "Framework/Application/SlateApplication.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformTime.h"
#include "Materials/Material.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"

DEFINE_LOG_CATEGORY_STATIC(LogKsa64GlobalViewer, Log, All);

namespace
{
constexpr double Wgs84EquatorialRadiusCentimetres = 6378.137 * 100'000.0;
constexpr double Wgs84PolarScale = 6356.752314245 / 6378.137;
constexpr int32 EarthGridLatitudeSteps = 12;
constexpr int32 EarthGridLongitudeSteps = 24;
constexpr int32 EarthGridCurveSteps = 96;
constexpr int32 MaximumPathSegments = 32'768;

void AddSegment(
    TArray<FVector3d>& Points,
    const FVector3d& Start,
    const FVector3d& End)
{
    Points.Add(Start);
    Points.Add(End);
}

FVector3d EarthPoint(double LatitudeRadians, double LongitudeRadians)
{
    const double CosLatitude = FMath::Cos(LatitudeRadians);
    const double X = Wgs84EquatorialRadiusCentimetres
        * CosLatitude
        * FMath::Cos(LongitudeRadians);
    const double Y = -Wgs84EquatorialRadiusCentimetres
        * CosLatitude
        * FMath::Sin(LongitudeRadians);
    const double Z = Wgs84EquatorialRadiusCentimetres
        * Wgs84PolarScale
        * FMath::Sin(LatitudeRadians);
    return FVector3d(X, Y, Z);
}
}

void UKsa64GlobalViewerSubsystem::Initialize(FSubsystemCollectionBase& Collection)
{
    Collection.InitializeDependency<UKsa64LiveMissionSubsystem>();
    Super::Initialize(Collection);
    SemanticState.Layout = EKsa64GlobalViewerLayout::HybridMissionDirector;
    SemanticState.RequestedCamera = EKsa64GlobalCameraMode::AutomaticDirector;
    SemanticState.ResolvedCamera = EKsa64GlobalCameraMode::LaunchLocalEnu;
    LastSceneSampleWallSeconds = FPlatformTime::Seconds();

    if (UKsa64LiveMissionSubsystem* Operations = GetOperations())
    {
        Operations->SetDashboardVisible(false);
    }
    TickerHandle = FTSTicker::GetCoreTicker().AddTicker(
        FTickerDelegate::CreateUObject(this, &UKsa64GlobalViewerSubsystem::Tick));
}

void UKsa64GlobalViewerSubsystem::Deinitialize()
{
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

bool UKsa64GlobalViewerSubsystem::Tick(float DeltaSeconds)
{
    InstallOverlayIfPossible();
    EnsureScene();
    ObserveOperations(DeltaSeconds);
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
    if (World == nullptr || !World->IsGameWorld())
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

    AActor* VehicleActor = World->SpawnActor<AActor>(
        AActor::StaticClass(),
        FTransform::Identity);
    if (VehicleActor != nullptr)
    {
        VehicleActor->SetActorLabel(TEXT("KSA-G10R Schematic Vehicle"));
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
        Camera->SetActorLabel(TEXT("KSA64 Global Viewer Camera"));
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
            Wgs84EquatorialRadiusCentimetres / 50.0,
            Wgs84EquatorialRadiusCentimetres / 50.0,
            Wgs84EquatorialRadiusCentimetres * Wgs84PolarScale / 50.0));
    }
    if (AtmosphereMesh.IsValid())
    {
        AtmosphereMesh->SetRelativeScale3D(FVector(
            Wgs84EquatorialRadiusCentimetres * 1.012 / 50.0,
            Wgs84EquatorialRadiusCentimetres * 1.012 / 50.0,
            Wgs84EquatorialRadiusCentimetres * Wgs84PolarScale * 1.012 / 50.0));
    }
    if (LocatorMesh.IsValid())
    {
        LocatorMesh->SetRelativeScale3D(FVector(0.08));
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
        FVector3d Previous = EarthPoint(LatRadians, -UE_PI);
        for (int32 Step = 1; Step <= EarthGridCurveSteps; ++Step)
        {
            const double Longitude =
                -UE_PI + 2.0 * UE_PI * Step / EarthGridCurveSteps;
            const FVector3d Current = EarthPoint(LatRadians, Longitude);
            AddSegment(Segments, Previous, Current);
            Previous = Current;
        }
    }
    for (int32 Longitude = 0; Longitude < EarthGridLongitudeSteps; ++Longitude)
    {
        const double LonRadians =
            2.0 * UE_PI * Longitude / EarthGridLongitudeSteps;
        FVector3d Previous = EarthPoint(-UE_PI * 0.5, LonRadians);
        for (int32 Step = 1; Step <= EarthGridCurveSteps / 2; ++Step)
        {
            const double Latitude =
                -UE_PI * 0.5 + UE_PI * Step / (EarthGridCurveSteps / 2);
            const FVector3d Current = EarthPoint(Latitude, LonRadians);
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

void UKsa64GlobalViewerSubsystem::RefreshSemanticState(
    const FKsa64GlobalSceneSample& Sample,
    const UKsa64LiveMissionSubsystem& Operations)
{
    const FKsa64OperationsViewModel& View = Operations.GetViewModel();
    SemanticState.DisplayAvailability =
        EKsa64GlobalDisplayAvailability::ActiveFrameFallback;
    SemanticState.ReleaseEpoch = Sample.ReleaseEpoch;
    SemanticState.MissionTimeQ16 = Sample.MissionTimeQ16;
    SemanticState.FrameIdentity = Sample.FrameIdentity;
    SemanticState.SegmentIdentity = Sample.SegmentIdentity;
    SemanticState.EventMask = Sample.EventMask;
    SemanticState.DiscontinuityMask = Sample.DiscontinuityMask;
    SemanticState.ContinuityIdentity = Sample.ContinuityIdentity;
    SemanticState.bExactSnap = Sample.bExactSnap;
    SemanticState.bAttitudeAvailable = Sample.bAttitudeValid;
    SemanticState.FrameLabel =
        Ksa64GlobalViewerPolicy::FrameLabel(Sample.FrameIdentity);
    SemanticState.SourceMask = (1u << 0) | (1u << 1);
    if (Sample.bGroundPositionValid)
    {
        SemanticState.SourceMask |= 1u << 2;
    }
    if (SemanticState.bTruthPermitted)
    {
        SemanticState.SourceMask |= 1u << 3;
    }
    SemanticState.ObservedPathPoints = Operations.GetReleaseHistory().Num();
    SemanticState.PlannedPathPoints = Operations.GetPlannedReferencePath().Num();
    SemanticState.OnboardPathPoints = Operations.GetOnboardPredictionPath().Num();
    SemanticState.GroundPathPoints = Operations.GetGroundPredictionPath().Num();
    uint32 Transitions = 0;
    uint32 PreviousFrame = 0;
    for (const FKsa64OperationsReleasePoint& Point : Operations.GetReleaseHistory())
    {
        if (PreviousFrame != 0 && Point.FrameIdentity != PreviousFrame)
        {
            ++Transitions;
        }
        if (Point.FrameIdentity != 0)
        {
            PreviousFrame = Point.FrameIdentity;
        }
    }
    SemanticState.TransitionMarkers = Transitions;
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
    const bool bEarthCentred =
        SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::EarthFixed
        || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::EarthInertial
        || SemanticState.ResolvedCamera == EKsa64GlobalCameraMode::FreeOrbit;
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        SemanticState.DisplayOriginQ12Km[Axis] =
            bEarthCentred || !Sample.bPositionValid
                ? 0
                : Ksa64GlobalViewerPolicy::QuantizeOriginQ12(
                    Sample.PositionQ12Km[Axis]);
    }
    ApplyOriginToStaticDomain();
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
    const bool bLocal = FrameIdentity == 1;
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
    if (!Sample.bPositionValid || !VehicleBodyMesh.IsValid())
    {
        if (VehicleBodyMesh.IsValid()) VehicleBodyMesh->GetOwner()->SetActorHiddenInGame(true);
        if (LocatorMesh.IsValid()) LocatorMesh->SetVisibility(false);
        return;
    }
    const FVector3d Relative =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            Sample.PositionQ12Km,
            SemanticState.DisplayOriginQ12Km);
    AActor* Vehicle = VehicleBodyMesh->GetOwner();
    Vehicle->SetActorHiddenInGame(false);
    Vehicle->SetActorLocation(Relative);
    // No attitude is invented when the role-filtered product does not provide
    // it. The schematic remains identity-oriented and is explicitly labelled.
    if (Sample.bAttitudeValid)
    {
        const FQuat Quaternion(
            static_cast<double>(Sample.AttitudeQ30[1]) / (1ll << 30),
            -static_cast<double>(Sample.AttitudeQ30[2]) / (1ll << 30),
            static_cast<double>(Sample.AttitudeQ30[3]) / (1ll << 30),
            static_cast<double>(Sample.AttitudeQ30[0]) / (1ll << 30));
        Vehicle->SetActorRotation(Quaternion.GetNormalized());
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
}

void UKsa64GlobalViewerSubsystem::UpdatePaths(uint32 ActiveFrame)
{
    const UKsa64LiveMissionSubsystem* Operations = GetOperations();
    if (Operations == nullptr)
    {
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
    if (!ViewerCamera.IsValid() || !Sample.bPositionValid)
    {
        return;
    }
    const FVector3d Target =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            Sample.PositionQ12Km,
            SemanticState.DisplayOriginQ12Km);
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
            + Direction * (Wgs84EquatorialRadiusCentimetres * 1.85)
            + FVector3d(0.0, 0.0, Wgs84EquatorialRadiusCentimetres * 0.18);
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
            ? TEXT("GLOBAL DISPLAY V1")
            : TEXT("ACTIVE-FRAME OPERATIONAL FALLBACK")));
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

void UKsa64GlobalViewerSubsystem::SetSceneReadyForAutomation(bool bReady)
{
    SemanticState.bSceneReady = bReady;
}
#endif
