#pragma once

#include "CoreMinimal.h"

enum class EKsa64GlobalViewerLayout : uint8
{
    HybridMissionDirector = 1,
    EngineeringSplit = 2,
    CinematicFullscreen = 3
};

enum class EKsa64GlobalCameraMode : uint8
{
    AutomaticDirector = 1,
    LaunchLocalEnu = 2,
    VehicleChase = 3,
    EarthFixed = 4,
    EarthInertial = 5,
    RecoveryLocalEnu = 6,
    FreeOrbit = 7,
    TrueScaleInspection = 8
};

enum class EKsa64GlobalExperienceMode : uint8
{
    None = 0,
    GuidedOperations = 1,
    NominalReplay = 2
};

enum class EKsa64GlobalReplayPace : uint8
{
    Paused = 0,
    Quarter = 1,
    Half = 2,
    One = 3,
    Two = 4,
    Four = 5,
    Eight = 6,
    Sixteen = 7,
    Unpaced = 8
};

enum class EKsa64GlobalDisplaySource : uint8
{
    PlannedReference = 1,
    OnboardEstimate = 2,
    GroundEstimate = 3,
    SimTruth = 4
};

enum class EKsa64GlobalDisplayAvailability : uint8
{
    Unavailable = 0,
    ActiveFrameFallback = 1,
    GlobalDisplayV1 = 2
};

struct FKsa64GlobalSceneSample
{
    uint32 ReleaseEpoch = 0;
    uint32 MissionTimeQ16 = 0;
    uint32 FrameIdentity = 0;
    uint32 SegmentIdentity = 0;
    uint32 EventMask = 0;
    uint32 DiscontinuityMask = 0;
    uint64 ContinuityIdentity = 0;
    int32 PositionQ12Km[3] = {0, 0, 0};
    int32 GroundPositionQ12Km[3] = {0, 0, 0};
    int32 AttitudeQ30[4] = {1 << 30, 0, 0, 0};
    int32 AngularRateQ24[3] = {0, 0, 0};
    bool bPositionValid = false;
    bool bGroundPositionValid = false;
    bool bAttitudeValid = false;
    bool bExactSnap = true;
};

struct FKsa64GlobalSemanticState
{
    FString Schema = TEXT("ksa64.unreal-global-viewer-semantic.v1");
    EKsa64GlobalViewerLayout Layout = EKsa64GlobalViewerLayout::HybridMissionDirector;
    EKsa64GlobalExperienceMode ExperienceMode = EKsa64GlobalExperienceMode::None;
    EKsa64GlobalReplayPace ReplayPace = EKsa64GlobalReplayPace::Paused;
    EKsa64GlobalCameraMode RequestedCamera = EKsa64GlobalCameraMode::AutomaticDirector;
    EKsa64GlobalCameraMode ResolvedCamera = EKsa64GlobalCameraMode::LaunchLocalEnu;
    EKsa64GlobalDisplayAvailability DisplayAvailability =
        EKsa64GlobalDisplayAvailability::Unavailable;
    uint32 ReleaseEpoch = 0;
    uint32 MissionTimeQ16 = 0;
    uint32 FrameIdentity = 0;
    uint32 SegmentIdentity = 0;
    uint32 EventMask = 0;
    uint32 DiscontinuityMask = 0;
    uint64 ContinuityIdentity = 0;
    uint32 SourceMask = 0;
    uint32 ObservedPathPoints = 0;
    uint32 PlannedPathPoints = 0;
    uint32 OnboardPathPoints = 0;
    uint32 GroundPathPoints = 0;
    uint32 TransitionMarkers = 0;
    uint32 ReplayOldestRelease = 0;
    uint32 ReplayNewestRelease = 0;
    uint32 ReplaySelectedRelease = 0;
    uint32 ReplayBookmarkCount = 0;
    uint32 OverallDisposition = 0;
    uint32 ObjectiveDisposition = 0;
    uint32 VehicleDisposition = 0;
    uint32 ProcedureDisposition = 0;
    uint32 OperatorDisposition = 0;
    uint32 AvionicsDisposition = 0;
    uint32 EvidenceDisposition = 0;
    int64 DisplayOriginQ12Km[3] = {0, 0, 0};
    bool bSceneReady = false;
    bool bAcceptanceEligible = false;
    bool bSessionOpen = false;
    bool bExactSnap = true;
    bool bOperationsDeskVisible = false;
    bool bAutoDirectorSuspended = false;
    bool bTruthPermitted = false;
    bool bTruthVisible = false;
    bool bAttitudeAvailable = false;
    bool bVehicleLocatorVisible = true;
    bool bTrueScaleInsetVisible = true;
    bool bObservationComplete = true;
    FString FrameLabel = TEXT("FRAME —");
    FString RoleLabel = TEXT("GUIDED OPERATOR");
    FString StatusLabel = TEXT("WAITING FOR OPERATIONS");
    FString SourceLabel = TEXT("ONBOARD ESTIMATE");
    FString DispositionLabel = TEXT("OUTCOME PENDING");

    FString ToDeterministicJson() const;
};
