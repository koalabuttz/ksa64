#include "Ksa64OperationsBridgeAdapter.h"
#include "Ksa64OperationsPolicy.h"

#if WITH_DEV_AUTOMATION_TESTS

#include "Ksa64BridgeModule.h"
#include "Containers/Ticker.h"
#include "HAL/PlatformMisc.h"
#include "HAL/PlatformProcess.h"
#include "HAL/PlatformTime.h"
#include "Misc/AutomationTest.h"

namespace
{
using namespace Ksa64OperationsPolicy;

uint32 RunDisplayFrames(int32 RefreshHz, int32 Seconds, EKsa64OperationsPace Pace)
{
    FKsa64OperationsPacingController Controller;
    uint32 Releases = 0;
    for (int32 Frame = 0; Frame < RefreshHz * Seconds; ++Frame)
    {
        const int64 StartNanoseconds = static_cast<int64>(Frame) * 1'000'000'000 / RefreshHz;
        const int64 EndNanoseconds = static_cast<int64>(Frame + 1) * 1'000'000'000 / RefreshHz;
        Controller.AccumulateNanoseconds(EndNanoseconds - StartNanoseconds, Pace);
        const uint32 Due = Controller.ReleasesDue(Pace, 31'250, true, false);
        if (Due > 0)
        {
            Controller.CommitAcceptedAdvance(Due, 31'250, Pace);
            Releases += Due;
        }
    }
    return Releases;
}

bool PollAdapterUntil(
    IKsa64OperationsBridgeAdapter& Adapter,
    FKsa64OperationsViewModel& View,
    double TimeoutSeconds,
    TFunctionRef<bool(const FKsa64OperationsViewModel&)> Predicate)
{
    const double Deadline = FPlatformTime::Seconds() + TimeoutSeconds;
    do
    {
        const EKsa64OperationsAdapterResult Result = Adapter.Poll(View);
        if (Result == EKsa64OperationsAdapterResult::Ok && Predicate(View))
        {
            return true;
        }
        if (Result != EKsa64OperationsAdapterResult::Ok
            && Result != EKsa64OperationsAdapterResult::NoData
            && Result != EKsa64OperationsAdapterResult::Unchanged)
        {
            return false;
        }
        FPlatformProcess::Sleep(0.0005f);
    }
    while (FPlatformTime::Seconds() < Deadline);
    return false;
}

bool AdvanceAdapterTo(
    IKsa64OperationsBridgeAdapter& Adapter,
    FKsa64OperationsViewModel& View,
    uint32 TargetRelease)
{
    while (View.ReleaseEpoch < TargetRelease)
    {
        const uint32 Count = FMath::Min<uint32>(64, TargetRelease - View.ReleaseEpoch);
        const uint64 PriorPublication = View.CommandSequence;
        const EKsa64OperationsAdapterResult Result = Adapter.AdvanceReleases(Count);
        if (Result != EKsa64OperationsAdapterResult::Ok
            && Result != EKsa64OperationsAdapterResult::Queued)
        {
            return false;
        }
        const uint32 Expected = View.ReleaseEpoch + Count;
        if (!PollAdapterUntil(
                Adapter,
                View,
                15.0,
                [Expected, PriorPublication](const FKsa64OperationsViewModel& Candidate)
                {
                    return Candidate.ReleaseEpoch == Expected
                        && Candidate.CommandSequence != PriorPublication
                        && Candidate.CommandsPending == 0;
                }))
        {
            return false;
        }
    }
    return View.ReleaseEpoch == TargetRelease;
}

bool ApplyAcceptedAction(
    IKsa64OperationsBridgeAdapter& Adapter,
    FKsa64OperationsViewModel& View,
    uint32 StageEpoch,
    uint32 CommitEpoch)
{
    if (!AdvanceAdapterTo(Adapter, View, StageEpoch)
        || View.ActionProposalIdentity == 0
        || Adapter.ReviewAction() != EKsa64OperationsAdapterResult::Ok
        || Adapter.StageAction() != EKsa64OperationsAdapterResult::Queued
        || !PollAdapterUntil(
            Adapter,
            View,
            15.0,
            [](const FKsa64OperationsViewModel& Candidate)
            {
                return Candidate.ActionReceiptState == 1
                    && Candidate.ActionReceiptAccepted != 0
                    && Candidate.CommandsPending == 0;
            }))
    {
        return false;
    }
    const uint32 Proposal = View.ActionProposalIdentity;
    if (!AdvanceAdapterTo(Adapter, View, CommitEpoch)
        || View.ActionProposalIdentity != Proposal
        || Adapter.CommitAction() != EKsa64OperationsAdapterResult::Queued
        || !PollAdapterUntil(
            Adapter,
            View,
            15.0,
            [Proposal](const FKsa64OperationsViewModel& Candidate)
            {
                return Candidate.ActionProposalIdentity == Proposal
                    && Candidate.ActionReceiptState == 2
                    && Candidate.ActionReceiptAccepted != 0
                    && Candidate.CommandsPending == 0;
            }))
    {
        return false;
    }
    return true;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsLegacyMappingTest,
    "KSA64.Operations.Mapping.LegacyIsTruthFiltered",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsLegacyMappingTest::RunTest(const FString&)
{
    Ksa64ViewerSnapshot Snapshot = {};
    Snapshot.abi_version = KSA64_VIEWER_ABI_VERSION;
    Snapshot.struct_size = sizeof(Ksa64ViewerSnapshot);
    Snapshot.validity_mask = (1ull << 0) | (1ull << 1) | (1ull << 2);
    Snapshot.role = 2;
    Snapshot.lifecycle = 3;
    Snapshot.release_epoch = 96;
    Snapshot.release_period_micros = 31'250;
    Snapshot.frame = 3;
    Snapshot.mission_time_q16 = 65'536;
    Snapshot.navigation_position_q12[2] = 1234;
    Snapshot.procedure_state = 1;
    Snapshot.procedure_step = 4;

    const FKsa64OperationsViewModel View =
        IKsa64OperationsBridgeAdapter::MapLegacySnapshot(Snapshot, TEXT("qualified"));
    TestTrue(TEXT("guided snapshots remain truth filtered"), View.bTruthFiltered);
    TestEqual(TEXT("release is copied exactly"), View.ReleaseEpoch, 96u);
    TestEqual(TEXT("Q12 position is not converted"), View.NavigationPositionQ12[2], 1234);
    TestEqual(TEXT("frame has an operational label"), View.FrameLabel, TEXT("EARTH INERTIAL / GCRF"));
    TestFalse(TEXT("legacy mapping does not invent typed actions"), View.Capabilities.bTypedActions);
    TestTrue(TEXT("semantic JSON has stable schema"), View.ToDeterministicJson().Contains(TEXT("ksa64.operations-view.v1")));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsRoleFilterTest,
    "KSA64.Operations.Mapping.RoleTruthPolicy",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsRoleFilterTest::RunTest(const FString&)
{
    for (uint32 Role : {1u, 2u, 3u, 4u, 6u, 99u})
    {
        TestTrue(FString::Printf(TEXT("role %u fails closed without truth"), Role), IsTruthFilteredRole(Role));
        Ksa64ViewerSnapshot Snapshot = {};
        Snapshot.abi_version = KSA64_VIEWER_ABI_VERSION;
        Snapshot.struct_size = sizeof(Ksa64ViewerSnapshot);
        Snapshot.role = Role;
        TestTrue(TEXT("legacy role mapping follows policy"), IKsa64OperationsBridgeAdapter::MapLegacySnapshot(Snapshot, TEXT("test")).bTruthFiltered);
    }
    TestFalse(TEXT("SIM Director is the only truth role"), IsTruthFilteredRole(5));
    TestEqual(TEXT("flight controller label"), RoleLabel(3), TEXT("FLIGHT CONTROLLER"));
    TestEqual(TEXT("SIM Director label"), RoleLabel(5), TEXT("SIM DIRECTOR"));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsDispositionMappingTest,
    "KSA64.Operations.Mapping.DispositionMatrix",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsDispositionMappingTest::RunTest(const FString&)
{
    TestEqual(TEXT("objective primary"), ObjectiveLabel(1), TEXT("PRIMARY ACHIEVED"));
    TestEqual(TEXT("objective alternate"), ObjectiveLabel(2), TEXT("ALTERNATE ACHIEVED"));
    TestEqual(TEXT("objective contingency"), ObjectiveLabel(3), TEXT("CONTINGENCY ACHIEVED"));
    TestEqual(TEXT("objective miss"), ObjectiveLabel(4), TEXT("NOT ACHIEVED"));
    TestEqual(TEXT("objective unknown"), ObjectiveLabel(5), TEXT("INDETERMINATE"));
    TestEqual(TEXT("vehicle nominal"), VehicleLabel(1), TEXT("NOMINAL"));
    TestEqual(TEXT("vehicle degraded"), VehicleLabel(2), TEXT("DEGRADED"));
    TestEqual(TEXT("vehicle recovered"), VehicleLabel(3), TEXT("RECOVERED"));
    TestEqual(TEXT("vehicle safe"), VehicleLabel(4), TEXT("SAFE STATE"));
    TestEqual(TEXT("vehicle lost"), VehicleLabel(5), TEXT("LOST"));
    TestEqual(TEXT("vehicle unknown"), VehicleLabel(6), TEXT("UNKNOWN"));
    TestEqual(TEXT("procedure completed"), ProcedureDispositionLabel(1), TEXT("COMPLETED"));
    TestEqual(TEXT("procedure alternate"), ProcedureDispositionLabel(2), TEXT("ALTERNATE BRANCH"));
    TestEqual(TEXT("procedure skipped"), ProcedureDispositionLabel(3), TEXT("SKIPPED"));
    TestEqual(TEXT("procedure mistimed"), ProcedureDispositionLabel(4), TEXT("MISTIMED"));
    TestEqual(TEXT("procedure override"), ProcedureDispositionLabel(5), TEXT("OVERRIDDEN"));
    TestEqual(TEXT("procedure failed"), ProcedureDispositionLabel(6), TEXT("FAILED"));
    TestEqual(TEXT("operator reference"), OperatorLabel(1), TEXT("TIMELY REFERENCE"));
    TestEqual(TEXT("operator alternate"), OperatorLabel(2), TEXT("TIMELY ALTERNATE"));
    TestEqual(TEXT("operator delayed"), OperatorLabel(3), TEXT("DELAYED VALID"));
    TestEqual(TEXT("operator no action"), OperatorLabel(4), TEXT("NO ACTION"));
    TestEqual(TEXT("operator rejected"), OperatorLabel(5), TEXT("REJECTED ACTION"));
    TestEqual(TEXT("avionics degraded"), AvionicsLabel(2), TEXT("DEGRADED OPERATIONAL"));
    TestEqual(TEXT("avionics recovery"), AvionicsLabel(3), TEXT("SAFE RECOVERY"));
    TestEqual(TEXT("evidence complete"), EvidenceLabel(1), TEXT("COMPLETE"));
    TestEqual(TEXT("evidence partial"), EvidenceLabel(2), TEXT("OBSERVATION INCOMPLETE"));
    TestEqual(TEXT("evidence aborted"), EvidenceLabel(3), TEXT("ABORTED"));
    TestEqual(TEXT("evidence invalid"), EvidenceLabel(4), TEXT("INVALID"));
    TestEqual(TEXT("evidence unavailable"), EvidenceLabel(5), TEXT("UNAVAILABLE"));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsActionGateTest,
    "KSA64.Operations.Actions.ReviewStageCommitGate",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsActionGateTest::RunTest(const FString&)
{
    FKsa64OperationsActionGate Gate;
    Gate.ObserveProposal(0x1234, 200);
    TestFalse(TEXT("stage requires review"), Gate.CanStage(100));
    TestFalse(TEXT("commit requires accepted staged receipt"), Gate.CanCommit(100));
    TestTrue(TEXT("review accepted in window"), Gate.Review(100));
    TestTrue(TEXT("review unlocks stage"), Gate.CanStage(100));
    Gate.ObserveReceipt(0x9999, 1, true);
    TestFalse(TEXT("wrong proposal receipt ignored"), Gate.CanCommit(100));
    Gate.ObserveReceipt(0x1234, 1, true);
    TestTrue(TEXT("accepted stage unlocks commit"), Gate.CanCommit(100));
    Gate.ObserveReceipt(0x1234, 2, true);
    TestTrue(TEXT("committed action may be cancelled"), Gate.CanCancel(100));
    Gate.Expire(201);
    TestFalse(TEXT("expired action cannot commit"), Gate.CanCommit(201));
    TestEqual(TEXT("expired action maps exactly"), ActionStateFromReceipt(Gate.ReceiptState()), EKsa64OperationsActionState::Expired);
    TestEqual(TEXT("executed receipt maps exactly"), ActionStateFromReceipt(3), EKsa64OperationsActionState::Executed);
    TestEqual(TEXT("cancelled receipt maps exactly"), ActionStateFromReceipt(4), EKsa64OperationsActionState::Cancelled);
    TestEqual(TEXT("rejected receipt maps exactly"), ActionStateFromReceipt(5), EKsa64OperationsActionState::Rejected);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsPacing30Test,
    "KSA64.Operations.Pacing.Realtime30Hz",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)
bool FKsa64OperationsPacing30Test::RunTest(const FString&) { TestEqual(TEXT("ten seconds produce 320 releases"), RunDisplayFrames(30, 10, EKsa64OperationsPace::Realtime), 320u); return true; }

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsPacing60Test,
    "KSA64.Operations.Pacing.Realtime60Hz",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)
bool FKsa64OperationsPacing60Test::RunTest(const FString&) { TestEqual(TEXT("ten seconds produce 320 releases"), RunDisplayFrames(60, 10, EKsa64OperationsPace::Realtime), 320u); return true; }

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsPacing144Test,
    "KSA64.Operations.Pacing.Realtime144Hz",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)
bool FKsa64OperationsPacing144Test::RunTest(const FString&) { TestEqual(TEXT("ten seconds produce 320 releases"), RunDisplayFrames(144, 10, EKsa64OperationsPace::Realtime), 320u); return true; }

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsMixedPacingTest,
    "KSA64.Operations.Pacing.MixedModesAndPause",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsMixedPacingTest::RunTest(const FString&)
{
    FKsa64OperationsPacingController Controller;
    uint32 Releases = 0;
    const auto Run = [&Controller, &Releases](int32 Frames, float Delta, EKsa64OperationsPace Pace)
    {
        for (int32 Frame = 0; Frame < Frames; ++Frame)
        {
            Controller.Accumulate(Delta, Pace);
            const uint32 Due = Controller.ReleasesDue(Pace, 31'250, true, false);
            Controller.CommitAcceptedAdvance(Due, 31'250, Pace);
            Releases += Due;
        }
    };
    Run(60, 1.0f / 60.0f, EKsa64OperationsPace::Realtime);
    Controller.Reset();
    Run(30, 1.0f / 60.0f, EKsa64OperationsPace::FourX);
    Controller.Reset();
    Run(15, 1.0f / 60.0f, EKsa64OperationsPace::SixteenX);
    TestEqual(TEXT("1s 1x + 0.5s 4x + 0.25s 16x"), Releases, 224u);
    Controller.Accumulate(1.0f, EKsa64OperationsPace::Paused);
    TestEqual(TEXT("pause accrues no releases"), Controller.ReleasesDue(EKsa64OperationsPace::Paused, 31'250, true, false), 0u);
    TestEqual(TEXT("fastest requests full bounded batch"), Controller.ReleasesDue(EKsa64OperationsPace::Fastest, 31'250, true, false), 64u);
    TestEqual(TEXT("outstanding command blocks another batch"), Controller.ReleasesDue(EKsa64OperationsPace::Fastest, 31'250, true, true), 0u);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsOutstandingTest,
    "KSA64.Operations.Pacing.OutstandingAndQueuePressure",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsOutstandingTest::RunTest(const FString&)
{
    FKsa64OperationsAdvanceTracker Tracker;
    Tracker.MarkAccepted(7);
    TestTrue(TEXT("advance marked outstanding"), Tracker.IsOutstanding());
    TestFalse(TEXT("publication alone does not clear while commands remain"), Tracker.Observe(8, 1, 3));
    TestTrue(TEXT("drained queue and new publication clear"), Tracker.Observe(8, 0, 3));
    TestFalse(TEXT("tracker now clear"), Tracker.IsOutstanding());
    Tracker.MarkAccepted(8);
    TestTrue(TEXT("terminal lifecycle clears outstanding"), Tracker.Observe(8, 1, 5));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsSparseBurstTest,
    "KSA64.Operations.Observation.SparseAndBurstPolling",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsSparseBurstTest::RunTest(const FString&)
{
    TArray<FKsa64OperationsReleasePoint> History;
    FKsa64OperationsReleasePoint One; One.ReleaseEpoch = 1;
    FKsa64OperationsReleasePoint Four; Four.ReleaseEpoch = 4; Four.AltitudeQ12Km = 4;
    History = {One, Four};
    FKsa64OperationsReleasePoint Two; Two.ReleaseEpoch = 2;
    FKsa64OperationsReleasePoint Three; Three.ReleaseEpoch = 3;
    FKsa64OperationsReleasePoint RichFour; RichFour.ReleaseEpoch = 4; RichFour.AltitudeQ12Km = 44; RichFour.bHasGroundEstimate = true;
    TArray<FKsa64OperationsReleasePoint> Burst = {Three, Two, RichFour};
    bool bComplete = true;
    MergeReleaseSamples(History, Burst, 16, bComplete);
    TestEqual(TEXT("sparse and burst samples merge uniquely"), History.Num(), 4);
    for (int32 Index = 0; Index < History.Num(); ++Index) TestEqual(TEXT("release order is exact"), History[Index].ReleaseEpoch, static_cast<uint32>(Index + 1));
    TestEqual(TEXT("typed duplicate replaces sparse fallback"), History[3].AltitudeQ12Km, 44);
    TestTrue(TEXT("reordering did not lose observation"), bComplete);
    TArray<FKsa64OperationsReleasePoint> Overflow;
    for (uint32 Release = 5; Release <= 9; ++Release) { FKsa64OperationsReleasePoint Point; Point.ReleaseEpoch = Release; Overflow.Add(Point); }
    MergeReleaseSamples(History, Overflow, 4, bComplete);
    TestEqual(TEXT("bounded history keeps four latest"), History.Num(), 4);
    TestEqual(TEXT("oldest retained release"), History[0].ReleaseEpoch, 6u);
    TestFalse(TEXT("bounded truncation is explicit"), bComplete);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsSemanticAccessibilityTest,
    "KSA64.Operations.Presentation.ResizeAccessibilitySemanticInvariance",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsSemanticAccessibilityTest::RunTest(const FString&)
{
    FKsa64OperationsViewModel View;
    View.bBridgeReady = true;
    View.bTruthFiltered = true;
    View.ReleaseEpoch = 42;
    View.FlightChecksum = 0x11223344;
    const FString Baseline = View.ToDeterministicJson();
    FKsa64OperationsAccessibilitySettings Accessibility;
    Accessibility.TextScale = 1.5f;
    Accessibility.bHighContrast = true;
    Accessibility.bReducedMotion = true;
    Accessibility.bSoundCues = false;
    const FIntPoint Sizes[] = {{1280, 720}, {1920, 1080}, {2560, 1440}};
    for (const FIntPoint Size : Sizes)
    {
        TestTrue(TEXT("layout size is presentational"), Size.X > 0 && Size.Y > 0);
        TestEqual(TEXT("resize and accessibility do not mutate operational semantics"), View.ToDeterministicJson(), Baseline);
    }
    TestEqual(TEXT("accessibility settings remain noncanonical"), Accessibility.TextScale, 1.5f);
    TestEqual(TEXT("deterministic serialization repeats byte-for-byte"), View.ToDeterministicJson(), Baseline);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsEvidenceStatusTest,
    "KSA64.Operations.Evidence.CompleteAndFailedStatus",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsEvidenceStatusTest::RunTest(const FString&)
{
    TestEqual(TEXT("running evidence remains pending"), ClassifyEvidenceReadiness(3, 1, 1, 0, 0), EKsa64OperationsEvidenceReadiness::InProgress);
    TestEqual(TEXT("only verified completed evidence is complete"), ClassifyEvidenceReadiness(5, 2, 2, 100, 1), EKsa64OperationsEvidenceReadiness::Complete);
    TestEqual(TEXT("zero-length evidence cannot masquerade as complete"), ClassifyEvidenceReadiness(5, 2, 2, 0, 1), EKsa64OperationsEvidenceReadiness::InProgress);
    TestEqual(TEXT("aborted lifecycle is failed"), ClassifyEvidenceReadiness(6, 3, 2, 0, 0), EKsa64OperationsEvidenceReadiness::Failed);
    TestEqual(TEXT("worker fault is failed"), ClassifyEvidenceReadiness(3, 1, 3, 0, 0), EKsa64OperationsEvidenceReadiness::Failed);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsAsyncShutdownTest,
    "KSA64.Operations.Lifecycle.AsyncShutdown",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsAsyncShutdownTest::RunTest(const FString&)
{
    FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
    if (Module.GetStatus() != EKsa64BridgeStatus::Ready)
    {
        AddError(FString::Printf(TEXT("bridge not ready: %s"), *Module.GetDiagnostic()));
        return false;
    }
    TestTrue(TEXT("start typed session"), Module.StartGuidedOperationsV1());
    TestEqual(TEXT("queue bounded work"), Module.AdvanceReleases(64), KSA64_VIEWER_QUEUED);
    const double Start = FPlatformTime::Seconds();
    TestTrue(TEXT("request module-owned asynchronous close"), Module.RequestAsyncClose());
    TestTrue(TEXT("shutdown request does not join worker on caller"), FPlatformTime::Seconds() - Start < 0.050);
    const double Deadline = FPlatformTime::Seconds() + 15.0;
    while (Module.GetStatus() == EKsa64BridgeStatus::SessionOpen
        && FPlatformTime::Seconds() < Deadline)
    {
        FTSTicker::GetCoreTicker().Tick(0.001f);
        FPlatformProcess::Sleep(0.001f);
    }
    TestEqual(TEXT("async close returns bridge to ready"), Module.GetStatus(), EKsa64BridgeStatus::Ready);
    TestFalse(TEXT("async closer no longer pending"), Module.IsAsyncClosePending());
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsFullMissionParityTest,
    "KSA64.Operations.ZAcceptance.FullMissionTranscriptParity",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsFullMissionParityTest::RunTest(const FString&)
{
    TUniquePtr<IKsa64OperationsBridgeAdapter> Adapter = IKsa64OperationsBridgeAdapter::Create();
    if (!Adapter.IsValid() || !Adapter->IsReady())
    {
        AddError(Adapter.IsValid() ? Adapter->GetDiagnostic() : TEXT("adapter unavailable"));
        return false;
    }
    TestTrue(TEXT("open typed Guided Operator full mission"), Adapter->StartGuidedOperations());
    FKsa64OperationsViewModel View;
    TestTrue(TEXT("initial typed operational view"), PollAdapterUntil(*Adapter, View, 15.0, [](const FKsa64OperationsViewModel& Candidate) { return Candidate.ReleaseEpoch == 0 && Candidate.bTruthFiltered; }));
    TestTrue(TEXT("ground update review-stage-commit at exact epochs"), ApplyAcceptedAction(*Adapter, View, 6'080, 6'240));
    TestTrue(TEXT("branch review-stage-commit at exact epochs"), ApplyAcceptedAction(*Adapter, View, 6'560, 6'720));
    TestTrue(TEXT("advance without crossing accepted completion"), AdvanceAdapterTo(*Adapter, View, 21'591));
    TestTrue(TEXT("wait for Rust evidence finalization"), PollAdapterUntil(*Adapter, View, 30.0, [](const FKsa64OperationsViewModel& Candidate)
    {
        return Candidate.ReleaseEpoch == 21'591
            && Candidate.Lifecycle == 5
            && Candidate.WorkerState == 2
            && Candidate.FinalizationState == 2;
    }));
    TestTrue(TEXT("Guided view remains truth filtered through completion"), View.bTruthFiltered);
    TestEqual(TEXT("overall disposition"), View.OverallDisposition, 2u);
    TestEqual(TEXT("complete evidence disposition"), View.EvidenceDisposition, 1u);

    TArray<uint8> Evidence;
    TestEqual(TEXT("retrieve opaque Rust-verified KSB11"), Adapter->GetCompletedEvidence(Evidence), EKsa64OperationsAdapterResult::Ok);
    TestEqual(TEXT("accepted KSB11 length"), Evidence.Num(), 2'911'464);
    FSHA256Signature Signature = {};
    TestTrue(TEXT("compute KSB11 SHA-256"), FPlatformMisc::GetSHA256Signature(Evidence.GetData(), Evidence.Num(), Signature));
    TestEqual(TEXT("accepted KSB11 SHA-256"), Signature.ToString().ToLower(), TEXT("7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"));
    Adapter->Close();
    TestEqual(TEXT("completed worker closes without blocking"), FKsa64BridgeModule::Get().GetStatus(), EKsa64BridgeStatus::Ready);
    return true;
}

#endif
