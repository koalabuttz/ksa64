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

uint32 HashPathPoints(
    const TArray<FKsa64GlobalPathPointProduct>& Points,
    int32 StartIndex,
    int32 PointCount)
{
    uint32 Hash = 0x811c9dc5u;
    const auto Mix = [&Hash](uint32 Value)
    {
        for (uint32 Shift = 0; Shift < 32; Shift += 8)
        {
            Hash ^= (Value >> Shift) & 0xffu;
            Hash *= 0x01000193u;
        }
    };
    const int32 SafeStart = FMath::Clamp(StartIndex, 0, Points.Num());
    const int32 SafeCount = FMath::Clamp(PointCount, 0, Points.Num() - SafeStart);
    const int32 EndIndex = SafeStart + SafeCount;
    for (int32 Index = SafeStart; Index < EndIndex; ++Index)
    {
        const FKsa64GlobalPathPointProduct& Point = Points[Index];
        Mix(Point.ReleaseEpoch);
        Mix(Point.MissionTimeQ16);
        Mix(Point.Segment);
        Mix(Point.EventMask);
        Mix(Point.AnchorIdentity);
        for (const int32 Axis : Point.PositionQ12Km)
        {
            Mix(static_cast<uint32>(Axis));
        }
    }
    return Hash;
}

bool TryExpectedLatestGuidedDisplayRelease(
    uint32 OperationsRelease,
    uint32& OutDisplayRelease)
{
    if (OperationsRelease == 0)
    {
        return false;
    }
    // FullMissionSession::release_epoch is the next release boundary. The
    // display product at that boundary is the interval that just completed.
    OutDisplayRelease = OperationsRelease - 1;
    return true;
}

EKsa64GuidedDisplaySyncDecision ObserveGuidedDisplaySync(
    uint32 DisplayRelease,
    uint32 ExpectedDisplayRelease,
    uint32 FrameLimit,
    uint32& WaitFrames)
{
    if (DisplayRelease < ExpectedDisplayRelease)
    {
        ++WaitFrames;
        return WaitFrames >= FrameLimit
            ? EKsa64GuidedDisplaySyncDecision::RejectTimeout
            : EKsa64GuidedDisplaySyncDecision::Wait;
    }
    if (DisplayRelease > ExpectedDisplayRelease)
    {
        return EKsa64GuidedDisplaySyncDecision::RejectAhead;
    }
    WaitFrames = 0;
    return EKsa64GuidedDisplaySyncDecision::Aligned;
}

bool RequiredGlobalPathSourcesAvailable(
    uint32 RequiredSourceMask,
    uint32 AvailableSourceMask)
{
    return (AvailableSourceMask & RequiredSourceMask) == RequiredSourceMask;
}

FLinearColor PathColorForFlags(const FLinearColor& Normal, uint16 Flags)
{
    FLinearColor Result = Normal;
    // Terminal is a completion property, not a degradation. Resync, stale,
    // and incomplete states use Babylon's deterministic severity precedence.
    if ((Flags & Ksa64GlobalPathFlags::ResyncRequired) != 0)
    {
        Result.A = FMath::Min(Result.A, 0.18f);
    }
    else if ((Flags & Ksa64GlobalPathFlags::Stale) != 0)
    {
        Result.A = FMath::Min(Result.A, 0.28f);
    }
    else if ((Flags & Ksa64GlobalPathFlags::Incomplete) != 0)
    {
        Result.A = FMath::Min(Result.A, 0.48f);
    }
    return Result;
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
        // Match the browser director: follow powered ECEF ascent closely,
        // then open to an Earth-fixed engineering view at burnout. Entry
        // remains Earth-fixed so neither renderer invents a source change.
        return SegmentIdentity == 2 && ReleaseEpoch < 1'920
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
