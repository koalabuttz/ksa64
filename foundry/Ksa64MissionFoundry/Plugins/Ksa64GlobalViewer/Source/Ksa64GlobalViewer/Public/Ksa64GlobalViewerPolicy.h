#pragma once

#include "CoreMinimal.h"
#include "Ksa64GlobalViewerTypes.h"

namespace Ksa64GlobalViewerPolicy
{
constexpr int64 Q12OneKilometre = 4'096;
constexpr double CentimetresPerKilometre = 100'000.0;
constexpr int64 LocalOriginQuantumQ12 = 100 * Q12OneKilometre;

double Q12Kilometres(int64 Raw);
int64 QuantizeOriginQ12(int64 Raw, int64 Quantum = LocalOriginQuantumQ12);
FQuat Ksa64BodyToFrameQuaternionToUnreal(const int32 QuaternionQ30[4]);
FVector3d Ksa64RightHandedToUnrealCentimetres(
    const int32 PositionQ12Km[3],
    const int64 OriginQ12Km[3]);
FVector3d Ksa64RightHandedToUnrealCentimetres(
    const int64 PositionQ12Km[3],
    const int64 OriginQ12Km[3]);
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
