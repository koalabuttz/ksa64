#include "Ksa64GlobalLineComponent.h"

#include "PrimitiveSceneProxy.h"
#include "SceneManagement.h"

#if PLATFORM_WINDOWS && !RHI_RAYTRACING
#error "Ksa64GlobalViewer must match the UE 5.8 Launcher renderer ABI; keep RayTracingMode=Inline and runtime r.RayTracing=False"
#endif

namespace
{
class FKsa64GlobalLineSceneProxy final : public FPrimitiveSceneProxy
{
public:
    FKsa64GlobalLineSceneProxy(
        const UKsa64GlobalLineComponent* Component,
        const TArray<FVector3f>& InSegmentPoints,
        const FLinearColor& InColor,
        float InThickness)
        : FPrimitiveSceneProxy(Component)
        , SegmentPoints(InSegmentPoints)
        , Color(InColor)
        , Thickness(InThickness)
    {
    }

    virtual void GetDynamicMeshElements(
        const TArray<const FSceneView*>& Views,
        const FSceneViewFamily& ViewFamily,
        uint32 VisibilityMap,
        FMeshElementCollector& Collector) const override
    {
        QUICK_SCOPE_CYCLE_COUNTER(STAT_Ksa64GlobalLineSceneProxy_GetDynamicMeshElements);
        for (int32 ViewIndex = 0; ViewIndex < Views.Num(); ++ViewIndex)
        {
            if ((VisibilityMap & (1u << ViewIndex)) == 0)
            {
                continue;
            }
            FPrimitiveDrawInterface* Pdi = Collector.GetPDI(ViewIndex);
            const FMatrix ComponentToWorld = GetLocalToWorld();
            for (int32 Point = 0; Point + 1 < SegmentPoints.Num(); Point += 2)
            {
                Pdi->DrawLine(
                    ComponentToWorld.TransformPosition(FVector(SegmentPoints[Point])),
                    ComponentToWorld.TransformPosition(FVector(SegmentPoints[Point + 1])),
                    Color,
                    SDPG_World,
                    Thickness,
                    0.0f,
                    true);
            }
        }
    }

    virtual FPrimitiveViewRelevance GetViewRelevance(const FSceneView* View) const override
    {
        FPrimitiveViewRelevance Result;
        Result.bDrawRelevance = IsShown(View);
        Result.bDynamicRelevance = true;
        Result.bRenderInMainPass = ShouldRenderInMainPass();
        Result.bUsesLightingChannels = GetLightingChannelMask() != GetDefaultLightingChannelMask();
        Result.bShadowRelevance = IsShadowCast(View);
        return Result;
    }

    virtual SIZE_T GetTypeHash() const override
    {
        static size_t UniquePointer;
        return reinterpret_cast<SIZE_T>(&UniquePointer);
    }

    virtual uint32 GetMemoryFootprint() const override
    {
        return sizeof(*this) + GetAllocatedSize();
    }

    uint32 GetAllocatedSize() const
    {
        return SegmentPoints.GetAllocatedSize();
    }

private:
    TArray<FVector3f> SegmentPoints;
    FLinearColor Color;
    float Thickness = 1.0f;
};
}

void UKsa64GlobalLineComponent::SetSegments(
    const TArray<FVector3d>& InSegmentPoints,
    const FLinearColor& InColor,
    float InThickness)
{
    SegmentPoints.Reset(InSegmentPoints.Num());
    for (const FVector3d& Point : InSegmentPoints)
    {
        SegmentPoints.Add(FVector3f(Point));
    }
    LineColor = InColor;
    Thickness = FMath::Max(0.25f, InThickness);
    UpdateBounds();
    MarkRenderStateDirty();
}

void UKsa64GlobalLineComponent::ResetSegments()
{
    SegmentPoints.Reset();
    UpdateBounds();
    MarkRenderStateDirty();
}

void UKsa64GlobalLineComponent::AppendWorldSamplePoints(
    TArray<FVector3d>& OutPoints,
    int32 MaximumSamples) const
{
    if (SegmentPoints.IsEmpty() || MaximumSamples <= 0)
    {
        return;
    }
    const int32 Samples = FMath::Min(MaximumSamples, SegmentPoints.Num());
    const FTransform Transform = GetComponentTransform();
    for (int32 Sample = 0; Sample < Samples; ++Sample)
    {
        const int32 Index = Samples == 1
            ? 0
            : static_cast<int32>(
                static_cast<int64>(Sample) * (SegmentPoints.Num() - 1)
                / (Samples - 1));
        OutPoints.Add(FVector3d(Transform.TransformPosition(FVector(SegmentPoints[Index]))));
    }
}

FPrimitiveSceneProxy* UKsa64GlobalLineComponent::CreateSceneProxy()
{
    return SegmentPoints.Num() >= 2
        ? new FKsa64GlobalLineSceneProxy(this, SegmentPoints, LineColor, Thickness)
        : nullptr;
}

FBoxSphereBounds UKsa64GlobalLineComponent::CalcBounds(const FTransform& LocalToWorld) const
{
    if (SegmentPoints.IsEmpty())
    {
        return FBoxSphereBounds(FSphere(FVector::ZeroVector, 1.0)).TransformBy(LocalToWorld);
    }
    FBox LocalBounds(EForceInit::ForceInit);
    for (const FVector3f& Point : SegmentPoints)
    {
        LocalBounds += FVector(Point);
    }
    return FBoxSphereBounds(LocalBounds.ExpandBy(FMath::Max(1.0f, Thickness))).TransformBy(LocalToWorld);
}
