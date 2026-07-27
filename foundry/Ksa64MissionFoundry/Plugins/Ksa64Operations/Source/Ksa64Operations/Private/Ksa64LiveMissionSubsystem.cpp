#include "Ksa64LiveMissionSubsystem.h"

#include "Ksa64OperationsBridgeAdapter.h"
#include "Ksa64OperationsDashboard.h"

#include "Containers/Ticker.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "HAL/PlatformFileManager.h"
#include "HAL/PlatformMisc.h"
#include "HAL/PlatformProcess.h"
#include "HAL/PlatformTime.h"
#include "Misc/CommandLine.h"
#include "Misc/Crc.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Framework/Application/SlateApplication.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "Widgets/SWidget.h"

DEFINE_LOG_CATEGORY_STATIC(LogKsa64Operations, Log, All);

namespace
{
constexpr int32 MaxReleaseHistory = 32'768;
constexpr int32 MaxTimelineItems = 512;

}

void UKsa64LiveMissionSubsystem::Initialize(FSubsystemCollectionBase& Collection)
{
    Super::Initialize(Collection);
    Bridge = IKsa64OperationsBridgeAdapter::Create();
    ViewModel.bBridgeReady = Bridge.IsValid() && Bridge->IsReady();
    ViewModel.BridgeStatus = ViewModel.bBridgeReady
        ? TEXT("BRIDGE QUALIFIED")
        : TEXT("BRIDGE UNAVAILABLE");
    ViewModel.LastDiagnostic = Bridge.IsValid()
        ? Bridge->GetDiagnostic()
        : TEXT("operations bridge adapter could not be created");
    ViewModel.Capabilities = Bridge.IsValid()
        ? Bridge->GetCapabilities()
        : FKsa64OperationsBridgeCapabilities{};
    ViewModel.PresentationPace = EKsa64OperationsPace::Paused;

    TickerHandle = FTSTicker::GetCoreTicker().AddTicker(
        FTickerDelegate::CreateUObject(this, &UKsa64LiveMissionSubsystem::Tick));
    AppendTimeline(TEXT("SYSTEM"), ViewModel.BridgeStatus, !ViewModel.bBridgeReady);

    bAcceptanceMode = FParse::Param(FCommandLine::Get(), TEXT("Ksa64Phase12bAcceptance"));
    if (bAcceptanceMode)
    {
        Accessibility.bSoundCues = false;
        AcceptanceStartedSeconds = FPlatformTime::Seconds();
        if (StartGuidedOperations())
        {
            AcceptancePhase = 1;
            ViewModel.PresentationPace = EKsa64OperationsPace::Fastest;
            UE_LOG(LogKsa64Operations, Display, TEXT("KSA64_PHASE12B_ACCEPTANCE_BEGIN"));
        }
        else
        {
            FailAcceptance(TEXT("typed Guided Operator session could not start"));
        }
    }
}

void UKsa64LiveMissionSubsystem::Deinitialize()
{
    if (TickerHandle.IsValid())
    {
        FTSTicker::GetCoreTicker().RemoveTicker(TickerHandle);
        TickerHandle.Reset();
    }
    if (GEngine != nullptr
        && GEngine->GameViewport != nullptr
        && Dashboard.IsValid()
        && bDashboardInstalled)
    {
        GEngine->GameViewport->RemoveViewportWidgetContent(Dashboard.ToSharedRef());
    }
    Dashboard.Reset();
    bDashboardInstalled = false;
    if (Bridge.IsValid())
    {
        Bridge->Close();
        Bridge.Reset();
    }
    Super::Deinitialize();
}

bool UKsa64LiveMissionSubsystem::StartGuidedOperations()
{
    if (!Bridge.IsValid() || !Bridge->IsReady())
    {
        ViewModel.LastDiagnostic = Bridge.IsValid()
            ? Bridge->GetDiagnostic()
            : TEXT("bridge adapter unavailable");
        AppendTimeline(TEXT("BRIDGE"), TEXT("Session start rejected"), true);
        return false;
    }

    if (!Bridge->StartGuidedOperations())
    {
        ViewModel.LastDiagnostic = Bridge->GetDiagnostic();
        AppendTimeline(TEXT("BRIDGE"), TEXT("Session start failed"), true);
        return false;
    }

    ReleaseHistory.Reset();
    Timeline.Reset();
    PredictionPath.Reset();
    LastObservedRelease = 0;
    LastObservedCommandSequence = 0;
    PacingController.Reset();
    AdvanceTracker.Reset();
    bEvidenceSaved = false;
    bAcceptanceVerified = false;
    ViewModel.bShutdownRequested = false;
    ViewModel.EvidencePath.Reset();
    ViewModel.EvidenceStatus = TEXT("EVIDENCE PENDING");
    ViewModel.bSessionOpen = true;
    ViewModel.SessionStatus = TEXT("OPENING");
    ViewModel.PresentationPace = EKsa64OperationsPace::Realtime;
    AppendTimeline(TEXT("MISSION"), TEXT("Guided GNSS-loss operations session opened"));
    EmitProceduralCue(TEXT("session-open"));
    PollBridge();
    return true;
}

void UKsa64LiveMissionSubsystem::PausePresentation()
{
    SetPace(EKsa64OperationsPace::Paused);
}

void UKsa64LiveMissionSubsystem::ResumeRealtime()
{
    SetPace(EKsa64OperationsPace::Realtime);
}

void UKsa64LiveMissionSubsystem::StepOneRelease()
{
    if (!Bridge.IsValid() || !ViewModel.bSessionOpen || AdvanceTracker.IsOutstanding())
    {
        return;
    }
    ViewModel.PresentationPace = EKsa64OperationsPace::Paused;
    PacingController.Reset();
    const EKsa64OperationsAdapterResult Result = Bridge->AdvanceOneRelease();
    HandleAdapterResult(Result, TEXT("single release"));
    if (Result == EKsa64OperationsAdapterResult::Ok
        || Result == EKsa64OperationsAdapterResult::Queued)
    {
        AdvanceTracker.MarkAccepted(ViewModel.CommandSequence);
        ViewModel.bAdvanceOutstanding = true;
    }
}

void UKsa64LiveMissionSubsystem::SetPace(EKsa64OperationsPace Pace)
{
    if (ViewModel.PresentationPace == Pace)
    {
        return;
    }
    ViewModel.PresentationPace = Pace;
    PacingController.Reset();
    AppendTimeline(TEXT("PACE"), GetPaceLabel().ToString());
}

void UKsa64LiveMissionSubsystem::ReviewAction()
{
    if (Bridge.IsValid()) HandleAdapterResult(Bridge->ReviewAction(), TEXT("review action"));
}

void UKsa64LiveMissionSubsystem::StageAction()
{
    if (Bridge.IsValid()) HandleAdapterResult(Bridge->StageAction(), TEXT("stage action"));
}

void UKsa64LiveMissionSubsystem::CommitAction()
{
    if (Bridge.IsValid()) HandleAdapterResult(Bridge->CommitAction(), TEXT("commit action"));
}

void UKsa64LiveMissionSubsystem::CancelAction()
{
    if (Bridge.IsValid()) HandleAdapterResult(Bridge->CancelAction(), TEXT("cancel action"));
}

void UKsa64LiveMissionSubsystem::RequestShutdown()
{
    if (!Bridge.IsValid() || !ViewModel.bSessionOpen || ViewModel.bShutdownRequested)
    {
        return;
    }
    const EKsa64OperationsAdapterResult Result = Bridge->RequestShutdown();
    HandleAdapterResult(Result, TEXT("shutdown request"));
    if (Result == EKsa64OperationsAdapterResult::Ok
        || Result == EKsa64OperationsAdapterResult::Queued)
    {
        ViewModel.bShutdownRequested = true;
        AppendTimeline(TEXT("SYSTEM"), TEXT("Graceful worker shutdown requested"));
    }
}

bool UKsa64LiveMissionSubsystem::SaveCompletedEvidence()
{
    if (!Bridge.IsValid() || ViewModel.FinalizationState != 2 || ViewModel.Lifecycle != 5)
    {
        ViewModel.EvidenceStatus = TEXT("EVIDENCE NOT READY; NOTHING SAVED");
        return false;
    }

    TArray<uint8> Evidence;
    if (Bridge->GetCompletedEvidence(Evidence) != EKsa64OperationsAdapterResult::Ok
        || Evidence.Num() <= 0
        || static_cast<uint64>(Evidence.Num()) != ViewModel.EvidenceLength
        || FCrc::MemCrc32(Evidence.GetData(), Evidence.Num()) != ViewModel.EvidenceCrc32)
    {
        ViewModel.EvidenceStatus = TEXT("EVIDENCE TRANSPORT VERIFICATION FAILED");
        ViewModel.bObservationComplete = false;
        AppendTimeline(TEXT("EVIDENCE"), ViewModel.EvidenceStatus, true);
        return false;
    }

    FSHA256Signature Signature = {};
    if (!FPlatformMisc::GetSHA256Signature(Evidence.GetData(), Evidence.Num(), Signature))
    {
        ViewModel.EvidenceStatus = TEXT("EVIDENCE SHA-256 FAILED");
        return false;
    }
    ViewModel.EvidenceSha256 = Signature.ToString().ToLower();
    if (bAcceptanceMode
        && (Evidence.Num() != 2'911'464
            || ViewModel.EvidenceSha256 != TEXT("7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4")))
    {
        ViewModel.EvidenceStatus = TEXT("ACCEPTANCE EVIDENCE IDENTITY MISMATCH");
        ViewModel.bObservationComplete = false;
        return false;
    }

    const FString Directory = FPaths::Combine(FPaths::ProjectSavedDir(), TEXT("KSA64"), TEXT("Evidence"));
    IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
    if (!PlatformFile.CreateDirectoryTree(*Directory))
    {
        ViewModel.EvidenceStatus = TEXT("EVIDENCE DIRECTORY CREATION FAILED");
        return false;
    }
    const FString Name = FString::Printf(
        TEXT("KSB11-%08X-%08X-%llu.ksb11"),
        ViewModel.EvidenceIdentity,
        ViewModel.EvidenceCrc32,
        static_cast<unsigned long long>(ViewModel.EvidenceLength));
    const FString FinalPath = FPaths::Combine(Directory, Name);
    if (PlatformFile.FileExists(*FinalPath))
    {
        TArray<uint8> Existing;
        if (FFileHelper::LoadFileToArray(Existing, *FinalPath) && Existing == Evidence)
        {
            ViewModel.EvidencePath = FinalPath;
            ViewModel.EvidenceStatus = TEXT("EVIDENCE VERIFIED / ALREADY SAVED");
            bEvidenceSaved = true;
            bAcceptanceVerified = !bAcceptanceMode
                || (Evidence.Num() == 2'911'464
                    && ViewModel.EvidenceSha256 == TEXT("7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"));
            return true;
        }
        ViewModel.EvidenceStatus = TEXT("EVIDENCE PATH COLLISION; EXISTING BYTES DIFFER");
        return false;
    }
    const FString TemporaryPath = FString::Printf(
        TEXT("%s.tmp-%u"),
        *FinalPath,
        FPlatformProcess::GetCurrentProcessId());
    PlatformFile.DeleteFile(*TemporaryPath);
    if (!FFileHelper::SaveArrayToFile(Evidence, *TemporaryPath)
        || !PlatformFile.MoveFile(*FinalPath, *TemporaryPath))
    {
        PlatformFile.DeleteFile(*TemporaryPath);
        ViewModel.EvidenceStatus = TEXT("ATOMIC EVIDENCE SAVE FAILED");
        return false;
    }
    ViewModel.EvidencePath = FinalPath;
    ViewModel.EvidenceStatus = TEXT("EVIDENCE VERIFIED / SAVED");
    bEvidenceSaved = true;
    bAcceptanceVerified = !bAcceptanceMode
        || (Evidence.Num() == 2'911'464
            && ViewModel.EvidenceSha256 == TEXT("7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"));
    AppendTimeline(TEXT("EVIDENCE"), ViewModel.EvidenceStatus);
    return true;
}

void UKsa64LiveMissionSubsystem::ToggleReducedMotion()
{
    Accessibility.bReducedMotion = !Accessibility.bReducedMotion;
}

void UKsa64LiveMissionSubsystem::ToggleHighContrast()
{
    Accessibility.bHighContrast = !Accessibility.bHighContrast;
}

void UKsa64LiveMissionSubsystem::ToggleSoundCues()
{
    Accessibility.bSoundCues = !Accessibility.bSoundCues;
}

void UKsa64LiveMissionSubsystem::CycleTextScale()
{
    if (Accessibility.TextScale < 1.24f)
    {
        Accessibility.TextScale = 1.25f;
    }
    else if (Accessibility.TextScale < 1.49f)
    {
        Accessibility.TextScale = 1.5f;
    }
    else
    {
        Accessibility.TextScale = 1.0f;
    }
}

FString UKsa64LiveMissionSubsystem::ExportSemanticStateJson() const
{
    FString Output;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Output);
    Writer->WriteObjectStart();
    Writer->WriteValue(TEXT("schema"), TEXT("ksa64.mission-foundry-semantic-state.v1"));
    Writer->WriteValue(TEXT("view"), ViewModel.ToDeterministicJson());
    Writer->WriteValue(TEXT("release_history_count"), ReleaseHistory.Num());
    Writer->WriteValue(TEXT("timeline_count"), Timeline.Num());
    Writer->WriteValue(TEXT("prediction_point_count"), PredictionPath.Num());
    Writer->WriteValue(TEXT("text_scale"), Accessibility.TextScale);
    Writer->WriteValue(TEXT("reduced_motion"), Accessibility.bReducedMotion);
    Writer->WriteValue(TEXT("high_contrast"), Accessibility.bHighContrast);
    Writer->WriteValue(TEXT("sound_cues"), Accessibility.bSoundCues);
    Writer->WriteObjectEnd();
    Writer->Close();
    return Output;
}

FText UKsa64LiveMissionSubsystem::GetPaceLabel() const
{
    switch (ViewModel.PresentationPace)
    {
    case EKsa64OperationsPace::Realtime: return FText::FromString(TEXT("REALTIME 1×"));
    case EKsa64OperationsPace::Paused: return FText::FromString(TEXT("PAUSED"));
    case EKsa64OperationsPace::FourX: return FText::FromString(TEXT("FAST 4×"));
    case EKsa64OperationsPace::SixteenX: return FText::FromString(TEXT("FAST 16×"));
    case EKsa64OperationsPace::Fastest: return FText::FromString(TEXT("MAXIMUM"));
    default: return FText::FromString(TEXT("UNKNOWN"));
    }
}

FText UKsa64LiveMissionSubsystem::GetMissionElapsedLabel() const
{
    if ((ViewModel.ValidityMask & (1ull << 1)) == 0)
    {
        return FText::FromString(TEXT("MET —"));
    }
    const double Seconds = static_cast<double>(ViewModel.MissionTimeQ16) / 65'536.0;
    const int32 Whole = FMath::Max(0, FMath::FloorToInt(Seconds));
    return FText::FromString(FString::Printf(
        TEXT("MET %02d:%02d:%02d.%03d"),
        Whole / 3600,
        (Whole / 60) % 60,
        Whole % 60,
        FMath::FloorToInt((Seconds - Whole) * 1000.0)));
}

FText UKsa64LiveMissionSubsystem::GetReleaseLabel() const
{
    return FText::FromString(FString::Printf(TEXT("REL %u"), ViewModel.ReleaseEpoch));
}

bool UKsa64LiveMissionSubsystem::Tick(float DeltaSeconds)
{
    InstallDashboardIfPossible();
    PollBridge();
    if (bAcceptanceMode)
    {
        TickAcceptance();
        return true;
    }

    const bool bRunnable = ViewModel.bSessionOpen
        && ViewModel.Lifecycle != 5
        && ViewModel.Lifecycle != 6
        && !ViewModel.bShutdownRequested;
    PacingController.Accumulate(DeltaSeconds, ViewModel.PresentationPace);
    const uint32 Releases = PacingController.ReleasesDue(
        ViewModel.PresentationPace,
        ViewModel.ReleasePeriodMicros,
        bRunnable,
        AdvanceTracker.IsOutstanding());
    if (Releases == 0 || !Bridge.IsValid())
    {
        return true;
    }

    const EKsa64OperationsAdapterResult Result = Bridge->AdvanceReleases(Releases);
    HandleAdapterResult(Result, TEXT("paced release batch"));
    if (Result == EKsa64OperationsAdapterResult::Ok
        || Result == EKsa64OperationsAdapterResult::Queued)
    {
        PacingController.CommitAcceptedAdvance(
            Releases,
            ViewModel.ReleasePeriodMicros,
            ViewModel.PresentationPace);
        AdvanceTracker.MarkAccepted(ViewModel.CommandSequence);
        ViewModel.bAdvanceOutstanding = true;
    }
    return true;
}

void UKsa64LiveMissionSubsystem::InstallDashboardIfPossible()
{
    if (bDashboardInstalled || GEngine == nullptr || GEngine->GameViewport == nullptr)
    {
        return;
    }
    Dashboard = SNew(SKsa64OperationsDashboard).Subsystem(this);
    GEngine->GameViewport->AddViewportWidgetContent(Dashboard.ToSharedRef(), 100);
    FSlateApplication::Get().SetKeyboardFocus(Dashboard, EFocusCause::SetDirectly);
    bDashboardInstalled = true;
}

void UKsa64LiveMissionSubsystem::PollBridge()
{
    if (!Bridge.IsValid() || !ViewModel.bSessionOpen)
    {
        return;
    }

    FKsa64OperationsViewModel Candidate;
    const EKsa64OperationsAdapterResult Result = Bridge->Poll(Candidate);
    if (Result != EKsa64OperationsAdapterResult::Ok)
    {
        if (Result != EKsa64OperationsAdapterResult::NoData
            && Result != EKsa64OperationsAdapterResult::Unchanged)
        {
            HandleAdapterResult(Result, TEXT("snapshot poll"));
        }
        return;
    }

    const FKsa64OperationsViewModel Previous = ViewModel;
    Candidate.PresentationPace = ViewModel.PresentationPace;
    Candidate.bShutdownRequested = ViewModel.bShutdownRequested;
    Candidate.EvidencePath = ViewModel.EvidencePath;
    Candidate.bAdvanceOutstanding = AdvanceTracker.IsOutstanding();
    ViewModel = MoveTemp(Candidate);

    TArray<FKsa64OperationsTimelineItem> TypedTimeline;
    Bridge->DrainTimeline(TypedTimeline);
    for (FKsa64OperationsTimelineItem& Item : TypedTimeline)
    {
        Timeline.Add(MoveTemp(Item));
    }
    if (Timeline.Num() > MaxTimelineItems)
    {
        Timeline.RemoveAt(0, Timeline.Num() - MaxTimelineItems, EAllowShrinking::No);
        ViewModel.bObservationComplete = false;
    }

    TArray<FKsa64OperationsReleasePoint> TypedSamples;
    Bridge->DrainReleaseSamples(TypedSamples);
    Ksa64OperationsPolicy::MergeReleaseSamples(
        ReleaseHistory,
        TypedSamples,
        MaxReleaseHistory,
        ViewModel.bObservationComplete);
    for (const FKsa64OperationsReleasePoint& Point : TypedSamples)
    {
        LastObservedRelease = FMath::Max(LastObservedRelease, Point.ReleaseEpoch);
    }
    Bridge->ReadPredictionPath(PredictionPath);

    LastObservedCommandSequence = ViewModel.CommandSequence;
    if (AdvanceTracker.Observe(
        ViewModel.CommandSequence,
        ViewModel.CommandsPending,
        ViewModel.Lifecycle))
    {
        ViewModel.bAdvanceOutstanding = false;
    }
    ObserveSnapshot(Previous);
    ObserveCompletionAndShutdown();
}

void UKsa64LiveMissionSubsystem::ObserveSnapshot(
    const FKsa64OperationsViewModel& Previous)
{
    if (ViewModel.ReleaseEpoch != LastObservedRelease)
    {
        LastObservedRelease = ViewModel.ReleaseEpoch;
        FKsa64OperationsReleasePoint Point;
        Point.ReleaseEpoch = ViewModel.ReleaseEpoch;
        Point.MissionTimeQ16 = ViewModel.MissionTimeQ16;
        Point.bHasMissionTime = (ViewModel.ValidityMask & (1ull << 1)) != 0;
        Point.bHasPosition = (ViewModel.ValidityMask & (1ull << 2)) != 0;
        for (int32 Axis = 0; Axis < 3; ++Axis)
        {
            Point.PositionQ12[Axis] = ViewModel.NavigationPositionQ12[Axis];
        }
        TArray<FKsa64OperationsReleasePoint> SparsePoint;
        SparsePoint.Add(Point);
        Ksa64OperationsPolicy::MergeReleaseSamples(
            ReleaseHistory,
            SparsePoint,
            MaxReleaseHistory,
            ViewModel.bObservationComplete);
    }
    if (ViewModel.FrameIdentity != Previous.FrameIdentity)
    {
        AppendTimeline(TEXT("FRAME"), ViewModel.FrameLabel, true);
    }
    if (ViewModel.ProcedureStep != Previous.ProcedureStep
        || ViewModel.ProcedureState != Previous.ProcedureState)
    {
        AppendTimeline(TEXT("PROCEDURE"), ViewModel.ProcedureLabel, true);
        EmitProceduralCue(TEXT("procedure-change"));
    }
    if (ViewModel.StagedLoadIdentity != Previous.StagedLoadIdentity)
    {
        AppendTimeline(TEXT("UPLINK"), ViewModel.UplinkLabel, true);
    }
    if (ViewModel.Lifecycle != Previous.Lifecycle)
    {
        AppendTimeline(TEXT("MISSION"), ViewModel.SessionStatus, true);
    }
}

bool UKsa64LiveMissionSubsystem::QueueAcceptanceAdvance(uint32 TargetRelease)
{
    if (ViewModel.ReleaseEpoch >= TargetRelease)
    {
        return ViewModel.ReleaseEpoch == TargetRelease;
    }
    if (AdvanceTracker.IsOutstanding())
    {
        return true;
    }
    const uint32 Count = FMath::Min<uint32>(64, TargetRelease - ViewModel.ReleaseEpoch);
    const EKsa64OperationsAdapterResult Result = Bridge->AdvanceReleases(Count);
    if (Result != EKsa64OperationsAdapterResult::Ok
        && Result != EKsa64OperationsAdapterResult::Queued)
    {
        FailAcceptance(FString::Printf(TEXT("advance to release %u failed"), TargetRelease));
        return false;
    }
    AdvanceTracker.MarkAccepted(ViewModel.CommandSequence);
    ViewModel.bAdvanceOutstanding = true;
    return true;
}

void UKsa64LiveMissionSubsystem::FailAcceptance(const FString& Reason)
{
    if (bAcceptanceFailed)
    {
        return;
    }
    bAcceptanceFailed = true;
    AcceptanceFailureReason = Reason;
    AcceptancePhase = 250;
    UE_LOG(LogKsa64Operations, Error, TEXT("KSA64_PHASE12B_ACCEPTANCE_FAIL_PENDING: %s"), *Reason);
    RequestShutdown();
}

void UKsa64LiveMissionSubsystem::TickAcceptance()
{
    if (bAcceptanceExitRequested)
    {
        return;
    }
    if (FPlatformTime::Seconds() - AcceptanceStartedSeconds > 180.0 && !bAcceptanceFailed)
    {
        FailAcceptance(TEXT("acceptance timed out after 180 seconds"));
    }
    if (bAcceptanceFailed)
    {
        if (!ViewModel.bSessionOpen || ViewModel.WorkerState == 2 || ViewModel.WorkerState == 3)
        {
            bAcceptanceExitRequested = true;
            UE_LOG(LogKsa64Operations, Error, TEXT("KSA64_PHASE12B_ACCEPTANCE_FAIL: %s"), *AcceptanceFailureReason);
            FPlatformMisc::RequestExitWithStatus(true, 1, TEXT("Phase12B acceptance failure"));
        }
        return;
    }
    if (!Bridge.IsValid())
    {
        FailAcceptance(TEXT("operations adapter disappeared"));
        return;
    }

    switch (AcceptancePhase)
    {
    case 1:
        if (!QueueAcceptanceAdvance(6'080)) return;
        if (ViewModel.ReleaseEpoch == 6'080 && !AdvanceTracker.IsOutstanding())
        {
            if (Bridge->ReviewAction() != EKsa64OperationsAdapterResult::Ok
                || Bridge->StageAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("ground update review/stage failed at release 6080"));
                return;
            }
            AcceptancePhase = 2;
        }
        break;
    case 2:
        if (ViewModel.ActionReceiptState == 1 && ViewModel.ActionReceiptAccepted != 0)
            AcceptancePhase = 3;
        break;
    case 3:
        if (!QueueAcceptanceAdvance(6'240)) return;
        if (ViewModel.ReleaseEpoch == 6'240 && !AdvanceTracker.IsOutstanding())
        {
            if (Bridge->CommitAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("ground update commit failed at release 6240"));
                return;
            }
            AcceptancePhase = 4;
        }
        break;
    case 4:
        if (ViewModel.ActionReceiptState == 2 && ViewModel.ActionReceiptAccepted != 0)
            AcceptancePhase = 5;
        break;
    case 5:
        if (!QueueAcceptanceAdvance(6'560)) return;
        if (ViewModel.ReleaseEpoch == 6'560 && !AdvanceTracker.IsOutstanding())
        {
            if (Bridge->ReviewAction() != EKsa64OperationsAdapterResult::Ok
                || Bridge->StageAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("branch review/stage failed at release 6560"));
                return;
            }
            AcceptancePhase = 6;
        }
        break;
    case 6:
        if (ViewModel.ActionReceiptState == 1 && ViewModel.ActionReceiptAccepted != 0)
            AcceptancePhase = 7;
        break;
    case 7:
        if (!QueueAcceptanceAdvance(6'720)) return;
        if (ViewModel.ReleaseEpoch == 6'720 && !AdvanceTracker.IsOutstanding())
        {
            if (Bridge->CommitAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("branch commit failed at release 6720"));
                return;
            }
            AcceptancePhase = 8;
        }
        break;
    case 8:
        if (ViewModel.ActionReceiptState == 2 && ViewModel.ActionReceiptAccepted != 0)
            AcceptancePhase = 9;
        break;
    case 9:
        if (!QueueAcceptanceAdvance(21'591)) return;
        if (ViewModel.ReleaseEpoch == 21'591 && !AdvanceTracker.IsOutstanding())
            AcceptancePhase = 10;
        break;
    case 10:
        if (bAcceptanceVerified && !ViewModel.bSessionOpen)
        {
            bAcceptanceExitRequested = true;
            UE_LOG(
                LogKsa64Operations,
                Display,
                TEXT("KSA64_PHASE12B_ACCEPTANCE_PASS release=21591 length=2911464 sha256=7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4 path=%s"),
                *ViewModel.EvidencePath);
            FPlatformMisc::RequestExitWithStatus(true, 0, TEXT("Phase12B acceptance complete"));
        }
        else if (ViewModel.Lifecycle == 5 && ViewModel.FinalizationState != 2)
        {
            FailAcceptance(TEXT("mission completed without verified finalization"));
        }
        break;
    default:
        break;
    }
}

void UKsa64LiveMissionSubsystem::ObserveCompletionAndShutdown()
{
    const bool bTerminal = ViewModel.Lifecycle == 5 || ViewModel.Lifecycle == 6;
    if (ViewModel.FinalizationState == 2 && ViewModel.Lifecycle == 5 && !bEvidenceSaved)
    {
        SaveCompletedEvidence();
    }
    else if (ViewModel.FinalizationState == 3 || ViewModel.WorkerState == 3)
    {
        ViewModel.EvidenceStatus = TEXT("EVIDENCE FAILED / UNAVAILABLE");
        ViewModel.bObservationComplete = false;
    }

    const bool bWorkerTerminal = ViewModel.WorkerState == 2 || ViewModel.WorkerState == 3;
    const bool bSafeToClose = bWorkerTerminal
        && (bTerminal || ViewModel.bShutdownRequested)
        && (ViewModel.FinalizationState != 2 || bEvidenceSaved);
    if (bSafeToClose && Bridge.IsValid())
    {
        Bridge->Close();
        ViewModel.bSessionOpen = false;
        ViewModel.bAdvanceOutstanding = false;
        AdvanceTracker.Reset();
        AppendTimeline(TEXT("SYSTEM"), TEXT("Mission worker closed cleanly"));
    }
}

void UKsa64LiveMissionSubsystem::AppendTimeline(
    const FString& Category,
    const FString& Summary,
    bool bAttention)
{
    FKsa64OperationsTimelineItem Item;
    Item.Sequence = Timeline.IsEmpty() ? 1 : Timeline.Last().Sequence + 1;
    Item.ReleaseEpoch = ViewModel.ReleaseEpoch;
    Item.Category = Category;
    Item.Summary = Summary;
    Item.bAttention = bAttention;
    Timeline.Add(MoveTemp(Item));
    if (Timeline.Num() > MaxTimelineItems)
    {
        Timeline.RemoveAt(0, Timeline.Num() - MaxTimelineItems, EAllowShrinking::No);
        ViewModel.bObservationComplete = false;
    }
}

void UKsa64LiveMissionSubsystem::HandleAdapterResult(
    EKsa64OperationsAdapterResult Result,
    const TCHAR* Operation)
{
    switch (Result)
    {
    case EKsa64OperationsAdapterResult::Ok:
    case EKsa64OperationsAdapterResult::Queued:
        return;
    case EKsa64OperationsAdapterResult::NoData:
    case EKsa64OperationsAdapterResult::Unchanged:
        return;
    case EKsa64OperationsAdapterResult::Unsupported:
        ViewModel.LastDiagnostic = FString::Printf(
            TEXT("%s requires the typed Phase 12B bridge capability"),
            Operation);
        break;
    case EKsa64OperationsAdapterResult::QueueFull:
        ViewModel.LastDiagnostic = FString::Printf(TEXT("%s: bridge queue full"), Operation);
        break;
    case EKsa64OperationsAdapterResult::Lifecycle:
        ViewModel.LastDiagnostic = FString::Printf(TEXT("%s: invalid lifecycle"), Operation);
        break;
    default:
        ViewModel.LastDiagnostic = FString::Printf(TEXT("%s failed"), Operation);
        break;
    }
    AppendTimeline(TEXT("BRIDGE"), ViewModel.LastDiagnostic, true);
}

void UKsa64LiveMissionSubsystem::EmitProceduralCue(const TCHAR* CueName)
{
    if (!Accessibility.bSoundCues)
    {
        return;
    }
    // Phase 12B deliberately carries no licensed sound asset. This named,
    // noncanonical hook is ready for the generated cue bank in Phase 12E.
    UE_LOG(LogKsa64Operations, Verbose, TEXT("procedural cue: %s"), CueName);
}
