#include "Ksa64GlobalViewerPolicy.h"

namespace Ksa64GlobalViewerPolicy
{
double Q12Kilometres(int64 Raw)
{
    return static_cast<double>(Raw) / static_cast<double>(Q12OneKilometre);
}

int64 QuantizeOriginQ12(int64 Raw, int64 Quantum)
{
    if (Quantum <= 0)
    {
        return 0;
    }
    const int64 Half = Quantum / 2;
    return Raw >= 0
        ? ((Raw + Half) / Quantum) * Quantum
        : -(((-Raw + Half) / Quantum) * Quantum);
}

FVector3d Ksa64RightHandedToUnrealCentimetres(
    const int64 PositionQ12Km[3],
    const int64 OriginQ12Km[3])
{
    // KSA64 is right handed. Unreal is left handed, so reflect +Y once and
    // nowhere else. This is a presentation basis mapping, never propagation.
    return FVector3d(
        Q12Kilometres(PositionQ12Km[0] - OriginQ12Km[0]) * CentimetresPerKilometre,
        -Q12Kilometres(PositionQ12Km[1] - OriginQ12Km[1]) * CentimetresPerKilometre,
        Q12Kilometres(PositionQ12Km[2] - OriginQ12Km[2]) * CentimetresPerKilometre);
}

FVector3d Ksa64RightHandedToUnrealCentimetres(
    const int32 PositionQ12Km[3],
    const int64 OriginQ12Km[3])
{
    const int64 Wide[3] = {
        PositionQ12Km[0],
        PositionQ12Km[1],
        PositionQ12Km[2],
    };
    return Ksa64RightHandedToUnrealCentimetres(Wide, OriginQ12Km);
}

FQuat Ksa64BodyToFrameQuaternionToUnreal(const int32 QuaternionQ30[4])
{
    const double Scale = 1.0 / static_cast<double>(1ll << 30);
    return FQuat(
        -static_cast<double>(QuaternionQ30[1]) * Scale,
        static_cast<double>(QuaternionQ30[2]) * Scale,
        -static_cast<double>(QuaternionQ30[3]) * Scale,
        static_cast<double>(QuaternionQ30[0]) * Scale).GetNormalized();
}

bool ShouldSnap(
    const FKsa64GlobalSceneSample& Previous,
    const FKsa64GlobalSceneSample& Current)
{
    return !Previous.bPositionValid
        || !Current.bPositionValid
        || Current.ReleaseEpoch <= Previous.ReleaseEpoch
        || Current.ReleaseEpoch - Previous.ReleaseEpoch > 64
        || Current.FrameIdentity != Previous.FrameIdentity
        || Current.SegmentIdentity != Previous.SegmentIdentity
        || Current.ContinuityIdentity != Previous.ContinuityIdentity
        || Current.EventMask != 0
        || Current.DiscontinuityMask != 0
        || !Previous.bAttitudeValid
        || !Current.bAttitudeValid;
}

EKsa64GlobalCameraMode ResolveAutomaticCamera(
    uint32 FrameIdentity,
    uint32 SegmentIdentity,
    uint32 ReleaseEpoch)
{
    if (FrameIdentity == 3)
    {
        return EKsa64GlobalCameraMode::EarthInertial;
    }
    if (FrameIdentity == 2)
    {
        // The frame remains authoritative. Segment is only a presentation hint
        // for a closer entry view when the typed GlobalDisplayV1 stream exists.
        return SegmentIdentity == 4
            ? EKsa64GlobalCameraMode::VehicleChase
            : EKsa64GlobalCameraMode::EarthFixed;
    }
    if (FrameIdentity == 1)
    {
        return ReleaseEpoch < 64 || SegmentIdentity == 1
            ? EKsa64GlobalCameraMode::LaunchLocalEnu
            : EKsa64GlobalCameraMode::RecoveryLocalEnu;
    }
    return EKsa64GlobalCameraMode::VehicleChase;
}

FString FrameLabel(uint32 FrameIdentity)
{
    switch (FrameIdentity)
    {
    case 1: return TEXT("LOCAL ENU");
    case 2: return TEXT("EARTH FIXED / ECEF");
    case 3: return TEXT("EARTH INERTIAL / GCRF");
    default: return TEXT("FRAME UNAVAILABLE");
    }
}

FString CameraLabel(EKsa64GlobalCameraMode Camera)
{
    switch (Camera)
    {
    case EKsa64GlobalCameraMode::AutomaticDirector: return TEXT("AUTO DIRECTOR");
    case EKsa64GlobalCameraMode::LaunchLocalEnu: return TEXT("LAUNCH / LOCAL ENU");
    case EKsa64GlobalCameraMode::VehicleChase: return TEXT("VEHICLE CHASE");
    case EKsa64GlobalCameraMode::EarthFixed: return TEXT("EARTH FIXED");
    case EKsa64GlobalCameraMode::EarthInertial: return TEXT("EARTH INERTIAL");
    case EKsa64GlobalCameraMode::RecoveryLocalEnu: return TEXT("RECOVERY / LOCAL ENU");
    case EKsa64GlobalCameraMode::FreeOrbit: return TEXT("FREE ORBIT");
    case EKsa64GlobalCameraMode::TrueScaleInspection: return TEXT("TRUE-SCALE INSPECTION");
    default: return TEXT("CAMERA UNAVAILABLE");
    }
}

FString LayoutLabel(EKsa64GlobalViewerLayout Layout)
{
    switch (Layout)
    {
    case EKsa64GlobalViewerLayout::HybridMissionDirector: return TEXT("HYBRID DIRECTOR");
    case EKsa64GlobalViewerLayout::EngineeringSplit: return TEXT("ENGINEERING SPLIT");
    case EKsa64GlobalViewerLayout::CinematicFullscreen: return TEXT("CINEMATIC");
    default: return TEXT("LAYOUT UNAVAILABLE");
    }
}
}
