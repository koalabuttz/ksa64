#pragma once

#include "CoreMinimal.h"
#include "Components/PrimitiveComponent.h"
#include "Ksa64GlobalLineComponent.generated.h"

/**
 * Presentation-only preallocated line list. Points are supplied as pairs and
 * are already relative to the viewer's display origin.
 */
UCLASS(ClassGroup = (KSA64), meta = (BlueprintSpawnableComponent))
class KSA64GLOBALVIEWER_API UKsa64GlobalLineComponent final : public UPrimitiveComponent
{
    GENERATED_BODY()

public:
    void SetSegments(
        const TArray<FVector3d>& InSegmentPoints,
        const FLinearColor& InColor,
        float InThickness = 1.0f);
    void ResetSegments();
    int32 GetSegmentCount() const { return SegmentPoints.Num() / 2; }

    virtual FPrimitiveSceneProxy* CreateSceneProxy() override;
    virtual FBoxSphereBounds CalcBounds(const FTransform& LocalToWorld) const override;
    virtual bool ShouldCreatePhysicsState() const override { return false; }

private:
    TArray<FVector3f> SegmentPoints;
    FLinearColor LineColor = FLinearColor::White;
    float Thickness = 1.0f;
};
