#include "Ksa64OperationsBridgeAdapter.h"
#include "Ksa64LiveMissionSubsystem.h"
#include "Ksa64OperationsDashboard.h"
#include "Ksa64OperationsPolicy.h"

#if WITH_DEV_AUTOMATION_TESTS

#include "Ksa64BridgeModule.h"
#include "Containers/Ticker.h"
#include "HAL/PlatformMisc.h"
#include "HAL/PlatformProcess.h"
#include "HAL/PlatformTime.h"
#include "Misc/AutomationTest.h"
#include "Engine/GameInstance.h"
#include "Input/Events.h"
#include "InputCoreTypes.h"
#include "Layout/ArrangedChildren.h"
#include "Layout/Children.h"
#include "UObject/StrongObjectPtr.h"
#include "Widgets/Input/SButton.h"

namespace
{
using namespace Ksa64OperationsPolicy;

FString AdapterFailure;

void InspectDashboardAccessibility(
    const TSharedRef<SWidget>& Widget,
    int32& OutButtonCount,
    bool& bOutAllButtonsCustom,
    bool& bOutAllButtonsNamed)
{
    if (Widget->GetType() == FName(TEXT("SButton")))
    {
        ++OutButtonCount;
        bOutAllButtonsCustom &=
            Widget->GetAccessibleBehavior() == EAccessibleBehavior::Custom;
        bOutAllButtonsNamed &= !Widget->GetAccessibleText().IsEmpty();
    }
    FChildren* Children = Widget->GetChildren();
    if (Children == nullptr)
    {
        return;
    }
    for (int32 Index = 0; Index < Children->Num(); ++Index)
    {
        InspectDashboardAccessibility(
            Children->GetChildAt(Index),
            OutButtonCount,
            bOutAllButtonsCustom,
            bOutAllButtonsNamed);
    }
}

TSharedPtr<SButton> FindDashboardButton(
    const TSharedRef<SWidget>& Widget,
    const FString& AccessibleName)
{
    if (Widget->GetType() == FName(TEXT("SButton"))
        && Widget->GetAccessibleText().ToString() == AccessibleName)
    {
        return StaticCastSharedRef<SButton>(Widget);
    }
    FChildren* Children = Widget->GetChildren();
    if (Children == nullptr)
    {
        return nullptr;
    }
    for (int32 Index = 0; Index < Children->Num(); ++Index)
    {
        TSharedPtr<SButton> Found = FindDashboardButton(
            Children->GetChildAt(Index),
            AccessibleName);
        if (Found.IsValid())
        {
            return Found;
        }
    }
    return nullptr;
}

FString AdapterState(const FKsa64OperationsViewModel& View)
{
    return FString::Printf(
        TEXT("release=%u publication=%llu pending=%u command_result=%d worker=%u finalization=%u proposal=%08X receipt=%u overflow=%u"),
        View.ReleaseEpoch,
        static_cast<unsigned long long>(View.CommandSequence),
        View.CommandsPending,
        View.CommandResult,
        View.WorkerState,
        View.FinalizationState,
        View.ActionProposalIdentity,
        View.ActionReceiptState,
        View.TransportOverflow);
}

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

void CloseAdapterAndWait(IKsa64OperationsBridgeAdapter& Adapter)
{
    Adapter.Close();
    const double Deadline = FPlatformTime::Seconds() + 15.0;
    while (FKsa64BridgeModule::Get().GetStatus() == EKsa64BridgeStatus::SessionOpen
        && FPlatformTime::Seconds() < Deadline)
    {
        FTSTicker::GetCoreTicker().Tick(0.001f);
        FPlatformProcess::Sleep(0.001f);
    }
    if (FKsa64BridgeModule::Get().GetStatus() == EKsa64BridgeStatus::SessionOpen)
    {
        // A shutdown request that remains open beyond the bounded close gate is
        // already a lifecycle failure, not a slow valid mission. Release the
        // test-owned handle so it cannot corrupt the following automation case.
        AdapterFailure = TEXT("adapter asynchronous close did not complete");
        FKsa64BridgeModule::Get().CloseSession();
    }
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
    AdapterFailure = FString::Printf(TEXT("poll timed out: %s"), *AdapterState(View));
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
            AdapterFailure = FString::Printf(
                TEXT("advance enqueue failed result=%u target=%u: %s"),
                static_cast<uint32>(Result),
                TargetRelease,
                *AdapterState(View));
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
            AdapterFailure = FString::Printf(
                TEXT("advance expected release=%u from publication=%llu: %s"),
                Expected,
                static_cast<unsigned long long>(PriorPublication),
                *AdapterState(View));
            return false;
        }
        TArray<FKsa64OperationsTimelineItem> DiscardedTimeline;
        TArray<FKsa64OperationsReleasePoint> DiscardedSamples;
        Adapter.DrainTimeline(DiscardedTimeline);
        Adapter.DrainReleaseSamples(DiscardedSamples);
        if (View.TransportOverflow != 0 || !View.bObservationComplete)
        {
            AdapterFailure = FString::Printf(TEXT("presentation stream incomplete: %s"), *AdapterState(View));
            return false;
        }
    }
    if (View.ReleaseEpoch != TargetRelease)
    {
        AdapterFailure = FString::Printf(TEXT("advance missed target=%u: %s"), TargetRelease, *AdapterState(View));
        return false;
    }
    return true;
}

bool ApplyAcceptedAction(
    IKsa64OperationsBridgeAdapter& Adapter,
    FKsa64OperationsViewModel& View,
    uint32 StageEpoch,
    uint32 CommitEpoch)
{
    AdapterFailure.Reset();
    if (!AdvanceAdapterTo(Adapter, View, StageEpoch))
    {
        return false;
    }
    if (View.ActionProposalIdentity == 0)
    {
        AdapterFailure = FString::Printf(TEXT("no proposal at stage release=%u: %s"), StageEpoch, *AdapterState(View));
        return false;
    }
    if (Adapter.ReviewAction() != EKsa64OperationsAdapterResult::Ok)
    {
        AdapterFailure = FString::Printf(TEXT("review rejected at release=%u: %s"), StageEpoch, *AdapterState(View));
        return false;
    }
    if (Adapter.StageAction() != EKsa64OperationsAdapterResult::Queued)
    {
        AdapterFailure = FString::Printf(TEXT("stage did not queue at release=%u: %s"), StageEpoch, *AdapterState(View));
        return false;
    }
    if (!PollAdapterUntil(
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
        AdapterFailure = FString::Printf(TEXT("stage receipt missing at release=%u: %s"), StageEpoch, *AdapterState(View));
        return false;
    }
    const uint32 Proposal = View.ActionProposalIdentity;
    if (!AdvanceAdapterTo(Adapter, View, CommitEpoch))
    {
        return false;
    }
    if (View.ActionProposalIdentity != Proposal)
    {
        AdapterFailure = FString::Printf(TEXT("proposal changed before commit=%u: %s"), CommitEpoch, *AdapterState(View));
        return false;
    }
    if (Adapter.CommitAction() != EKsa64OperationsAdapterResult::Queued)
    {
        AdapterFailure = FString::Printf(TEXT("commit did not queue at release=%u: %s"), CommitEpoch, *AdapterState(View));
        return false;
    }
    if (!PollAdapterUntil(
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
        AdapterFailure = FString::Printf(TEXT("commit receipt missing at release=%u: %s"), CommitEpoch, *AdapterState(View));
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
    FKsa64OperationsRealAdapterPacingTest,
    "KSA64.Operations.Pacing.RealAdapterRefreshInvariance",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsRealAdapterPacingTest::RunTest(const FString&)
{
    struct FOutcome
    {
        uint32 ReleaseEpoch = 0;
        uint32 FlightChecksum = 0;
        uint32 NavigationChecksum = 0;
        uint32 CommandChecksum = 0;
    };

    const auto RunAtRefresh = [this](int32 RefreshHz, FOutcome& OutOutcome)
    {
        TUniquePtr<IKsa64OperationsBridgeAdapter> Adapter =
            IKsa64OperationsBridgeAdapter::Create();
        if (!Adapter.IsValid() || !Adapter->IsReady()
            || !Adapter->StartGuidedOperations())
        {
            AdapterFailure = FString::Printf(
                TEXT("%d Hz adapter start failed: %s"),
                RefreshHz,
                Adapter.IsValid() ? *Adapter->GetDiagnostic() : TEXT("adapter unavailable"));
            return false;
        }

        FKsa64OperationsViewModel View;
        if (!PollAdapterUntil(
                *Adapter,
                View,
                15.0,
                [](const FKsa64OperationsViewModel& Candidate)
                {
                    return Candidate.ReleaseEpoch == 0
                        && Candidate.CommandsPending == 0
                        && Candidate.bTruthFiltered;
                }))
        {
            AdapterFailure = FString::Printf(
                TEXT("%d Hz initial view failed: %s"),
                RefreshHz,
                *AdapterFailure);
            CloseAdapterAndWait(*Adapter);
            return false;
        }

        FKsa64OperationsPacingController Controller;
        for (int32 Frame = 0; Frame < RefreshHz * 10; ++Frame)
        {
            const int64 StartNanoseconds =
                static_cast<int64>(Frame) * 1'000'000'000 / RefreshHz;
            const int64 EndNanoseconds =
                static_cast<int64>(Frame + 1) * 1'000'000'000 / RefreshHz;
            Controller.AccumulateNanoseconds(
                EndNanoseconds - StartNanoseconds,
                EKsa64OperationsPace::Realtime);
            const uint32 Due = Controller.ReleasesDue(
                EKsa64OperationsPace::Realtime,
                31'250,
                true,
                false);
            if (Due == 0)
            {
                continue;
            }

            const uint32 ExpectedRelease = View.ReleaseEpoch + Due;
            const uint64 PriorPublication = View.CommandSequence;
            const EKsa64OperationsAdapterResult AdvanceResult =
                Adapter->AdvanceReleases(Due);
            if (AdvanceResult != EKsa64OperationsAdapterResult::Ok
                && AdvanceResult != EKsa64OperationsAdapterResult::Queued)
            {
                AdapterFailure = FString::Printf(
                    TEXT("%d Hz advance failed at frame %d with result %u: %s"),
                    RefreshHz,
                    Frame,
                    static_cast<uint32>(AdvanceResult),
                    *AdapterState(View));
                CloseAdapterAndWait(*Adapter);
                return false;
            }
            if (!PollAdapterUntil(
                    *Adapter,
                    View,
                    15.0,
                    [ExpectedRelease, PriorPublication](
                        const FKsa64OperationsViewModel& Candidate)
                    {
                        return Candidate.ReleaseEpoch == ExpectedRelease
                            && Candidate.CommandSequence != PriorPublication
                            && Candidate.CommandsPending == 0;
                    }))
            {
                AdapterFailure = FString::Printf(
                    TEXT("%d Hz advance observation failed at frame %d: %s"),
                    RefreshHz,
                    Frame,
                    *AdapterFailure);
                CloseAdapterAndWait(*Adapter);
                return false;
            }
            TArray<FKsa64OperationsTimelineItem> DiscardedTimeline;
            TArray<FKsa64OperationsReleasePoint> DiscardedSamples;
            Adapter->DrainTimeline(DiscardedTimeline);
            Adapter->DrainReleaseSamples(DiscardedSamples);
            if (View.TransportOverflow != 0 || !View.bObservationComplete)
            {
                AdapterFailure = FString::Printf(
                    TEXT("%d Hz presentation stream incomplete: %s"),
                    RefreshHz,
                    *AdapterState(View));
                CloseAdapterAndWait(*Adapter);
                return false;
            }
            Controller.CommitAcceptedAdvance(
                Due,
                31'250,
                EKsa64OperationsPace::Realtime);
        }

        OutOutcome.ReleaseEpoch = View.ReleaseEpoch;
        OutOutcome.FlightChecksum = View.FlightChecksum;
        OutOutcome.NavigationChecksum = View.NavigationChecksum;
        OutOutcome.CommandChecksum = View.CommandChecksum;
        const EKsa64OperationsAdapterResult ShutdownResult =
            Adapter->RequestShutdown();
        if (ShutdownResult != EKsa64OperationsAdapterResult::Ok
            && ShutdownResult != EKsa64OperationsAdapterResult::Queued)
        {
            AdapterFailure = FString::Printf(
                TEXT("%d Hz shutdown request failed with result %u"),
                RefreshHz,
                static_cast<uint32>(ShutdownResult));
            CloseAdapterAndWait(*Adapter);
            return false;
        }
        if (!PollAdapterUntil(
                *Adapter,
                View,
                15.0,
                [](const FKsa64OperationsViewModel& Candidate)
                {
                    return Candidate.WorkerState == 2
                        || Candidate.WorkerState == 3;
                }))
        {
            AdapterFailure = FString::Printf(
                TEXT("%d Hz worker shutdown failed: %s"),
                RefreshHz,
                *AdapterFailure);
            CloseAdapterAndWait(*Adapter);
            return false;
        }
        if (View.WorkerState != 2 || View.FinalizationState != 1)
        {
            AdapterFailure = FString::Printf(
                TEXT("%d Hz clean partial shutdown reported worker=%u finalization=%u"),
                RefreshHz,
                View.WorkerState,
                View.FinalizationState);
            CloseAdapterAndWait(*Adapter);
            return false;
        }
        CloseAdapterAndWait(*Adapter);
        if (FKsa64BridgeModule::Get().GetStatus() != EKsa64BridgeStatus::Ready)
        {
            AdapterFailure = FString::Printf(
                TEXT("%d Hz bridge did not return to ready"),
                RefreshHz);
            return false;
        }
        return true;
    };

    TArray<FOutcome> Outcomes;
    for (const int32 RefreshHz : {30, 60, 144})
    {
        FOutcome Outcome;
        if (!RunAtRefresh(RefreshHz, Outcome))
        {
            AddError(AdapterFailure);
            return false;
        }
        TestEqual(
            FString::Printf(TEXT("%d Hz reaches 320 exact releases"), RefreshHz),
            Outcome.ReleaseEpoch,
            320u);
        TestNotEqual(
            FString::Printf(TEXT("%d Hz flight checksum is populated"), RefreshHz),
            Outcome.FlightChecksum,
            0u);
        Outcomes.Add(Outcome);
    }

    for (int32 Index = 1; Index < Outcomes.Num(); ++Index)
    {
        TestEqual(TEXT("flight checksum is refresh invariant"), Outcomes[Index].FlightChecksum, Outcomes[0].FlightChecksum);
        TestEqual(TEXT("navigation checksum is refresh invariant"), Outcomes[Index].NavigationChecksum, Outcomes[0].NavigationChecksum);
        TestEqual(TEXT("command checksum is refresh invariant"), Outcomes[Index].CommandChecksum, Outcomes[0].CommandChecksum);
    }
    return true;
}

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
    Tracker.MarkAccepted(7, 6'240);
    TestTrue(TEXT("advance marked outstanding"), Tracker.IsOutstanding());
    TestFalse(TEXT("delayed publication without release progress does not clear"), Tracker.Observe(8, 6'240, 0, 3));
    TestFalse(TEXT("release progress does not clear while commands remain"), Tracker.Observe(9, 6'304, 1, 3));
    TestTrue(TEXT("new publication with drained queue and release progress clears"), Tracker.Observe(9, 6'304, 0, 3));
    TestFalse(TEXT("tracker now clear"), Tracker.IsOutstanding());
    Tracker.MarkAccepted(9, 6'304);
    TestTrue(TEXT("terminal lifecycle clears outstanding"), Tracker.Observe(9, 6'304, 1, 5));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsTimingPercentileTest,
    "KSA64.Operations.Performance.NearestRankP99",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsTimingPercentileTest::RunTest(const FString&)
{
    TArray<int64> Samples;
    for (int64 Value = 600; Value >= 1; --Value)
    {
        Samples.Add(Value);
    }
    TestEqual(TEXT("600-sample nearest-rank p99"), NearestRankP99Nanoseconds(Samples), 594ll);
    TestEqual(TEXT("single-sample p99"), NearestRankP99Nanoseconds({42}), 42ll);
    TestEqual(TEXT("empty p99 is invalid"), NearestRankP99Nanoseconds({}), -1ll);
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

    TStrongObjectPtr<UGameInstance> GameInstance(
        NewObject<UGameInstance>(GetTransientPackage()));
    TStrongObjectPtr<UKsa64LiveMissionSubsystem> Subsystem(
        NewObject<UKsa64LiveMissionSubsystem>(GameInstance.Get()));
    const TSharedRef<SKsa64OperationsDashboard> Dashboard =
        SNew(SKsa64OperationsDashboard).Subsystem(Subsystem.Get());
    TestTrue(TEXT("actual dashboard supports keyboard focus"), Dashboard->SupportsKeyboardFocus());

    const FGeometry RootGeometry = FGeometry::MakeRoot(
        FVector2f(1920.0f, 1080.0f),
        FSlateLayoutTransform());
    const FModifierKeysState NoModifiers;
    const auto SendKey = [&Dashboard, &RootGeometry, &NoModifiers](const FKey& Key)
    {
        return Dashboard->OnKeyDown(
            RootGeometry,
            FKeyEvent(Key, NoModifiers, 0, false, 0, 0));
    };
    TestTrue(TEXT("realtime keyboard action handled"), SendKey(EKeys::One).IsEventHandled());
    TestEqual(
        TEXT("realtime keyboard action changes only presentation pace"),
        Subsystem->GetViewModel().PresentationPace,
        EKsa64OperationsPace::Realtime);
    TestTrue(TEXT("four-times keyboard action handled"), SendKey(EKeys::Four).IsEventHandled());
    TestEqual(
        TEXT("four-times keyboard action changes only presentation pace"),
        Subsystem->GetViewModel().PresentationPace,
        EKsa64OperationsPace::FourX);
    TestTrue(TEXT("maximum keyboard action handled"), SendKey(EKeys::Zero).IsEventHandled());
    TestEqual(
        TEXT("maximum keyboard action changes only presentation pace"),
        Subsystem->GetViewModel().PresentationPace,
        EKsa64OperationsPace::Fastest);
    TestEqual(
        TEXT("smooth display is the presentation default"),
        Subsystem->GetDisplayMode(),
        EKsa64OperationsDisplayMode::Smooth);
    TestTrue(TEXT("exact-view keyboard action handled"), SendKey(EKeys::E).IsEventHandled());
    TestEqual(
        TEXT("exact-view keyboard action changes only display mode"),
        Subsystem->GetDisplayMode(),
        EKsa64OperationsDisplayMode::Exact);
    TestTrue(TEXT("smooth-view keyboard action handled"), SendKey(EKeys::E).IsEventHandled());
    TestEqual(
        TEXT("second exact-view action restores smooth display"),
        Subsystem->GetDisplayMode(),
        EKsa64OperationsDisplayMode::Smooth);
    TestTrue(TEXT("engineering drawer keyboard action handled"), SendKey(EKeys::D).IsEventHandled());
    TestTrue(TEXT("pause keyboard action handled"), SendKey(EKeys::SpaceBar).IsEventHandled());
    TestEqual(
        TEXT("pause keyboard action returns to paused presentation"),
        Subsystem->GetViewModel().PresentationPace,
        EKsa64OperationsPace::Paused);

    int32 ButtonCount = 0;
    bool bAllButtonsCustom = true;
    bool bAllButtonsNamed = true;
    InspectDashboardAccessibility(
        Dashboard,
        ButtonCount,
        bAllButtonsCustom,
        bAllButtonsNamed);
    TestEqual(TEXT("actual dashboard exposes every command button"), ButtonCount, 16);
    TestTrue(TEXT("every command button uses custom accessible text"), bAllButtonsCustom);
    TestTrue(TEXT("every command button accessible name is nonempty"), bAllButtonsNamed);

    const FString SubsystemBaseline = Subsystem->GetViewModel().ToDeterministicJson();
    Subsystem->ToggleHighContrast();
    Subsystem->ToggleReducedMotion();
    Subsystem->CycleTextScale();
    Subsystem->CycleTextScale();
    TestTrue(TEXT("actual dashboard high contrast is enabled"), Subsystem->GetAccessibility().bHighContrast);
    TestTrue(TEXT("actual dashboard reduced motion is enabled"), Subsystem->GetAccessibility().bReducedMotion);
    TestEqual(TEXT("actual dashboard text scale reaches 150 percent"), Subsystem->GetAccessibility().TextScale, 1.5f);

    const FIntPoint Sizes[] = {{1280, 720}, {1920, 1080}, {2560, 1440}};
    for (const FIntPoint Size : Sizes)
    {
        Dashboard->SlatePrepass(1.0f);
        const FVector2D Desired = Dashboard->GetDesiredSize();
        TestTrue(TEXT("actual dashboard desired width remains positive"), Desired.X > 0.0f);
        TestTrue(TEXT("actual dashboard desired height remains positive"), Desired.Y > 0.0f);
        FArrangedChildren Arranged(EVisibility::Visible);
        Dashboard->ArrangeChildren(
            FGeometry::MakeRoot(
                FVector2f(static_cast<float>(Size.X), static_cast<float>(Size.Y)),
                FSlateLayoutTransform()),
            Arranged);
        TestTrue(TEXT("actual dashboard arranges visible content"), Arranged.Num() > 0);
        TestEqual(TEXT("resize does not mutate detached operational semantics"), View.ToDeterministicJson(), Baseline);
        TestEqual(
            TEXT("resize/accessibility do not mutate subsystem operational semantics"),
            Subsystem->GetViewModel().ToDeterministicJson(),
            SubsystemBaseline);
    }
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
    TestEqual(TEXT("completed snapshot waits for asynchronous finalization"), ClassifyEvidenceReadiness(5, 1, 1, 0, 0), EKsa64OperationsEvidenceReadiness::InProgress);
    TestEqual(TEXT("only verified completed evidence is complete"), ClassifyEvidenceReadiness(5, 2, 2, 100, 1), EKsa64OperationsEvidenceReadiness::Complete);
    TestEqual(TEXT("zero-length evidence cannot masquerade as complete"), ClassifyEvidenceReadiness(5, 2, 2, 0, 1), EKsa64OperationsEvidenceReadiness::InProgress);
    TestEqual(TEXT("aborted lifecycle is failed"), ClassifyEvidenceReadiness(6, 3, 2, 0, 0), EKsa64OperationsEvidenceReadiness::Failed);
    TestEqual(TEXT("worker fault is failed"), ClassifyEvidenceReadiness(3, 1, 3, 0, 0), EKsa64OperationsEvidenceReadiness::Failed);
    TestEqual(
        TEXT("SHA-256 empty vector"),
        Sha256Hex(nullptr, 0),
        TEXT("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
    static constexpr uint8 Abc[] = {'a', 'b', 'c'};
    TestEqual(
        TEXT("SHA-256 abc vector"),
        Sha256Hex(Abc, UE_ARRAY_COUNT(Abc)),
        TEXT("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64OperationsReplayToGuidedLifecycleTest,
    "KSA64.Operations.Lifecycle.NominalReplayToGuided",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsReplayToGuidedLifecycleTest::RunTest(const FString&)
{
    TUniquePtr<IKsa64OperationsBridgeAdapter> Adapter =
        IKsa64OperationsBridgeAdapter::Create();
    if (!Adapter.IsValid() || !Adapter->IsReady())
    {
        AddError(Adapter.IsValid()
            ? Adapter->GetDiagnostic()
            : TEXT("adapter unavailable"));
        return false;
    }
    if (!Adapter->StartNominalGlobalReplay())
    {
        AddError(FString::Printf(
            TEXT("nominal replay start failed: %s"),
            *Adapter->GetDiagnostic()));
        return false;
    }
    Ksa64GlobalDisplayAvailabilityV1 Availability = {};
    EKsa64OperationsAdapterResult AvailabilityResult =
        EKsa64OperationsAdapterResult::NoData;
    const double AvailabilityDeadline = FPlatformTime::Seconds() + 15.0;
    do
    {
        AvailabilityResult = Adapter->GlobalDisplayAvailability(Availability);
        if (AvailabilityResult == EKsa64OperationsAdapterResult::Ok)
        {
            break;
        }
        if (AvailabilityResult != EKsa64OperationsAdapterResult::NoData
            && AvailabilityResult != EKsa64OperationsAdapterResult::Unchanged)
        {
            break;
        }
        FPlatformProcess::Sleep(0.0005f);
    }
    while (FPlatformTime::Seconds() < AvailabilityDeadline);
    TestEqual(
        TEXT("nominal replay publishes GlobalDisplayV1 within the bounded worker-publication gate"),
        AvailabilityResult,
        EKsa64OperationsAdapterResult::Ok);
    if (AvailabilityResult == EKsa64OperationsAdapterResult::Ok)
    {
        TestEqual(TEXT("nominal replay is SIM Director"), Availability.role, 5u);
    }
    TestEqual(
        TEXT("nominal replay closes synchronously through its dedicated path"),
        Adapter->RequestShutdown(),
        EKsa64OperationsAdapterResult::Ok);
    TestEqual(
        TEXT("replay shutdown returns the bridge to ready"),
        FKsa64BridgeModule::Get().GetStatus(),
        EKsa64BridgeStatus::Ready);
    Availability = {};
    TestEqual(
        TEXT("closed replay handle is no longer exposed"),
        Adapter->GlobalDisplayAvailability(Availability),
        EKsa64OperationsAdapterResult::Unsupported);

    if (!Adapter->StartGuidedOperations())
    {
        AddError(FString::Printf(
            TEXT("guided start after replay close failed: %s"),
            *Adapter->GetDiagnostic()));
        Adapter->Close();
        return false;
    }
    FKsa64OperationsViewModel View;
    const bool bGuidedReady = PollAdapterUntil(
        *Adapter,
        View,
        15.0,
        [](const FKsa64OperationsViewModel& Candidate)
        {
            return Candidate.ReleaseEpoch == 0
                && Candidate.bTruthFiltered
                && Candidate.RoleLabel.Equals(TEXT("GUIDED OPERATOR"));
        });
    if (!bGuidedReady)
    {
        AddError(FString::Printf(
            TEXT("guided view after replay close failed: %s"),
            *AdapterFailure));
    }
    CloseAdapterAndWait(*Adapter);
    TestEqual(
        TEXT("guided close preserves the typed asynchronous lifecycle"),
        FKsa64BridgeModule::Get().GetStatus(),
        EKsa64BridgeStatus::Ready);
    return bGuidedReady;
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
    FKsa64OperationsSlateActionTranscriptParityTest,
    "KSA64.Operations.ZAcceptance.SlateActionTranscriptParity",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64OperationsSlateActionTranscriptParityTest::RunTest(const FString&)
{
    TStrongObjectPtr<UGameInstance> GameInstance(
        NewObject<UGameInstance>(GetTransientPackage()));
    TStrongObjectPtr<UKsa64LiveMissionSubsystem> Subsystem(
        NewObject<UKsa64LiveMissionSubsystem>(GameInstance.Get()));
    if (!Subsystem->InitializeForAutomation()
        || !Subsystem->StartGuidedOperations())
    {
        AddError(FString::Printf(
            TEXT("live subsystem could not start: %s"),
            *Subsystem->GetViewModel().LastDiagnostic));
        Subsystem->CloseForAutomation();
        return false;
    }
    const TSharedRef<SKsa64OperationsDashboard> Dashboard =
        SNew(SKsa64OperationsDashboard).Subsystem(Subsystem.Get());

    const auto Click = [this, &Dashboard](const FString& Name)
    {
        const TSharedPtr<SButton> Button = FindDashboardButton(Dashboard, Name);
        if (!Button.IsValid())
        {
            AddError(FString::Printf(TEXT("dashboard button not found: %s"), *Name));
            return false;
        }
        if (!Button->IsEnabled())
        {
            AddError(FString::Printf(TEXT("dashboard button unexpectedly disabled: %s"), *Name));
            return false;
        }
        Button->SimulateClick();
        return true;
    };
    const auto ApplyThroughSlate = [this, &Subsystem, &Click](
        uint32 StageEpoch,
        uint32 CommitEpoch)
    {
        if (!Subsystem->AdvanceToReleaseForAutomation(StageEpoch, 30.0))
        {
            AddError(FString::Printf(TEXT("could not advance UI session to %u"), StageEpoch));
            return false;
        }
        if (!Click(TEXT("1  REVIEW")))
        {
            return false;
        }
        // Poll the reviewed adapter state through the subsystem before Slate
        // evaluates the Stage button's production IsEnabled attribute.
        if (!Subsystem->AdvanceToReleaseForAutomation(StageEpoch, 5.0))
        {
            AddError(FString::Printf(TEXT("review poll missed stage release %u (now %u)"), StageEpoch, Subsystem->GetViewModel().ReleaseEpoch));
            return false;
        }
        if (!Click(TEXT("2  STAGE")))
        {
            AddError(FString::Printf(TEXT("Slate stage button failed at %u"), StageEpoch));
            return false;
        }
        if (!Subsystem->WaitForActionReceiptForAutomation(1, 15.0))
        {
            const FKsa64OperationsViewModel& Failed = Subsystem->GetViewModel();
            AddError(FString::Printf(
                TEXT("Slate stage receipt failed at %u: release=%u proposal=%08X receipt=%llu state=%u accepted=%u pending=%u overflow=%u"),
                StageEpoch,
                Failed.ReleaseEpoch,
                Failed.ActionProposalIdentity,
                static_cast<unsigned long long>(Failed.ActionReceiptSequence),
                Failed.ActionReceiptState,
                Failed.ActionReceiptAccepted,
                Failed.CommandsPending,
                Failed.TransportOverflow));
            return false;
        }
        if (!Subsystem->AdvanceToReleaseForAutomation(CommitEpoch, 30.0))
        {
            AddError(FString::Printf(TEXT("could not advance UI session to commit %u (now %u)"), CommitEpoch, Subsystem->GetViewModel().ReleaseEpoch));
            return false;
        }
        if (!Click(TEXT("3  COMMIT")))
        {
            AddError(FString::Printf(TEXT("Slate commit button failed at %u"), CommitEpoch));
            return false;
        }
        if (!Subsystem->WaitForActionReceiptForAutomation(2, 15.0))
        {
            AddError(FString::Printf(TEXT("Slate commit receipt failed at %u"), CommitEpoch));
            return false;
        }
        return true;
    };

    if (!ApplyThroughSlate(6'080, 6'240)
        || !ApplyThroughSlate(6'560, 6'720)
        || !Subsystem->AdvanceToReleaseForAutomation(21'591, 60.0)
        || !Subsystem->WaitForCompletionForAutomation(30.0))
    {
        AddError(FString::Printf(
            TEXT("Slate transcript did not complete: release=%u diagnostic=%s"),
            Subsystem->GetViewModel().ReleaseEpoch,
            *Subsystem->GetViewModel().LastDiagnostic));
        Subsystem->CloseForAutomation();
        return false;
    }
    TArray<uint8> Evidence;
    TestTrue(
        TEXT("Slate-driven live subsystem exposes completed opaque evidence"),
        Subsystem->CopyCompletedEvidenceForAutomation(Evidence));
    TestEqual(TEXT("Slate transcript KSB11 length"), Evidence.Num(), 2'911'464);
    TestEqual(
        TEXT("Slate transcript KSB11 matches scripted oracle"),
        Sha256Hex(Evidence.GetData(), static_cast<uint64>(Evidence.Num())),
        TEXT("7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"));
    TestEqual(TEXT("Slate transcript contains four actions"), Subsystem->GetViewModel().ActionCount, 4u);
    TestTrue(TEXT("Slate transcript stays truth filtered"), Subsystem->GetViewModel().bTruthFiltered);
    Subsystem->CloseForAutomation();
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
    if (!Adapter->StartGuidedOperations())
    {
        AddError(FString::Printf(TEXT("typed Guided Operator start failed: %s"), *Adapter->GetDiagnostic()));
        return false;
    }
    FKsa64OperationsViewModel View;
    if (!PollAdapterUntil(*Adapter, View, 15.0, [](const FKsa64OperationsViewModel& Candidate) { return Candidate.ReleaseEpoch == 0 && Candidate.bTruthFiltered; }))
    {
        AddError(FString::Printf(TEXT("initial typed operational view failed: %s"), *AdapterFailure));
        CloseAdapterAndWait(*Adapter);
        return false;
    }
    if (!ApplyAcceptedAction(*Adapter, View, 6'080, 6'240))
    {
        AddError(FString::Printf(TEXT("ground update action failed: %s"), *AdapterFailure));
        CloseAdapterAndWait(*Adapter);
        return false;
    }
    if (!ApplyAcceptedAction(*Adapter, View, 6'560, 6'720))
    {
        AddError(FString::Printf(TEXT("branch action failed: %s"), *AdapterFailure));
        CloseAdapterAndWait(*Adapter);
        return false;
    }
    if (!AdvanceAdapterTo(*Adapter, View, 21'591))
    {
        AddError(FString::Printf(TEXT("mission completion advance failed: %s"), *AdapterFailure));
        CloseAdapterAndWait(*Adapter);
        return false;
    }
    if (!PollAdapterUntil(*Adapter, View, 30.0, [](const FKsa64OperationsViewModel& Candidate)
    {
        return Candidate.ReleaseEpoch == 21'591
            && Candidate.Lifecycle == 5
            && Candidate.WorkerState == 2
            && Candidate.FinalizationState == 2;
    }))
    {
        AddError(FString::Printf(TEXT("Rust evidence finalization failed: %s"), *AdapterFailure));
        CloseAdapterAndWait(*Adapter);
        return false;
    }
    TestTrue(TEXT("presentation queues remain complete"), View.bObservationComplete);
    TestEqual(TEXT("presentation queues never overflow"), View.TransportOverflow, 0u);
    TestTrue(TEXT("Guided view remains truth filtered through completion"), View.bTruthFiltered);
    TestEqual(TEXT("overall disposition"), View.OverallDisposition, 2u);
    TestEqual(TEXT("complete evidence disposition"), View.EvidenceDisposition, 1u);

    TArray<uint8> Evidence;
    TestEqual(TEXT("retrieve opaque Rust-verified KSB11"), Adapter->GetCompletedEvidence(Evidence), EKsa64OperationsAdapterResult::Ok);
    TestEqual(TEXT("accepted KSB11 length"), Evidence.Num(), 2'911'464);
    TestEqual(
        TEXT("accepted KSB11 SHA-256"),
        Sha256Hex(Evidence.GetData(), static_cast<uint64>(Evidence.Num())),
        TEXT("7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"));
    CloseAdapterAndWait(*Adapter);
    TestEqual(TEXT("completed worker closes without blocking"), FKsa64BridgeModule::Get().GetStatus(), EKsa64BridgeStatus::Ready);
    return true;
}

#endif
