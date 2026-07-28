#pragma once

#include "CoreMinimal.h"
#include "Ksa64GlobalDisplayCodec.h"
#include "Ksa64GlobalViewerTypes.h"

namespace Ksa64GlobalViewerPolicy
{
constexpr int64 Q12OneKilometre = 4'096;
constexpr double CentimetresPerKilometre = 100'000.0;
constexpr int64 LocalOriginQuantumQ12 = 100 * Q12OneKilometre;

enum class EKsa64GuidedDisplaySyncDecision : uint8
{
    Wait,
    Aligned,
    RejectAhead,
    RejectTimeout,
};

EKsa64GuidedDisplaySyncDecision ObserveGuidedDisplaySync(
    uint32 DisplayRelease,
    uint32 OperationsRelease,
    uint32 FrameLimit,
    uint32& WaitFrames);
bool RequiredGlobalPathSourcesAvailable(
    uint32 RequiredSourceMask,
    uint32 AvailableSourceMask);

double Q12Kilometres(int64 Raw);
int64 QuantizeOriginQ12(int64 Raw, int64 Quantum = LocalOriginQuantumQ12);
FQuat Ksa64BodyToFrameQuaternionToUnreal(const int32 QuaternionQ30[4]);
FVector3d Ksa64RightHandedToUnrealCentimetres(
    const int32 PositionQ12Km[3],
    const int64 OriginQ12Km[3]);
FVector3d Ksa64RightHandedToUnrealCentimetres(
    const int64 PositionQ12Km[3],
    const int64 OriginQ12Km[3]);
uint32 HashPathPoints(
    const TArray<FKsa64GlobalPathPointProduct>& Points,
    int32 StartIndex,
    int32 PointCount);
FLinearColor PathColorForFlags(const FLinearColor& Normal, uint16 Flags);
bool ShouldSnap(
    const FKsa64GlobalSceneSample& Previous,
    const FKsa64GlobalSceneSample& Current);
EKsa64GlobalCameraMode ResolveAutomaticCamera(
    uint32 FrameIdentity,
    uint32 SegmentIdentity,
    uint32 ReleaseEpoch);
FString FrameLabel(uint32 FrameIdentity);
FString CameraLabel(EKsa64GlobalCameraMode Camera);
FString LayoutLabel(EKsa64GlobalViewerLayout Layout);
}
