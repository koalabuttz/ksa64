#if WITH_DEV_AUTOMATION_TESTS

#include "Ksa64GlobalViewerPolicy.h"
#include "Ksa64GlobalViewerSubsystem.h"

#include "Engine/GameInstance.h"
#include "Misc/AutomationTest.h"

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
        TEXT("ECEF ascent uses Earth-fixed camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(2, 2, 2'000),
        EKsa64GlobalCameraMode::EarthFixed);
    TestEqual(
        TEXT("ECEF entry uses chase camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(2, 4, 13'000),
        EKsa64GlobalCameraMode::VehicleChase);
    TestEqual(
        TEXT("GCRF coast uses inertial camera"),
        Ksa64GlobalViewerPolicy::ResolveAutomaticCamera(3, 3, 8'124),
        EKsa64GlobalCameraMode::EarthInertial);
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

#endif
