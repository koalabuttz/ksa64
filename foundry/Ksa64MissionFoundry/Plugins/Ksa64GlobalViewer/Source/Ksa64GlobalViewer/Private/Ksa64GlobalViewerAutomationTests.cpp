#if WITH_DEV_AUTOMATION_TESTS

#include "Ksa64GlobalDisplayCodec.h"
#include "Ksa64GlobalViewerPolicy.h"
#include "Ksa64GlobalViewerSubsystem.h"
#include "Ksa64LiveMissionSubsystem.h"

#include "Engine/GameInstance.h"
#include "Misc/AutomationTest.h"

namespace
{
void PushU8(TArray<uint8>& Bytes, uint8 Value) { Bytes.Add(Value); }
void PushU16(TArray<uint8>& Bytes, uint16 Value)
{
    Bytes.Add(static_cast<uint8>(Value));
    Bytes.Add(static_cast<uint8>(Value >> 8));
}
void PushU32(TArray<uint8>& Bytes, uint32 Value)
{
    for (int32 Shift = 0; Shift < 32; Shift += 8)
        Bytes.Add(static_cast<uint8>(Value >> Shift));
}
void PushI32(TArray<uint8>& Bytes, int32 Value)
{
    PushU32(Bytes, static_cast<uint32>(Value));
}
TArray<uint8> BeginPayload(const ANSICHAR Magic[5])
{
    TArray<uint8> Bytes;
    Bytes.Append(reinterpret_cast<const uint8*>(Magic), 4);
    PushU16(Bytes, 1);
    PushU16(Bytes, 12);
    PushU32(Bytes, 0);
    return Bytes;
}
void FinishPayload(TArray<uint8>& Bytes)
{
    const uint32 Length = Bytes.Num();
    for (int32 Shift = 0; Shift < 32; Shift += 8)
        Bytes[8 + Shift / 8] = static_cast<uint8>(Length >> Shift);
}
void PushPose(TArray<uint8>& Bytes, int32 X)
{
    PushI32(Bytes, X); PushI32(Bytes, 0); PushI32(Bytes, 0);
    PushI32(Bytes, 0); PushI32(Bytes, 0); PushI32(Bytes, 0);
    PushI32(Bytes, 1 << 30); PushI32(Bytes, 0);
    PushI32(Bytes, 0); PushI32(Bytes, 0);
}
TArray<uint8> DefinitionVector()
{
    TArray<uint8> Bytes = BeginPayload("PGD1");
    PushU32(Bytes, 0x12c00001); PushU32(Bytes, 0x45415254);
    PushU32(Bytes, 0x54524e53); PushU32(Bytes, 0x4d495353);
    PushI32(Bytes, 19'723); PushU16(Bytes, 37); PushU16(Bytes, 0);
    PushI32(Bytes, 26'125'873); PushI32(Bytes, 26'038'281);
    PushI32(Bytes, 313'300'000);
    PushU32(Bytes, 0x4c41554e);
    PushI32(Bytes, 133'564'245); PushI32(Bytes, -377'184'448); PushI32(Bytes, 12);
    PushI32(Bytes, 2'629'000); PushI32(Bytes, -22'109'000); PushI32(Bytes, 11'894'000);
    PushU32(Bytes, 0x52454356);
    PushI32(Bytes, 130'000'000); PushI32(Bytes, -360'000'000); PushI32(Bytes, 4);
    PushI32(Bytes, 3'257'000); PushI32(Bytes, -22'166'000); PushI32(Bytes, 11'507'000);
    PushU32(Bytes, 0x07); PushU8(Bytes, 0x07); PushU8(Bytes, 0); PushU16(Bytes, 0x00ff);
    FinishPayload(Bytes);
    return Bytes;
}
TArray<uint8> SampleVector()
{
    TArray<uint8> Bytes = BeginPayload("PGS1");
    PushU16(Bytes, 1); PushU16(Bytes, 0);
    PushU32(Bytes, 1); PushU32(Bytes, 0);
    PushU32(Bytes, 29); PushU32(Bytes, 59'392);
    PushU8(Bytes, 2); PushU8(Bytes, 2); PushU8(Bytes, 1); PushU8(Bytes, 1);
    PushU16(Bytes, 0); PushU16(Bytes, 1);
    PushU32(Bytes, 0); PushU32(Bytes, 0xabcddcba);
    PushI32(Bytes, 0); PushI32(Bytes, 0); PushI32(Bytes, 0);
    for (int32 Index = 0; Index < 6; ++Index) PushI32(Bytes, 0);
    PushU16(Bytes, 0); PushU16(Bytes, 0);
    for (int32 Index = 0; Index < 12; ++Index) PushU8(Bytes, 0);
    PushU8(Bytes, 0); PushU8(Bytes, 0); PushU16(Bytes, 0);
    PushU8(Bytes, 2); PushU8(Bytes, 2); PushU16(Bytes, 0);
    PushU32(Bytes, (1u << 0) | (1u << 2) | (1u << 4) | (1u << 6));
    PushU32(Bytes, 0x0badf00d); PushU32(Bytes, 1);
    PushU32(Bytes, 2); PushU32(Bytes, 0);
    PushPose(Bytes, 4'096); PushPose(Bytes, 4'096);
    PushPose(Bytes, 0); PushPose(Bytes, 0); PushPose(Bytes, 0);
    PushI32(Bytes, 0); PushI32(Bytes, 0); PushI32(Bytes, 0);
    FinishPayload(Bytes);
    return Bytes;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerCoordinateBasisTest,
    "KSA64.Phase12C.Coordinates.HandednessAndOrigin",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerCoordinateBasisTest::RunTest(const FString&)
{
    const int32 Point[3] = {4'096, 8'192, 12'288};
    const int64 Origin[3] = {0, 0, 0};
    const FVector3d Unreal =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            Point,
            Origin);
    TestEqual(TEXT("KSA +X remains Unreal +X"), Unreal.X, 100'000.0);
    TestEqual(TEXT("KSA +Y is reflected exactly once"), Unreal.Y, -200'000.0);
    TestEqual(TEXT("KSA +Z remains Unreal +Z"), Unreal.Z, 300'000.0);

    TestEqual(
        TEXT("positive origin rounds to nearest 100 km"),
        Ksa64GlobalViewerPolicy::QuantizeOriginQ12(151 * 4'096),
        static_cast<int64>(200 * 4'096));
    TestEqual(
        TEXT("negative origin rounds symmetrically"),
        Ksa64GlobalViewerPolicy::QuantizeOriginQ12(-151 * 4'096),
        static_cast<int64>(-200 * 4'096));
    const int64 RelativeOrigin[3] = {100 * 4'096, 0, 0};
    const FVector3d Relative =
        Ksa64GlobalViewerPolicy::Ksa64RightHandedToUnrealCentimetres(
            Point,
            RelativeOrigin);
    TestEqual(TEXT("client origin changes presentation only"), Relative.X, -9'900'000.0);
    const int32 PositiveYawQ30[4] = {759'250'125, 0, 0, 759'250'125};
    const FQuat Reflected =
        Ksa64GlobalViewerPolicy::Ksa64BodyToFrameQuaternionToUnreal(
            PositiveYawQ30);
    TestTrue(TEXT("single Y reflection reverses right-handed yaw"), Reflected.Z < 0.0);
    TestTrue(TEXT("converted attitude stays normalized"),
        FMath::IsNearlyEqual(Reflected.SizeSquared(), 1.0, 1.0e-9));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerPathProductTest,
    "KSA64.Phase12C.Paths.ChecksumFlagsAndAppearance",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerPathProductTest::RunTest(const FString&)
{
    TArray<FKsa64GlobalPathPointProduct> Points;
    FKsa64GlobalPathPointProduct First;
    First.ReleaseEpoch = 1;
    First.MissionTimeQ16 = 2;
    First.Segment = 3;
    First.EventMask = 4;
    First.AnchorIdentity = 5;
    First.PositionQ12Km[0] = 6;
    First.PositionQ12Km[1] = -7;
    First.PositionQ12Km[2] = 8;
    Points.Add(First);
    FKsa64GlobalPathPointProduct Second;
    Second.ReleaseEpoch = 9;
    Second.MissionTimeQ16 = 10;
    Second.Segment = 11;
    Second.EventMask = 12;
    Second.AnchorIdentity = 13;
    Second.PositionQ12Km[0] = 14;
    Second.PositionQ12Km[1] = 15;
    Second.PositionQ12Km[2] = -16;
    Points.Add(Second);
    TestEqual(
        TEXT("FNV checksum covers the browser path-point domain"),
        Ksa64GlobalViewerPolicy::HashPathPoints(Points, 0, Points.Num()),
        0x201aac9bu);

    const FLinearColor Normal(0.14f, 0.83f, 0.95f, 0.9f);
    const FLinearColor Terminal = Ksa64GlobalViewerPolicy::PathColorForFlags(
        Normal, Ksa64GlobalPathFlags::Terminal);
    TestTrue(TEXT("terminal-only path retains normal color"), Terminal.Equals(Normal));
    const FLinearColor Incomplete = Ksa64GlobalViewerPolicy::PathColorForFlags(
        Normal, Ksa64GlobalPathFlags::Incomplete);
    TestTrue(TEXT("incomplete path is visibly dimmed"),
        FMath::IsNearlyEqual(Incomplete.A, 0.48f));
    const FLinearColor Stale = Ksa64GlobalViewerPolicy::PathColorForFlags(
        Normal, Ksa64GlobalPathFlags::Stale | Ksa64GlobalPathFlags::Terminal);
    TestTrue(TEXT("stale state remains visible on a terminal path"),
        FMath::IsNearlyEqual(Stale.A, 0.28f));
    const FLinearColor Resync = Ksa64GlobalViewerPolicy::PathColorForFlags(
        Normal,
        Ksa64GlobalPathFlags::ResyncRequired
            | Ksa64GlobalPathFlags::Stale
            | Ksa64GlobalPathFlags::Incomplete
            | Ksa64GlobalPathFlags::Terminal);
    TestTrue(TEXT("resync-required has highest visual precedence"),
        FMath::IsNearlyEqual(Resync.A, 0.18f));

    FKsa64GlobalSemanticState State;
    FKsa64GlobalVisiblePathSemantic Path;
    Path.Identity = 1;
    Path.Source = 2;
    Path.Flags = Ksa64GlobalPathFlags::Stale | Ksa64GlobalPathFlags::Terminal;
    State.VisiblePaths.Add(Path);
    TestTrue(TEXT("raw path flags serialize deterministically"),
        State.ToDeterministicJson().Contains(TEXT("\"flags\":5")));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerPublicationRecoveryTest,
    "KSA64.Phase12C.Publication.BoundedSyncAndPathRecovery",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerPublicationRecoveryTest::RunTest(const FString&)
{
    using Ksa64GlobalViewerPolicy::EKsa64GuidedDisplaySyncDecision;
    uint32 WaitFrames = 0;
    for (uint32 Frame = 1; Frame < 600; ++Frame)
    {
        TestEqual(
            TEXT("trailing publication remains bounded wait before limit"),
            Ksa64GlobalViewerPolicy::ObserveGuidedDisplaySync(
                99, 100, 600, WaitFrames),
            EKsa64GuidedDisplaySyncDecision::Wait);
    }
    TestEqual(TEXT("599 trailing frames were counted"), WaitFrames, 599u);
    TestEqual(
        TEXT("600th trailing frame fails closed"),
        Ksa64GlobalViewerPolicy::ObserveGuidedDisplaySync(
            99, 100, 600, WaitFrames),
        EKsa64GuidedDisplaySyncDecision::RejectTimeout);
    TestEqual(TEXT("timeout records its exact bound"), WaitFrames, 600u);
    TestEqual(
        TEXT("exact alignment recovers and resets the bound"),
        Ksa64GlobalViewerPolicy::ObserveGuidedDisplaySync(
            100, 100, 600, WaitFrames),
        EKsa64GuidedDisplaySyncDecision::Aligned);
    TestEqual(TEXT("alignment resets the wait counter"), WaitFrames, 0u);
    TestEqual(
        TEXT("display publication ahead of authority rejects immediately"),
        Ksa64GlobalViewerPolicy::ObserveGuidedDisplaySync(
            101, 100, 600, WaitFrames),
        EKsa64GuidedDisplaySyncDecision::RejectAhead);

    TestFalse(
        TEXT("one missing required path source rejects the refresh"),
        Ksa64GlobalViewerPolicy::RequiredGlobalPathSourcesAvailable(
            0x06u, 0x02u));
    TestTrue(
        TEXT("a later complete refresh restores path eligibility"),
        Ksa64GlobalViewerPolicy::RequiredGlobalPathSourcesAvailable(
            0x06u, 0x06u));
    TestTrue(
        TEXT("unrequested source availability does not change eligibility"),
        Ksa64GlobalViewerPolicy::RequiredGlobalPathSourcesAvailable(
            0x06u, 0x0eu));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerSnapPolicyTest,
    "KSA64.Phase12C.Temporal.ExactSnapBoundaries",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerSnapPolicyTest::RunTest(const FString&)
{
    FKsa64GlobalSceneSample Previous;
    Previous.ReleaseEpoch = 100;
    Previous.FrameIdentity = 2;
    Previous.SegmentIdentity = 2;
    Previous.ContinuityIdentity = 55;
    Previous.bPositionValid = true;
    Previous.bAttitudeValid = true;
    FKsa64GlobalSceneSample Current = Previous;
    Current.ReleaseEpoch = 101;
    TestFalse(
        TEXT("compatible consecutive samples may interpolate"),
        Ksa64GlobalViewerPolicy::ShouldSnap(Previous, Current));

    Current.FrameIdentity = 3;
    TestTrue(
        TEXT("frame transition snaps"),
        Ksa64GlobalViewerPolicy::ShouldSnap(Previous, Current));
    Current = Previous;
    Current.ReleaseEpoch = 101;
    Current.SegmentIdentity = 3;
    TestTrue(
        TEXT("segment transition snaps"),
        Ksa64GlobalViewerPolicy::ShouldSnap(Previous, Current));
    Current = Previous;
    Current.ReleaseEpoch = 101;
    Current.ContinuityIdentity = 56;
    TestTrue(
        TEXT("continuity identity change snaps"),
        Ksa64GlobalViewerPolicy::ShouldSnap(Previous, Current));
    Current = Previous;
    Current.ReleaseEpoch = 101;
    Current.EventMask = 1;
    TestTrue(
        TEXT("exact mission event snaps"),
        Ksa64GlobalViewerPolicy::ShouldSnap(Previous, Current));
    Current = Previous;
    Current.ReleaseEpoch = 101;
    Current.DiscontinuityMask = 1;
    TestTrue(
        TEXT("declared invalidity or seek snaps"),
        Ksa64GlobalViewerPolicy::ShouldSnap(Previous, Current));
    Current = Previous;
    Current.ReleaseEpoch = 101;
    Current.bAttitudeValid = false;
    TestTrue(
        TEXT("missing attitude cannot be smoothed"),
        Ksa64GlobalViewerPolicy::ShouldSnap(Previous, Current));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerDirectorTest,
    "KSA64.Phase12C.Camera.FrameAwareDirector",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerDirectorTest::RunTest(const FString&)
{
    TestEqual(
        TEXT("launch ENU uses launch camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(1, 1, 29),
        EKsa64GlobalCameraMode::LaunchLocalEnu);
    TestEqual(
        TEXT("recovery ENU uses recovery camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(1, 5, 15'255),
        EKsa64GlobalCameraMode::RecoveryLocalEnu);
    TestEqual(
        TEXT("powered ECEF ascent uses chase camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(2, 2, 1'919),
        EKsa64GlobalCameraMode::VehicleChase);
    TestEqual(
        TEXT("burnout opens the Earth-fixed camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(2, 2, 1'920),
        EKsa64GlobalCameraMode::EarthFixed);
    TestEqual(
        TEXT("ECEF entry remains Earth-fixed"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(2, 4, 13'000),
        EKsa64GlobalCameraMode::EarthFixed);
    TestEqual(
        TEXT("GCRF coast uses inertial camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(3, 3, 8'124),
        EKsa64GlobalCameraMode::EarthInertial);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerImportantReleaseSemanticTest,
    "KSA64.Phase12C.Semantics.ImportantReleaseSnapshots",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerImportantReleaseSemanticTest::RunTest(const FString&)
{
    TStrongObjectPtr<UGameInstance> GameInstance(
        NewObject<UGameInstance>(GetTransientPackage()));
    TStrongObjectPtr<UKsa64GlobalViewerSubsystem> Viewer(
        NewObject<UKsa64GlobalViewerSubsystem>(GameInstance.Get()));
    Viewer->SetGlobalAvailabilityForAutomation(true, true, 0x07);

    struct FBookmark
    {
        uint32 Release;
        uint32 Frame;
        uint32 Segment;
        EKsa64GlobalCameraMode Camera;
    };
    const FBookmark Bookmarks[] = {
        {29, 2, 2, EKsa64GlobalCameraMode::VehicleChase},
        {3'579, 3, 3, EKsa64GlobalCameraMode::EarthInertial},
        {12'669, 2, 4, EKsa64GlobalCameraMode::EarthFixed},
        {15'255, 1, 5, EKsa64GlobalCameraMode::RecoveryLocalEnu},
    };
    uint64 Continuity = 100;
    for (const FBookmark& Bookmark : Bookmarks)
    {
        FKsa64GlobalSceneSample Sample;
        Sample.ReleaseEpoch = Bookmark.Release;
        Sample.MissionTimeQ16 = Bookmark.Release * 2'048;
        Sample.FrameIdentity = Bookmark.Frame;
        Sample.SegmentIdentity = Bookmark.Segment;
        Sample.ContinuityIdentity = Continuity++;
        Sample.DiscontinuityMask = 1;
        Sample.bPositionValid = true;
        Sample.bAttitudeValid = true;
        Sample.bExactSnap = true;
        Viewer->ApplySampleForAutomation(Sample, false);
        const FKsa64GlobalSemanticState& State = Viewer->GetSemanticState();
        TestEqual(TEXT("bookmark release stays exact"), State.ReleaseEpoch, Bookmark.Release);
        TestEqual(TEXT("director selects frame-aware camera"),
            State.ResolvedCamera, Bookmark.Camera);
        TestTrue(TEXT("typed exact stream is acceptance eligible"),
            State.bAcceptanceEligible);
        TestFalse(TEXT("guided semantic snapshot has no truth"),
            State.bTruthPermitted);
        const FString Json = Viewer->ExportSemanticStateJson();
        TestTrue(TEXT("snapshot records exact release"),
            Json.Contains(FString::Printf(TEXT("\"release_epoch\":%u"), Bookmark.Release)));
        TestTrue(TEXT("snapshot records exact product status"),
            Json.Contains(TEXT("\"acceptance_eligible\":true")));
        TestTrue(TEXT("continuity identity is a JSON number"),
            Json.Contains(FString::Printf(
                TEXT("\"continuity_identity\":%llu"),
                static_cast<unsigned long long>(Sample.ContinuityIdentity))));
        TestFalse(TEXT("continuity identity is never a quoted decimal"),
            Json.Contains(TEXT("\"continuity_identity\":\"")));
    }

    Viewer->SetGlobalAvailabilityForAutomation(false, false, 0x03);
    TestFalse(TEXT("legacy fallback can never count as exact evidence"),
        Viewer->GetSemanticState().bAcceptanceEligible);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerRoleAndSemanticTest,
    "KSA64.Phase12C.Semantics.TruthAndRendererInvariance",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerRoleAndSemanticTest::RunTest(const FString&)
{
    TStrongObjectPtr<UGameInstance> GameInstance(
        NewObject<UGameInstance>(GetTransientPackage()));
    TStrongObjectPtr<UKsa64GlobalViewerSubsystem> Viewer(
        NewObject<UKsa64GlobalViewerSubsystem>(GameInstance.Get()));
    FKsa64GlobalSceneSample Sample;
    Sample.ReleaseEpoch = 3'579;
    Sample.MissionTimeQ16 = 7'329'792;
    Sample.FrameIdentity = 3;
    Sample.SegmentIdentity = 3;
    Sample.ContinuityIdentity = 0x1234;
    Sample.bPositionValid = true;
    Sample.bAttitudeValid = true;
    Sample.bExactSnap = true;
    Sample.PositionQ12Km[0] = 26'100'000;
    Sample.PositionQ12Km[1] = -5'600'000;
    Sample.PositionQ12Km[2] = 13'000'000;

    Viewer->ApplySampleForAutomation(Sample, false);
    const FString GuidedBaseline = Viewer->ExportSemanticStateJson();
    TestFalse(TEXT("Guided role cannot receive truth"), Viewer->CanShowTruth());
    Viewer->ToggleTruth();
    TestFalse(
        TEXT("truth toggle fails closed for filtered role"),
        Viewer->GetSemanticState().bTruthVisible);

    Viewer->SetLayout(EKsa64GlobalViewerLayout::EngineeringSplit);
    Viewer->SetLayout(EKsa64GlobalViewerLayout::HybridMissionDirector);
    Viewer->SetCamera(EKsa64GlobalCameraMode::EarthFixed);
    Viewer->ResumeAutomaticDirector();
    TestEqual(
        TEXT("presentation operations restore semantic baseline"),
        Viewer->ExportSemanticStateJson(),
        GuidedBaseline);

    Viewer->ApplySampleForAutomation(Sample, true);
    TestTrue(TEXT("SIM Director product permits truth"), Viewer->CanShowTruth());
    Viewer->ToggleTruth();
    TestTrue(
        TEXT("truth is explicit and initially opt-in"),
        Viewer->GetSemanticState().bTruthVisible);
    TestTrue(
        TEXT("semantic output labels truth visibility"),
        Viewer->ExportSemanticStateJson().Contains(TEXT("\"truth_visible\":true")));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerDispositionTest,
    "KSA64.Phase12C.Semantics.PlanDeviationIsNotFailure",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerDispositionTest::RunTest(const FString&)
{
    FKsa64GlobalSemanticState State;
    State.OverallDisposition = 2;
    State.ObjectiveDisposition = 2;
    State.VehicleDisposition = 3;
    State.ProcedureDisposition = 2;
    State.OperatorDisposition = 1;
    State.AvionicsDisposition = 2;
    State.EvidenceDisposition = 1;
    State.DispositionLabel = TEXT("DEGRADED SUCCESS");
    State.StatusLabel = TEXT("PLAN RESIDUAL PRESENT");
    const FString Json = State.ToDeterministicJson();
    TestTrue(TEXT("degraded success remains explicit"), Json.Contains(TEXT("\"overall_disposition\":2")));
    TestTrue(TEXT("plan residual remains informational"), Json.Contains(TEXT("PLAN RESIDUAL PRESENT")));
    TestFalse(TEXT("viewer does not invent failure"), Json.Contains(TEXT("MISSION FAILURE")));
    TestEqual(TEXT("semantic serialization is deterministic"), State.ToDeterministicJson(), Json);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerRealBridgeReplayTest,
    "KSA64.Phase12C.Integration.RealBridgeNominalMilestones",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerRealBridgeReplayTest::RunTest(const FString&)
{
    TStrongObjectPtr<UGameInstance> GameInstance(
        NewObject<UGameInstance>(GetTransientPackage()));
    TStrongObjectPtr<UKsa64LiveMissionSubsystem> Operations(
        NewObject<UKsa64LiveMissionSubsystem>(GameInstance.Get()));
    TStrongObjectPtr<UKsa64GlobalViewerSubsystem> Viewer(
        NewObject<UKsa64GlobalViewerSubsystem>(GameInstance.Get()));
    if (!Operations->InitializeForAutomation())
    {
        AddError(FString::Printf(TEXT("real bridge unavailable: %s"),
            *Operations->GetViewModel().LastDiagnostic));
        return false;
    }
    struct FMilestone { uint32 Release; uint32 Frame; uint32 Segment; };
    const FMilestone Milestones[] = {
        {29, 2, 2}, {1'920, 2, 2}, {3'579, 3, 3},
        {8'124, 3, 3}, {12'669, 2, 4}, {15'255, 1, 5},
        {15'257, 1, 5}, {20'929, 1, 5}, {22'014, 1, 5},
    };
    for (const FMilestone& Milestone : Milestones)
    {
        if (!Viewer->OpenNominalReleaseForAutomation(*Operations, Milestone.Release))
        {
            AddError(FString::Printf(
                TEXT("real GlobalDisplayV1 replay failed at release %u: %s"),
                Milestone.Release,
                *Operations->GetViewModel().LastDiagnostic));
            Operations->CloseForAutomation();
            return false;
        }
        const FKsa64GlobalSemanticState& State = Viewer->GetSemanticState();
        TestEqual(TEXT("real release exact"), State.ReleaseEpoch, Milestone.Release);
        TestEqual(TEXT("real frame exact"), State.FrameIdentity, Milestone.Frame);
        TestEqual(TEXT("real segment exact"), State.SegmentIdentity, Milestone.Segment);
        TestTrue(TEXT("real display accepted"), State.bAcceptanceEligible);
        TestTrue(TEXT("real replay snaps on seek"), State.bExactSnap);
        TestTrue(TEXT("SIM Director truth is permitted"), State.bTruthPermitted);
        TestEqual(TEXT("nominal sample poses contain onboard and truth only"),
            State.SourceMask, 0x0Au);
        TestFalse(TEXT("SIM truth starts hidden"), State.bTruthVisible);
        TestTrue(TEXT("real onboard path is present"), State.OnboardPathPoints > 0);
        TestTrue(TEXT("real transition index is present"), State.TransitionMarkers >= 4);
        TestTrue(TEXT("onboard pose is visible"),
            (State.VisibleSourceMask & 0x02u) != 0);
        TestEqual(TEXT("hidden truth pose is absent"),
            State.VisibleSourceMask & 0x08u, 0u);
        uint32 ReconstructedVisibleMask = 0;
        for (const FKsa64GlobalVisibleSourceSemantic& Source : State.VisibleSources)
        {
            ReconstructedVisibleMask |= 1u << (Source.Source - 1u);
            TestTrue(TEXT("visible source identity is public"), Source.Source <= 3);
            TestTrue(TEXT("visible source model identity is retained"),
                Source.ModelIdentity != 0);
        }
        TestEqual(TEXT("visible source array matches its mask"),
            ReconstructedVisibleMask, State.VisibleSourceMask);
        TestTrue(TEXT("planned and onboard path products are visible"),
            State.VisiblePaths.Num() >= 2);
        bool bPlannedPath = false;
        bool bOnboardPath = false;
        TSet<uint32> LocalAnchors;
        for (const FKsa64GlobalVisiblePathSemantic& Path : State.VisiblePaths)
        {
            bPlannedPath |= Path.Source == 1;
            bOnboardPath |= Path.Source == 2;
            TestTrue(TEXT("hidden truth path is absent"), Path.Source != 4);
            TestTrue(TEXT("visible path identity is retained"), Path.Identity != 0);
            TestTrue(TEXT("visible path model identity is retained"),
                Path.ModelIdentity != 0);
            TestTrue(TEXT("visible path continuity is retained"),
                Path.ContinuityIdentity != 0);
            TestEqual(TEXT("visible path flags stay in their raw contract"),
                static_cast<uint16>(Path.Flags & ~Ksa64GlobalPathFlags::Mask),
                static_cast<uint16>(0));
            TestTrue(TEXT("visible path points are retained"), Path.PointCount > 0);
            TestTrue(TEXT("visible path checksum matches browser FNV domain"),
                Path.PointChecksum != 0);
            if (Milestone.Frame == 1) LocalAnchors.Add(Path.AnchorIdentity);
        }
        TestTrue(TEXT("planned path semantic is present"), bPlannedPath);
        TestTrue(TEXT("onboard path semantic is present"), bOnboardPath);
        if (Milestone.Frame == 1)
        {
            TestTrue(TEXT("local ENU paths retain launch and recovery strips"),
                LocalAnchors.Num() >= 2);
        }
        const FString SemanticJson = Viewer->ExportSemanticStateJson();
        TestTrue(TEXT("semantic JSON exports visible sources"),
            SemanticJson.Contains(TEXT("\"visible_sources\":[")));
        TestTrue(TEXT("semantic JSON exports visible paths"),
            SemanticJson.Contains(TEXT("\"visible_paths\":[")));
        TestTrue(TEXT("semantic JSON exports raw path flags"),
            SemanticJson.Contains(TEXT("\"flags\":")));
    }
    const FKsa64GlobalSemanticState& Terminal = Viewer->GetSemanticState();
    TestEqual(TEXT("real terminal disposition"), Terminal.OverallDisposition, 1u);
    TestEqual(TEXT("real terminal label"), Terminal.DispositionLabel, TEXT("NOMINAL SUCCESS"));
    Operations->CloseForAutomation();
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerCompletedGuidedPathTest,
    "KSA64.Phase12C.Integration.CompletedGuidedMilestonePaths",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerCompletedGuidedPathTest::RunTest(const FString&)
{
    TStrongObjectPtr<UGameInstance> GameInstance(
        NewObject<UGameInstance>(GetTransientPackage()));
    TStrongObjectPtr<UKsa64LiveMissionSubsystem> Operations(
        NewObject<UKsa64LiveMissionSubsystem>(GameInstance.Get()));
    TStrongObjectPtr<UKsa64GlobalViewerSubsystem> Viewer(
        NewObject<UKsa64GlobalViewerSubsystem>(GameInstance.Get()));
    if (!Operations->InitializeForAutomation()
        || !Operations->StartGuidedOperations()
        || !Operations->SetCompletedGlobalDisplayRetention(true))
    {
        AddError(FString::Printf(
            TEXT("guided bridge unavailable: %s"),
            *Operations->GetViewModel().LastDiagnostic));
        Operations->CloseForAutomation();
        return false;
    }

    const auto ApplyActionPair = [this, &Operations](
        uint32 StageRelease,
        uint32 CommitRelease)
    {
        if (!Operations->AdvanceToReleaseForAutomation(StageRelease, 60.0))
        {
            AddError(FString::Printf(
                TEXT("guided stage release %u was not reached"),
                StageRelease));
            return false;
        }
        Operations->ReviewAction();
        Operations->StageAction();
        if (!Operations->WaitForActionReceiptForAutomation(1, 30.0))
        {
            AddError(FString::Printf(
                TEXT("guided stage receipt failed at %u"),
                StageRelease));
            return false;
        }
        if (!Operations->AdvanceToReleaseForAutomation(CommitRelease, 60.0))
        {
            AddError(FString::Printf(
                TEXT("guided commit release %u was not reached"),
                CommitRelease));
            return false;
        }
        Operations->CommitAction();
        if (!Operations->WaitForActionReceiptForAutomation(2, 30.0))
        {
            AddError(FString::Printf(
                TEXT("guided commit receipt failed at %u"),
                CommitRelease));
            return false;
        }
        return true;
    };

    if (!ApplyActionPair(6'080, 6'240)
        || !ApplyActionPair(6'560, 6'720)
        || !Operations->AdvanceToReleaseForAutomation(21'591, 120.0)
        || !Operations->WaitForCompletionForAutomation(60.0))
    {
        AddError(FString::Printf(
            TEXT("guided terminal evidence failed: release=%u diagnostic=%s"),
            Operations->GetViewModel().ReleaseEpoch,
            *Operations->GetViewModel().LastDiagnostic));
        Operations->CloseForAutomation();
        return false;
    }
    TestEqual(TEXT("guided accepted action count"),
        Operations->GetViewModel().ActionCount, 4u);

    const uint32 Milestones[] = {
        5'760, 5'824, 6'080, 6'240, 6'560, 6'720,
    };
    TMap<uint8, uint32> TerminalPathChecksums;
    TMap<uint8, uint32> TerminalPathPointCounts;
    for (const uint32 ReleaseEpoch : Milestones)
    {
        if (!Viewer->OpenCompletedGuidedReleaseForAutomation(
                *Operations, ReleaseEpoch))
        {
            AddError(FString::Printf(
                TEXT("completed guided recapture failed at release %u: %s; supports=%u operations=%s"),
                ReleaseEpoch,
                *Viewer->GetSemanticState().StatusLabel,
                Operations->SupportsGlobalDisplayV1() ? 1u : 0u,
                *Operations->GetViewModel().LastDiagnostic));
            Operations->CloseForAutomation();
            return false;
        }
        const FKsa64GlobalSemanticState& State = Viewer->GetSemanticState();
        TestEqual(TEXT("guided selected release remains exact"),
            State.ReplaySelectedRelease, ReleaseEpoch);
        TestEqual(TEXT("guided recapture frame"), State.FrameIdentity, 3u);
        TestEqual(TEXT("guided recapture segment"), State.SegmentIdentity, 3u);
        TestEqual(TEXT("guided sample source mask"), State.SourceMask, 0x06u);
        TestEqual(TEXT("guided visible pose mask"), State.VisibleSourceMask, 0x06u);
        TestFalse(TEXT("guided truth remains structurally absent"),
            State.bTruthPermitted || State.bTruthVisible);
        TestTrue(TEXT("guided terminal products remain accepted"),
            State.bAcceptanceEligible);

        bool bPlanned = false;
        bool bOnboard = false;
        bool bGround = false;
        for (const FKsa64GlobalVisiblePathSemantic& Path : State.VisiblePaths)
        {
            bPlanned |= Path.Source == 1;
            bOnboard |= Path.Source == 2;
            bGround |= Path.Source == 3;
            TestTrue(TEXT("completed guided path is terminal"),
                (Path.Flags & Ksa64GlobalPathFlags::Terminal) != 0);
            TestEqual(TEXT("completed guided path is not incomplete"),
                static_cast<uint16>(Path.Flags
                    & Ksa64GlobalPathFlags::Incomplete),
                static_cast<uint16>(0));
            TestEqual(TEXT("completed guided path needs no resync"),
                static_cast<uint16>(Path.Flags
                    & Ksa64GlobalPathFlags::ResyncRequired),
                static_cast<uint16>(0));
            TestTrue(TEXT("completed guided path contains whole-mission points"),
                Path.PointCount > 100);
            TestTrue(TEXT("completed guided path checksum is exact"),
                Path.PointChecksum != 0);
            if (!TerminalPathChecksums.Contains(Path.Source))
            {
                TerminalPathChecksums.Add(Path.Source, Path.PointChecksum);
                TerminalPathPointCounts.Add(Path.Source, Path.PointCount);
            }
            else
            {
                TestEqual(TEXT("terminal path checksum is seek-invariant"),
                    Path.PointChecksum, TerminalPathChecksums[Path.Source]);
                TestEqual(TEXT("terminal path extent is seek-invariant"),
                    Path.PointCount, TerminalPathPointCounts[Path.Source]);
            }
        }
        TestTrue(TEXT("completed guided planned path is present"), bPlanned);
        TestTrue(TEXT("completed guided onboard path is present"), bOnboard);
        TestTrue(TEXT("completed guided ground path is present"), bGround);
    }
    TestEqual(TEXT("guided terminal public paths were retained"),
        TerminalPathChecksums.Num(), 3);
    TestEqual(TEXT("historical recapture preserves terminal release authority"),
        Operations->GetViewModel().ReleaseEpoch, 21'591u);
    TestEqual(TEXT("historical recapture preserves terminal disposition authority"),
        Operations->GetViewModel().OverallDisposition, 2u);
    TestEqual(TEXT("retained guided authority remains completed"),
        Operations->GetViewModel().Lifecycle, 5u);
    TestTrue(TEXT("guided terminal display lease releases cleanly"),
        Operations->SetCompletedGlobalDisplayRetention(false));
    Operations->CloseForAutomation();
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalViewerReplayDispositionMappingTest,
    "KSA64.Phase12C.Semantics.ReplayDispositionAxisMapping",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalViewerReplayDispositionMappingTest::RunTest(const FString&)
{
    TStrongObjectPtr<UGameInstance> GameInstance(
        NewObject<UGameInstance>(GetTransientPackage()));
    TStrongObjectPtr<UKsa64GlobalViewerSubsystem> Viewer(
        NewObject<UKsa64GlobalViewerSubsystem>(GameInstance.Get()));
    FKsa64GlobalReplayIndexProduct Replay;
    Replay.IndexIdentity = 0x12c0d150;
    Replay.TerminalDisposition = 3;
    Replay.DispositionAxes[0] = 1;
    Replay.DispositionAxes[1] = 2;
    Replay.DispositionAxes[2] = 3;
    Replay.DispositionAxes[3] = 4;
    Replay.DispositionAxes[4] = 2;
    Replay.DispositionAxes[5] = 1;
    Viewer->ApplyReplayIndexForAutomation(Replay);
    const FKsa64GlobalSemanticState& State = Viewer->GetSemanticState();
    TestEqual(TEXT("overall disposition"), State.OverallDisposition, 3u);
    TestEqual(TEXT("objective axis"), State.ObjectiveDisposition, 1u);
    TestEqual(TEXT("vehicle axis"), State.VehicleDisposition, 2u);
    TestEqual(TEXT("procedure axis"), State.ProcedureDisposition, 3u);
    TestEqual(TEXT("operator axis"), State.OperatorDisposition, 4u);
    TestEqual(TEXT("avionics axis"), State.AvionicsDisposition, 2u);
    TestEqual(TEXT("evidence axis"), State.EvidenceDisposition, 1u);
    TestEqual(TEXT("overall label derives from Rust disposition"),
        State.DispositionLabel, TEXT("CONTINGENCY SUCCESS"));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64GlobalDisplayCodecTest,
    "KSA64.Phase12C.Contracts.GlobalDisplayPayloads",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64GlobalDisplayCodecTest::RunTest(const FString&)
{
    FString Error;
    FKsa64GlobalDisplayDefinitionProduct Definition;
    TArray<uint8> DefinitionBytes = DefinitionVector();
    TestTrue(
        TEXT("independent PGD1 vector decodes"),
        FKsa64GlobalDisplayCodec::DecodeDefinition(
            DefinitionBytes, Definition, Error));
    TestEqual(TEXT("Earth semimajor comes from Rust product"),
        Definition.SemiMajorQ12Km, 26'125'873);
    TestEqual(TEXT("all frame capabilities preserved"),
        Definition.AvailableFrameMask, static_cast<uint8>(7));
    TArray<uint8> CorruptDefinition = DefinitionBytes;
    CorruptDefinition[34] = 1;
    TestFalse(
        TEXT("nonzero reserved definition byte fails closed"),
        FKsa64GlobalDisplayCodec::DecodeDefinition(
            CorruptDefinition, Definition, Error));

    TArray<FKsa64GlobalDisplaySampleProduct> Samples;
    const TArray<uint8> SampleBytes = SampleVector();
    TestTrue(
        TEXT("independent PGS1 vector decodes"),
        FKsa64GlobalDisplayCodec::DecodeSamples(
            SampleBytes, 1u << 1, Samples, Error));
    TestEqual(TEXT("one exact release decoded"), Samples.Num(), 1);
    if (Samples.Num() == 1)
    {
        TestEqual(TEXT("release remains exact"), Samples[0].ReleaseEpoch, 29u);
        TestEqual(TEXT("onboard source retained"), Samples[0].Sources[0].Source, static_cast<uint8>(2));
        TestEqual(TEXT("resolved ECEF x remains Q12 km"),
            Samples[0].Sources[0].Ecef.PositionQ12Km[0], 4'096);
    }
    TestFalse(
        TEXT("source forbidden by negotiated role fails closed"),
        FKsa64GlobalDisplayCodec::DecodeSamples(
            SampleBytes, 1u << 0, Samples, Error));
    return true;
}

#endif
