#include "Ksa64LiveMissionSubsystem.h"

#include "Ksa64OperationsBridgeAdapter.h"
#include "Ksa64OperationsDashboard.h"

#include "Containers/Ticker.h"
#include "DynamicRHI.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "ImageUtils.h"
#include "HAL/PlatformFileManager.h"
#include "HAL/PlatformMisc.h"
#include "HAL/PlatformProcess.h"
#include "HAL/PlatformTime.h"
#include "Misc/CommandLine.h"
#include "Misc/App.h"
#include "Misc/Crc.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "UnrealClient.h"
#include "Framework/Application/SlateApplication.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "Widgets/SWidget.h"

DEFINE_LOG_CATEGORY_STATIC(LogKsa64Operations, Log, All);

namespace
{
constexpr int32 MaxReleaseHistory = 32'768;
constexpr int32 MaxTimelineItems = 512;
constexpr uint32 PresentationCaptureRelease = 6'080;
constexpr uint32 PresentationWarmupFrameCount = 120;
constexpr uint32 PresentationMeasuredFrameCount = 600;
constexpr int64 PresentationP99LimitNanoseconds = 1'000'000;
constexpr int64 PresentationMaximumLimitNanoseconds = 2'000'000;
constexpr int32 PresentationWidth = 1'920;
constexpr int32 PresentationHeight = 1'080;
constexpr uint32 PresentationExpectedReleaseDelta = 320;
constexpr uint32 PresentationDashboardReadyFrameLimit = 600;
constexpr int32 PresentationMinimumLuminanceRange = 24;
constexpr int32 PresentationMinimumColorBuckets = 8;

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
    bPresentationEvidenceMode = FParse::Param(
        FCommandLine::Get(),
        TEXT("Ksa64Phase12bPresentationEvidence"));
    if (bAcceptanceMode && bPresentationEvidenceMode)
    {
        UE_LOG(
            LogKsa64Operations,
            Error,
            TEXT("KSA64_PHASE12B_PRESENTATION_EVIDENCE_FAIL: acceptance modes are mutually exclusive"));
        FPlatformMisc::RequestExitWithStatus(
            false,
            1,
            TEXT("Phase12B acceptance modes are mutually exclusive"));
        return;
    }
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
    else if (bPresentationEvidenceMode)
    {
        Accessibility.TextScale = 1.25f;
        Accessibility.bReducedMotion = true;
        Accessibility.bHighContrast = true;
        Accessibility.bSoundCues = false;
        PresentationEvidenceStartedSeconds = FPlatformTime::Seconds();
        PresentationEvidenceDirectory = FPaths::Combine(
            FPaths::ProjectSavedDir(),
            TEXT("KSA64"),
            TEXT("PresentationEvidence"));
        PresentationScreenshotPath = FPaths::Combine(
            PresentationEvidenceDirectory,
            TEXT("phase12b-gnss-loss-operations-1920x1080.png"));
        PresentationSemanticPath = FPaths::Combine(
            PresentationEvidenceDirectory,
            TEXT("phase12b-gnss-loss-operations-semantic.json"));
        PresentationManifestPath = FPaths::Combine(
            PresentationEvidenceDirectory,
            TEXT("phase12b-presentation-evidence.json"));
        PresentationEvidenceServiceNanoseconds.Reserve(PresentationMeasuredFrameCount);
        if (StartGuidedOperations())
        {
            PresentationEvidencePhase = 1;
            ViewModel.PresentationPace = EKsa64OperationsPace::Fastest;
            UE_LOG(
                LogKsa64Operations,
                Display,
                TEXT("KSA64_PHASE12B_PRESENTATION_EVIDENCE_BEGIN release_target=6080 frames=600"));
        }
        else
        {
            FailPresentationEvidence(TEXT("typed Guided Operator session could not start"));
        }
    }
}

void UKsa64LiveMissionSubsystem::Deinitialize()
{
    if (PresentationScreenshotProcessedHandle.IsValid())
    {
        FScreenshotRequest::OnScreenshotRequestProcessed().Remove(PresentationScreenshotProcessedHandle);
        PresentationScreenshotProcessedHandle.Reset();
    }
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
    bGlobalReplayMode = false;
    bRetainCompletedGlobalDisplaySession = false;
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
    PlannedReferencePath.Reset();
    OnboardPredictionPath.Reset();
    GroundPredictionPath.Reset();
    LastVisualSampleSeconds = FPlatformTime::Seconds();
    VisualSnapUntilSeconds = LastVisualSampleSeconds;
    LastObservedRelease = 0;
    LastObservedCommandSequence = 0;
    PacingController.Reset();
    AdvanceTracker.Reset();
    bEvidenceSaved = false;
    bAcceptanceVerified = false;
    ViewModel.bShutdownRequested = false;
    ViewModel.EvidencePath.Reset();
    ViewModel.EvidenceSha256.Reset();
    ViewModel.EvidenceStatus = TEXT("EVIDENCE PENDING");
    ViewModel.bSessionOpen = true;
    ViewModel.SessionStatus = TEXT("OPENING");
    ViewModel.PresentationPace = EKsa64OperationsPace::Realtime;
    AppendTimeline(TEXT("MISSION"), TEXT("Guided GNSS-loss operations session opened"));
    EmitProceduralCue(TEXT("session-open"));
    PollBridge();
    return true;
}

bool UKsa64LiveMissionSubsystem::StartNominalGlobalReplay()
{
    bRetainCompletedGlobalDisplaySession = false;
    if (!Bridge.IsValid() || !Bridge->IsReady())
    {
        ViewModel.LastDiagnostic = Bridge.IsValid()
            ? Bridge->GetDiagnostic()
            : TEXT("bridge adapter unavailable");
        AppendTimeline(TEXT("BRIDGE"), TEXT("Nominal replay start rejected"), true);
        return false;
    }
    if (!Bridge->StartNominalGlobalReplay())
    {
        ViewModel.LastDiagnostic = Bridge->GetDiagnostic();
        AppendTimeline(TEXT("BRIDGE"), TEXT("Nominal replay validation start failed"), true);
        return false;
    }

    bGlobalReplayMode = true;
    ReleaseHistory.Reset();
    Timeline.Reset();
    PlannedReferencePath.Reset();
    OnboardPredictionPath.Reset();
    GroundPredictionPath.Reset();
    LastObservedRelease = 0;
    LastObservedCommandSequence = 0;
    PacingController.Reset();
    AdvanceTracker.Reset();
    bEvidenceSaved = false;
    bAcceptanceVerified = false;

    const FKsa64OperationsBridgeCapabilities EmptyCapabilities;
    ViewModel = FKsa64OperationsViewModel{};
    ViewModel.bBridgeReady = true;
    ViewModel.bSessionOpen = true;
    ViewModel.bSnapshotValid = false;
    ViewModel.bTruthFiltered = false;
    ViewModel.BridgeStatus = TEXT("BRIDGE 12C GLOBAL DISPLAY");
    ViewModel.SessionStatus = TEXT("VERIFYING FROZEN PHASE 10 REPLAY");
    ViewModel.RoleLabel = TEXT("SIM DIRECTOR · READ ONLY");
    ViewModel.PresentationPace = EKsa64OperationsPace::Paused;
    ViewModel.LastDiagnostic = Bridge->GetDiagnostic();
    ViewModel.Capabilities = EmptyCapabilities;
    ViewModel.EvidenceStatus = TEXT("CANONICAL EVIDENCE VALIDATION REQUIRED");
    AppendTimeline(TEXT("REPLAY"), TEXT("Frozen Phase 10 nominal replay validation started"));
    return true;
}

bool UKsa64LiveMissionSubsystem::SupportsGlobalDisplayV1() const
{
    return Bridge.IsValid() && Bridge->SupportsGlobalDisplayV1();
}

EKsa64OperationsAdapterResult UKsa64LiveMissionSubsystem::GetGlobalDisplayAvailability(
    Ksa64GlobalDisplayAvailabilityV1& OutAvailability) const
{
    return Bridge.IsValid()
        ? Bridge->GlobalDisplayAvailability(OutAvailability)
        : EKsa64OperationsAdapterResult::Unsupported;
}

EKsa64OperationsAdapterResult UKsa64LiveMissionSubsystem::GetGlobalDisplayDefinition(
    TArray<uint8>& OutPayload) const
{
    return Bridge.IsValid()
        ? Bridge->GlobalDisplayDefinition(OutPayload)
        : EKsa64OperationsAdapterResult::Unsupported;
}

EKsa64OperationsAdapterResult UKsa64LiveMissionSubsystem::PollGlobalDisplaySample(
    TArray<uint8>& OutPayload) const
{
    return Bridge.IsValid()
        ? Bridge->PollGlobalDisplaySample(OutPayload)
        : EKsa64OperationsAdapterResult::Unsupported;
}

EKsa64OperationsAdapterResult UKsa64LiveMissionSubsystem::GetGlobalDisplaySampleRange(
    const Ksa64GlobalDisplaySampleRangeRequestV1& Request,
    TArray<uint8>& OutPayload) const
{
    return Bridge.IsValid()
        ? Bridge->GlobalDisplaySampleRange(Request, OutPayload)
        : EKsa64OperationsAdapterResult::Unsupported;
}

EKsa64OperationsAdapterResult UKsa64LiveMissionSubsystem::PollGlobalDisplayTransition(
    TArray<uint8>& OutPayload) const
{
    return Bridge.IsValid()
        ? Bridge->PollGlobalDisplayTransition(OutPayload)
        : EKsa64OperationsAdapterResult::Unsupported;
}

EKsa64OperationsAdapterResult UKsa64LiveMissionSubsystem::GetGlobalReplayIndex(
    TArray<uint8>& OutPayload) const
{
    return Bridge.IsValid()
        ? Bridge->GlobalReplayIndex(OutPayload)
        : EKsa64OperationsAdapterResult::Unsupported;
}

EKsa64OperationsAdapterResult UKsa64LiveMissionSubsystem::GetGlobalPathChunk(
    const Ksa64GlobalDisplayPathRequestV1& Request,
    TArray<uint8>& OutPayload) const
{
    return Bridge.IsValid()
        ? Bridge->GlobalPathChunk(Request, OutPayload)
        : EKsa64OperationsAdapterResult::Unsupported;
}

void UKsa64LiveMissionSubsystem::SetDashboardVisible(bool bVisible)
{
    bDashboardRequestedVisible = bVisible;
    if (Dashboard.IsValid())
    {
        Dashboard->SetVisibility(
            bDashboardRequestedVisible
                ? EVisibility::Visible
                : EVisibility::Collapsed);
    }
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
    if (bGlobalReplayMode)
    {
        return;
    }
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
        AdvanceTracker.MarkAccepted(
            ViewModel.CommandSequence,
            ViewModel.ReleaseEpoch);
        ViewModel.bAdvanceOutstanding = true;
    }
}

void UKsa64LiveMissionSubsystem::SetPace(EKsa64OperationsPace Pace)
{
    if (bGlobalReplayMode)
    {
        return;
    }
    if (ViewModel.PresentationPace == Pace)
    {
        return;
    }
    ViewModel.PresentationPace = Pace;
    VisualSnapUntilSeconds = FPlatformTime::Seconds()
        + static_cast<double>(ViewModel.ReleasePeriodMicros) / 1'000'000.0;
    PacingController.Reset();
    AppendTimeline(TEXT("PACE"), GetPaceLabel().ToString());
}

bool UKsa64LiveMissionSubsystem::QueueBoundedAdvanceToRelease(
    uint32 TargetRelease,
    uint32 MaximumBatch)
{
    if (bGlobalReplayMode
        || !Bridge.IsValid()
        || !ViewModel.bSessionOpen
        || ViewModel.bShutdownRequested
        || ViewModel.Lifecycle == 5
        || ViewModel.Lifecycle == 6
        || ViewModel.ReleaseEpoch > TargetRelease)
    {
        return false;
    }
    if (ViewModel.ReleaseEpoch == TargetRelease || AdvanceTracker.IsOutstanding())
    {
        return true;
    }
    SetPace(EKsa64OperationsPace::Paused);
    const uint32 Count = FMath::Min(
        FMath::Clamp(MaximumBatch, 1u, Ksa64OperationsPolicy::MaximumAdvanceReleases),
        TargetRelease - ViewModel.ReleaseEpoch);
    const EKsa64OperationsAdapterResult Result = Bridge->AdvanceReleases(Count);
    HandleAdapterResult(Result, TEXT("bounded exact-release advance"));
    if (Result != EKsa64OperationsAdapterResult::Ok
        && Result != EKsa64OperationsAdapterResult::Queued)
    {
        return false;
    }
    AdvanceTracker.MarkAccepted(
        ViewModel.CommandSequence,
        ViewModel.ReleaseEpoch);
    ViewModel.bAdvanceOutstanding = true;
    return true;
}

void UKsa64LiveMissionSubsystem::ReviewAction()
{
    if (Bridge.IsValid()
        && ViewModel.bSessionOpen
        && ViewModel.Capabilities.bTypedActions
        && !ViewModel.bShutdownRequested
        && !AdvanceTracker.IsOutstanding()
        && ViewModel.CommandsPending == 0
        && ViewModel.Lifecycle != 5
        && ViewModel.Lifecycle != 6)
    {
        HandleAdapterResult(Bridge->ReviewAction(), TEXT("review action"));
    }
}

void UKsa64LiveMissionSubsystem::StageAction()
{
    if (Bridge.IsValid()
        && ViewModel.bSessionOpen
        && ViewModel.Capabilities.bTypedActions
        && !ViewModel.bShutdownRequested
        && !AdvanceTracker.IsOutstanding()
        && ViewModel.CommandsPending == 0
        && ViewModel.Lifecycle != 5
        && ViewModel.Lifecycle != 6)
    {
        HandleAdapterResult(Bridge->StageAction(), TEXT("stage action"));
    }
}

void UKsa64LiveMissionSubsystem::CommitAction()
{
    if (Bridge.IsValid()
        && ViewModel.bSessionOpen
        && ViewModel.Capabilities.bTypedActions
        && !ViewModel.bShutdownRequested
        && !AdvanceTracker.IsOutstanding()
        && ViewModel.CommandsPending == 0
        && ViewModel.Lifecycle != 5
        && ViewModel.Lifecycle != 6)
    {
        HandleAdapterResult(Bridge->CommitAction(), TEXT("commit action"));
    }
}

void UKsa64LiveMissionSubsystem::CancelAction()
{
    if (Bridge.IsValid()
        && ViewModel.bSessionOpen
        && ViewModel.Capabilities.bTypedActions
        && !ViewModel.bShutdownRequested
        && !AdvanceTracker.IsOutstanding()
        && ViewModel.CommandsPending == 0
        && ViewModel.Lifecycle != 5
        && ViewModel.Lifecycle != 6)
    {
        HandleAdapterResult(Bridge->CancelAction(), TEXT("cancel action"));
    }
}

bool UKsa64LiveMissionSubsystem::RequestShutdown()
{
    if (!ViewModel.bSessionOpen || ViewModel.bShutdownRequested)
    {
        return true;
    }
    if (!Bridge.IsValid())
    {
        return false;
    }
    const EKsa64OperationsAdapterResult Result = Bridge->RequestShutdown();
    HandleAdapterResult(Result, TEXT("shutdown request"));
    if (Result == EKsa64OperationsAdapterResult::Ok
        || Result == EKsa64OperationsAdapterResult::Queued)
    {
        ViewModel.bShutdownRequested = true;
        if (bGlobalReplayMode)
        {
            bGlobalReplayMode = false;
            ViewModel.bSessionOpen = false;
            ViewModel.bAdvanceOutstanding = false;
            AdvanceTracker.Reset();
        }
        AppendTimeline(TEXT("SYSTEM"), TEXT("Graceful worker shutdown requested"));
        return true;
    }
    return false;
}

bool UKsa64LiveMissionSubsystem::SetCompletedGlobalDisplayRetention(bool bRetain)
{
    if (bRetain
        && (!Bridge.IsValid() || !Bridge->SupportsGlobalDisplayV1()))
    {
        return false;
    }
    bRetainCompletedGlobalDisplaySession = bRetain;
    if (!bRetain && ViewModel.bSessionOpen)
    {
        ObserveCompletionAndShutdown();
    }
    return true;
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

    ViewModel.EvidenceSha256 = Ksa64OperationsPolicy::Sha256Hex(
        Evidence.GetData(),
        static_cast<uint64>(Evidence.Num()));
    if (ViewModel.EvidenceSha256.Len() != 64)
    {
        ViewModel.EvidenceStatus = TEXT("EVIDENCE SHA-256 FAILED");
        return false;
    }
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
    VisualSnapUntilSeconds = FPlatformTime::Seconds()
        + static_cast<double>(ViewModel.ReleasePeriodMicros) / 1'000'000.0;
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

void UKsa64LiveMissionSubsystem::ToggleDisplayMode()
{
    DisplayMode = DisplayMode == EKsa64OperationsDisplayMode::Smooth
        ? EKsa64OperationsDisplayMode::Exact
        : EKsa64OperationsDisplayMode::Smooth;
    VisualSnapUntilSeconds = FPlatformTime::Seconds()
        + static_cast<double>(ViewModel.ReleasePeriodMicros) / 1'000'000.0;
}

EKsa64OperationsDisplayMode UKsa64LiveMissionSubsystem::GetDisplayMode() const
{
    return Accessibility.bReducedMotion
        ? EKsa64OperationsDisplayMode::Exact
        : DisplayMode;
}

bool UKsa64LiveMissionSubsystem::GetVisualObservedPoint(
    FKsa64OperationsReleasePoint& OutPoint) const
{
    if (ReleaseHistory.IsEmpty())
    {
        return false;
    }
    OutPoint = ReleaseHistory.Last();
    OutPoint.PresentationReleaseEpoch = static_cast<double>(OutPoint.ReleaseEpoch);
    if (GetDisplayMode() == EKsa64OperationsDisplayMode::Exact
        || ViewModel.PresentationPace != EKsa64OperationsPace::Realtime
        || ViewModel.Lifecycle == 5
        || ViewModel.Lifecycle == 6
        || ReleaseHistory.Num() < 2
        || FPlatformTime::Seconds() < VisualSnapUntilSeconds)
    {
        return true;
    }
    const FKsa64OperationsReleasePoint& Previous = ReleaseHistory[ReleaseHistory.Num() - 2];
    const FKsa64OperationsReleasePoint& Current = ReleaseHistory.Last();
    if (Current.FrameIdentity != Previous.FrameIdentity
        || Current.ReleaseEpoch <= Previous.ReleaseEpoch
        || Current.ReleaseEpoch - Previous.ReleaseEpoch > 64
        || !Current.bHasMissionTime
        || !Previous.bHasMissionTime)
    {
        return true;
    }
    const double IntervalSeconds = static_cast<double>(
        Current.ReleaseEpoch - Previous.ReleaseEpoch)
        * static_cast<double>(ViewModel.ReleasePeriodMicros)
        / 1'000'000.0;
    if (IntervalSeconds <= 0.0)
    {
        return true;
    }
    const double Alpha = FMath::Clamp(
        (FPlatformTime::Seconds() - LastVisualSampleSeconds) / IntervalSeconds,
        0.0,
        1.0);
    const auto Interpolate = [Alpha](int32 From, int32 To)
    {
        return FMath::RoundToInt(
            static_cast<double>(From)
            + (static_cast<double>(To) - static_cast<double>(From)) * Alpha);
    };
    OutPoint.PresentationReleaseEpoch = static_cast<double>(Previous.ReleaseEpoch)
        + static_cast<double>(Current.ReleaseEpoch - Previous.ReleaseEpoch) * Alpha;
    OutPoint.MissionTimeQ16 = static_cast<uint32>(FMath::Max<int64>(0, FMath::RoundToInt64(
        static_cast<double>(Previous.MissionTimeQ16)
        + static_cast<double>(
            static_cast<int64>(Current.MissionTimeQ16)
            - static_cast<int64>(Previous.MissionTimeQ16)) * Alpha)));
    OutPoint.AltitudeQ12Km = Interpolate(Previous.AltitudeQ12Km, Current.AltitudeQ12Km);
    OutPoint.SpeedQ24KmS = Interpolate(Previous.SpeedQ24KmS, Current.SpeedQ24KmS);
    OutPoint.DownrangeQ12Km = Interpolate(Previous.DownrangeQ12Km, Current.DownrangeQ12Km);
    OutPoint.CrossrangeQ12Km = Interpolate(Previous.CrossrangeQ12Km, Current.CrossrangeQ12Km);
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        OutPoint.PositionQ12[Axis] = Interpolate(
            Previous.PositionQ12[Axis],
            Current.PositionQ12[Axis]);
        OutPoint.GroundPositionQ12[Axis] = Interpolate(
            Previous.GroundPositionQ12[Axis],
            Current.GroundPositionQ12[Axis]);
    }
    return true;
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
    Writer->WriteValue(TEXT("planned_reference_point_count"), PlannedReferencePath.Num());
    Writer->WriteValue(TEXT("onboard_prediction_point_count"), OnboardPredictionPath.Num());
    Writer->WriteValue(TEXT("ground_prediction_point_count"), GroundPredictionPath.Num());
    Writer->WriteValue(TEXT("display_mode"),
        GetDisplayMode() == EKsa64OperationsDisplayMode::Exact ? TEXT("exact") : TEXT("smooth"));
    Writer->WriteValue(TEXT("text_scale"), Accessibility.TextScale);
    Writer->WriteValue(TEXT("reduced_motion"), Accessibility.bReducedMotion);
    Writer->WriteValue(TEXT("high_contrast"), Accessibility.bHighContrast);
    Writer->WriteValue(TEXT("sound_cues"), Accessibility.bSoundCues);
    Writer->WriteValue(TEXT("presentation_pace"), GetPaceLabel().ToString());
    Writer->WriteValue(TEXT("dashboard_installed"), bDashboardInstalled);
    Writer->WriteValue(TEXT("capture_release_epoch"), ViewModel.ReleaseEpoch);
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

#if WITH_DEV_AUTOMATION_TESTS
bool UKsa64LiveMissionSubsystem::InitializeForAutomation()
{
    if (Bridge.IsValid())
    {
        Bridge->Close();
    }
    Bridge = IKsa64OperationsBridgeAdapter::Create();
    ViewModel = FKsa64OperationsViewModel{};
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
    return ViewModel.bBridgeReady;
}

bool UKsa64LiveMissionSubsystem::AdvanceToReleaseForAutomation(
    uint32 TargetRelease,
    double TimeoutSeconds)
{
    const double Deadline = FPlatformTime::Seconds() + TimeoutSeconds;
    for (;;)
    {
        PollBridge();
        if (ViewModel.ReleaseEpoch > TargetRelease)
        {
            return false;
        }
        if (ViewModel.WorkerState == 3
            || ViewModel.FinalizationState == 3
            || ViewModel.Lifecycle == 6
            || ViewModel.TransportOverflow != 0
            || !Bridge.IsValid())
        {
            return false;
        }
        if (ViewModel.ReleaseEpoch == TargetRelease
            && !AdvanceTracker.IsOutstanding()
            && ViewModel.CommandsPending == 0)
        {
            return true;
        }
        if (!AdvanceTracker.IsOutstanding())
        {
            const uint32 Count = FMath::Min<uint32>(
                64,
                TargetRelease - ViewModel.ReleaseEpoch);
            const EKsa64OperationsAdapterResult Result = Bridge->AdvanceReleases(Count);
            if (Result != EKsa64OperationsAdapterResult::Ok
                && Result != EKsa64OperationsAdapterResult::Queued)
            {
                return false;
            }
            AdvanceTracker.MarkAccepted(
                ViewModel.CommandSequence,
                ViewModel.ReleaseEpoch);
            ViewModel.bAdvanceOutstanding = true;
        }
        if (FPlatformTime::Seconds() >= Deadline)
        {
            return false;
        }
        FPlatformProcess::Sleep(0.0005f);
    }
}

bool UKsa64LiveMissionSubsystem::WaitForActionReceiptForAutomation(
    uint32 ExpectedState,
    double TimeoutSeconds)
{
    const uint64 PriorReceipt = ViewModel.ActionReceiptSequence;
    const double Deadline = FPlatformTime::Seconds() + TimeoutSeconds;
    do
    {
        PollBridge();
        if (ViewModel.ActionReceiptSequence > PriorReceipt)
        {
            return ViewModel.ActionReceiptState == ExpectedState
                && ViewModel.ActionReceiptAccepted != 0;
        }
        if (ViewModel.WorkerState == 3
            || ViewModel.FinalizationState == 3
            || ViewModel.Lifecycle == 6
            || ViewModel.TransportOverflow != 0)
        {
            return false;
        }
        FPlatformProcess::Sleep(0.0005f);
    }
    while (FPlatformTime::Seconds() < Deadline);
    return false;
}

bool UKsa64LiveMissionSubsystem::WaitForCompletionForAutomation(
    double TimeoutSeconds)
{
    const double Deadline = FPlatformTime::Seconds() + TimeoutSeconds;
    do
    {
        PollBridge();
        if (ViewModel.Lifecycle == 5
            && ViewModel.WorkerState == 2
            && ViewModel.FinalizationState == 2
            && bEvidenceSaved)
        {
            return true;
        }
        if (ViewModel.WorkerState == 3
            || ViewModel.FinalizationState == 3
            || ViewModel.Lifecycle == 6
            || ViewModel.TransportOverflow != 0)
        {
            return false;
        }
        FPlatformProcess::Sleep(0.0005f);
    }
    while (FPlatformTime::Seconds() < Deadline);
    return false;
}

bool UKsa64LiveMissionSubsystem::CopyCompletedEvidenceForAutomation(
    TArray<uint8>& OutBytes) const
{
    OutBytes.Reset();
    return bEvidenceSaved
        && !ViewModel.EvidencePath.IsEmpty()
        && FFileHelper::LoadFileToArray(OutBytes, *ViewModel.EvidencePath);
}

void UKsa64LiveMissionSubsystem::CloseForAutomation()
{
    if (bGlobalReplayMode)
    {
        RequestShutdown();
    }
    else if (Bridge.IsValid())
    {
        Bridge->Close();
    }
    ViewModel.bSessionOpen = false;
    ViewModel.bAdvanceOutstanding = false;
    AdvanceTracker.Reset();
}
#endif

bool UKsa64LiveMissionSubsystem::Tick(float DeltaSeconds)
{
    InstallDashboardIfPossible();
    if (bGlobalReplayMode)
    {
        return true;
    }
    if (bPresentationEvidenceMode)
    {
        TickPresentationEvidence(DeltaSeconds);
        return true;
    }
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
        AdvanceTracker.MarkAccepted(
            ViewModel.CommandSequence,
            ViewModel.ReleaseEpoch);
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
    Dashboard->SetVisibility(
        bDashboardRequestedVisible
            ? EVisibility::Visible
            : EVisibility::Collapsed);
    GEngine->GameViewport->AddViewportWidgetContent(Dashboard.ToSharedRef(), 100);
    if (bDashboardRequestedVisible)
    {
        FSlateApplication::Get().SetKeyboardFocus(
            Dashboard,
            EFocusCause::SetDirectly);
    }
    bDashboardInstalled = true;
}

void UKsa64LiveMissionSubsystem::PollBridge()
{
    if (bGlobalReplayMode || !Bridge.IsValid() || !ViewModel.bSessionOpen)
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
    Ksa64OperationsPolicy::PreserveHostEvidenceIdentity(ViewModel, Candidate);
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
    if (!TypedSamples.IsEmpty())
    {
        LastVisualSampleSeconds = FPlatformTime::Seconds();
    }
    for (const FKsa64OperationsReleasePoint& Point : TypedSamples)
    {
        LastObservedRelease = FMath::Max(LastObservedRelease, Point.ReleaseEpoch);
    }
    Bridge->ReadTrajectoryPath(
        EKsa64OperationsTrajectorySource::PlannedReference,
        PlannedReferencePath);
    Bridge->ReadTrajectoryPath(
        EKsa64OperationsTrajectorySource::OnboardEstimate,
        OnboardPredictionPath);
    Bridge->ReadTrajectoryPath(
        EKsa64OperationsTrajectorySource::GroundEstimate,
        GroundPredictionPath);

    LastObservedCommandSequence = ViewModel.CommandSequence;
    if (AdvanceTracker.Observe(
        ViewModel.CommandSequence,
        ViewModel.ReleaseEpoch,
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
    if (ViewModel.FrameIdentity != Previous.FrameIdentity
        || ViewModel.ProcedureStep != Previous.ProcedureStep
        || ViewModel.ProcedureState != Previous.ProcedureState
        || ViewModel.ActionCount != Previous.ActionCount
        || ViewModel.GnssState != Previous.GnssState
        || ViewModel.Lifecycle != Previous.Lifecycle
        || ViewModel.TransportOverflow != Previous.TransportOverflow)
    {
        VisualSnapUntilSeconds = FPlatformTime::Seconds()
            + static_cast<double>(ViewModel.ReleasePeriodMicros) / 1'000'000.0;
    }
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
    AdvanceTracker.MarkAccepted(
        ViewModel.CommandSequence,
        ViewModel.ReleaseEpoch);
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
    if (ViewModel.bSessionOpen && !RequestShutdown())
    {
        ExitAcceptanceFailure();
    }
}

void UKsa64LiveMissionSubsystem::ExitAcceptanceFailure()
{
    if (bAcceptanceExitRequested)
    {
        return;
    }
    if (Bridge.IsValid() && ViewModel.bSessionOpen)
    {
        Bridge->Close();
    }
    ViewModel.bSessionOpen = false;
    ViewModel.bAdvanceOutstanding = false;
    AdvanceTracker.Reset();
    bAcceptanceExitRequested = true;
    UE_LOG(LogKsa64Operations, Error, TEXT("KSA64_PHASE12B_ACCEPTANCE_FAIL: %s"), *AcceptanceFailureReason);
    FPlatformMisc::RequestExitWithStatus(
        false,
        1,
        TEXT("Phase12B acceptance failure"));
}

void UKsa64LiveMissionSubsystem::TickAcceptance()
{
    if (bAcceptanceExitRequested)
    {
        return;
    }
    if (FPlatformTime::Seconds() - AcceptanceStartedSeconds > 180.0
        && !bAcceptanceSlowWarningEmitted)
    {
        bAcceptanceSlowWarningEmitted = true;
        UE_LOG(
            LogKsa64Operations,
            Warning,
            TEXT("KSA64_PHASE12B_ACCEPTANCE_SLOW: still progressing; the run will not be terminated for duration alone"));
    }
    if (bAcceptanceFailed)
    {
        if (!ViewModel.bSessionOpen || ViewModel.WorkerState == 2 || ViewModel.WorkerState == 3)
        {
            ExitAcceptanceFailure();
        }
        return;
    }
    if (!Bridge.IsValid())
    {
        FailAcceptance(TEXT("operations adapter disappeared"));
        return;
    }
    if (ViewModel.WorkerState == 3
        || ViewModel.FinalizationState == 3
        || ViewModel.Lifecycle == 6
        || ViewModel.TransportOverflow != 0)
    {
        FailAcceptance(FString::Printf(
            TEXT("mission entered a proven failure state: worker=%u finalization=%u lifecycle=%u overflow=%u"),
            ViewModel.WorkerState,
            ViewModel.FinalizationState,
            ViewModel.Lifecycle,
            ViewModel.TransportOverflow));
        return;
    }
    if (!ViewModel.bSessionOpen && AcceptancePhase < 10)
    {
        FailAcceptance(TEXT("mission session closed before accepted evidence completion"));
        return;
    }

    switch (AcceptancePhase)
    {
    case 1:
        if (!QueueAcceptanceAdvance(6'080)) return;
        if (ViewModel.ReleaseEpoch == 6'080 && !AdvanceTracker.IsOutstanding())
        {
            if (Bridge->ReviewAction() != EKsa64OperationsAdapterResult::Ok)
            {
                FailAcceptance(TEXT("ground update review failed at release 6080"));
                return;
            }
            AcceptanceReceiptSequenceBeforeCommand = ViewModel.ActionReceiptSequence;
            AcceptanceExpectedProposalIdentity = ViewModel.ActionProposalIdentity;
            if (Bridge->StageAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("ground update stage failed at release 6080"));
                return;
            }
            AcceptancePhase = 2;
        }
        break;
    case 2:
        if (ViewModel.ActionReceiptSequence > AcceptanceReceiptSequenceBeforeCommand)
        {
            if (ViewModel.ActionProposalIdentity != AcceptanceExpectedProposalIdentity
                || ViewModel.ActionReceiptState != 1
                || ViewModel.ActionReceiptAccepted == 0)
            {
                FailAcceptance(TEXT("ground update stage receipt was rejected or mismatched"));
                return;
            }
            AcceptancePhase = 3;
        }
        break;
    case 3:
        if (!QueueAcceptanceAdvance(6'240)) return;
        if (ViewModel.ReleaseEpoch == 6'240 && !AdvanceTracker.IsOutstanding())
        {
            AcceptanceReceiptSequenceBeforeCommand = ViewModel.ActionReceiptSequence;
            AcceptanceExpectedProposalIdentity = ViewModel.ActionProposalIdentity;
            if (Bridge->CommitAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("ground update commit failed at release 6240"));
                return;
            }
            AcceptancePhase = 4;
        }
        break;
    case 4:
        if (ViewModel.ActionReceiptSequence > AcceptanceReceiptSequenceBeforeCommand)
        {
            if (ViewModel.ActionProposalIdentity != AcceptanceExpectedProposalIdentity
                || ViewModel.ActionReceiptState != 2
                || ViewModel.ActionReceiptAccepted == 0)
            {
                FailAcceptance(TEXT("ground update commit receipt was rejected or mismatched"));
                return;
            }
            AcceptancePhase = 5;
        }
        break;
    case 5:
        if (!QueueAcceptanceAdvance(6'560)) return;
        if (ViewModel.ReleaseEpoch == 6'560 && !AdvanceTracker.IsOutstanding())
        {
            if (Bridge->ReviewAction() != EKsa64OperationsAdapterResult::Ok)
            {
                FailAcceptance(TEXT("branch review failed at release 6560"));
                return;
            }
            AcceptanceReceiptSequenceBeforeCommand = ViewModel.ActionReceiptSequence;
            AcceptanceExpectedProposalIdentity = ViewModel.ActionProposalIdentity;
            if (Bridge->StageAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("branch stage failed at release 6560"));
                return;
            }
            AcceptancePhase = 6;
        }
        break;
    case 6:
        if (ViewModel.ActionReceiptSequence > AcceptanceReceiptSequenceBeforeCommand)
        {
            if (ViewModel.ActionProposalIdentity != AcceptanceExpectedProposalIdentity
                || ViewModel.ActionReceiptState != 1
                || ViewModel.ActionReceiptAccepted == 0)
            {
                FailAcceptance(TEXT("branch stage receipt was rejected or mismatched"));
                return;
            }
            AcceptancePhase = 7;
        }
        break;
    case 7:
        if (!QueueAcceptanceAdvance(6'720)) return;
        if (ViewModel.ReleaseEpoch == 6'720 && !AdvanceTracker.IsOutstanding())
        {
            AcceptanceReceiptSequenceBeforeCommand = ViewModel.ActionReceiptSequence;
            AcceptanceExpectedProposalIdentity = ViewModel.ActionProposalIdentity;
            if (Bridge->CommitAction() != EKsa64OperationsAdapterResult::Queued)
            {
                FailAcceptance(TEXT("branch commit failed at release 6720"));
                return;
            }
            AcceptancePhase = 8;
        }
        break;
    case 8:
        if (ViewModel.ActionReceiptSequence > AcceptanceReceiptSequenceBeforeCommand)
        {
            if (ViewModel.ActionProposalIdentity != AcceptanceExpectedProposalIdentity
                || ViewModel.ActionReceiptState != 2
                || ViewModel.ActionReceiptAccepted == 0)
            {
                FailAcceptance(TEXT("branch commit receipt was rejected or mismatched"));
                return;
            }
            AcceptancePhase = 9;
        }
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
            FPlatformMisc::RequestExitWithStatus(
                false,
                0,
                TEXT("Phase12B acceptance complete"));
        }
        // Lifecycle completion is published before the worker seals and
        // verifies the KSB11 bundle. That bounded interval is an expected
        // asynchronous finalization state, not failure. PollBridge saves and
        // closes the worker once FinalizationState becomes ready; proven
        // worker/finalization failures are rejected above.
        break;
    default:
        break;
    }
}

bool UKsa64LiveMissionSubsystem::QueuePresentationEvidenceAdvance(uint32 TargetRelease)
{
    if (ViewModel.ReleaseEpoch >= TargetRelease)
    {
        if (ViewModel.ReleaseEpoch != TargetRelease)
        {
            FailPresentationEvidence(FString::Printf(
                TEXT("presentation advance crossed release %u and reached %u"),
                TargetRelease,
                ViewModel.ReleaseEpoch));
            return false;
        }
        return true;
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
        FailPresentationEvidence(FString::Printf(
            TEXT("presentation advance to release %u failed with result %u"),
            TargetRelease,
            static_cast<uint32>(Result)));
        return false;
    }
    AdvanceTracker.MarkAccepted(
        ViewModel.CommandSequence,
        ViewModel.ReleaseEpoch);
    ViewModel.bAdvanceOutstanding = true;
    return true;
}

bool UKsa64LiveMissionSubsystem::WritePresentationSemanticAndRequestScreenshot()
{
    if (ViewModel.ReleaseEpoch != PresentationCaptureRelease
        || !ViewModel.bTruthFiltered
        || ViewModel.ActionProposalIdentity == 0
        || ViewModel.TransportOverflow != 0
        || !ViewModel.bObservationComplete)
    {
        FailPresentationEvidence(FString::Printf(
            TEXT("capture state is not accepted: release=%u truth_filtered=%u proposal=%08X overflow=%u complete=%u"),
            ViewModel.ReleaseEpoch,
            ViewModel.bTruthFiltered ? 1u : 0u,
            ViewModel.ActionProposalIdentity,
            ViewModel.TransportOverflow,
            ViewModel.bObservationComplete ? 1u : 0u));
        return false;
    }
    IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
    if (!PlatformFile.CreateDirectoryTree(*PresentationEvidenceDirectory))
    {
        FailPresentationEvidence(TEXT("presentation evidence directory creation failed"));
        return false;
    }
    if (PlatformFile.FileExists(*PresentationScreenshotPath)
        || PlatformFile.FileExists(*PresentationSemanticPath)
        || PlatformFile.FileExists(*PresentationManifestPath))
    {
        FailPresentationEvidence(TEXT("presentation evidence output path is not fresh"));
        return false;
    }
    const FString TemporarySemanticPath = PresentationSemanticPath + TEXT(".tmp");
    PlatformFile.DeleteFile(*TemporarySemanticPath);
    if (!FFileHelper::SaveStringToFile(
            ExportSemanticStateJson(),
            *TemporarySemanticPath,
            FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM)
        || !PlatformFile.MoveFile(*PresentationSemanticPath, *TemporarySemanticPath))
    {
        PlatformFile.DeleteFile(*TemporarySemanticPath);
        FailPresentationEvidence(TEXT("semantic evidence atomic write failed"));
        return false;
    }
    if (FScreenshotRequest::IsScreenshotRequested())
    {
        FailPresentationEvidence(TEXT("another screenshot request is already active"));
        return false;
    }
    bPresentationScreenshotProcessed = false;
    PresentationScreenshotWaitFrames = 0;
    if (PresentationScreenshotProcessedHandle.IsValid())
    {
        FScreenshotRequest::OnScreenshotRequestProcessed().Remove(PresentationScreenshotProcessedHandle);
    }
    PresentationScreenshotProcessedHandle =
        FScreenshotRequest::OnScreenshotRequestProcessed().AddUObject(
            this,
            &UKsa64LiveMissionSubsystem::OnPresentationScreenshotProcessed);
    FScreenshotRequest::RequestScreenshot(
        PresentationScreenshotPath,
        true,
        false,
        false,
        FIntRect(),
        true);
    UE_LOG(
        LogKsa64Operations,
        Display,
        TEXT("KSA64_PHASE12B_PRESENTATION_CAPTURE_REQUESTED release=6080 path=%s"),
        *PresentationScreenshotPath);
    return true;
}

void UKsa64LiveMissionSubsystem::OnPresentationScreenshotProcessed()
{
    bPresentationScreenshotProcessed = true;
    if (PresentationScreenshotProcessedHandle.IsValid())
    {
        FScreenshotRequest::OnScreenshotRequestProcessed().Remove(PresentationScreenshotProcessedHandle);
        PresentationScreenshotProcessedHandle.Reset();
    }
}

bool UKsa64LiveMissionSubsystem::ValidatePresentationScreenshot(
    int32& OutWidth,
    int32& OutHeight)
{
    OutWidth = 0;
    OutHeight = 0;
    PresentationScreenshotSampledPixels = 0;
    PresentationScreenshotDistinctColorBuckets = 0;
    PresentationScreenshotLuminanceRange = 0;
    PresentationScreenshotNonDarkSamples = 0;

    FImage Decoded;
    if (!FImageUtils::LoadImage(*PresentationScreenshotPath, Decoded)
        || Decoded.SizeX <= 0
        || Decoded.SizeY <= 0
        || Decoded.NumSlices != 1)
    {
        return false;
    }
    OutWidth = Decoded.SizeX;
    OutHeight = Decoded.SizeY;
    Decoded.ChangeFormat(ERawImageFormat::BGRA8, EGammaSpace::sRGB);
    const TArrayView64<const FColor> Pixels = Decoded.AsBGRA8();
    if (Pixels.Num() != static_cast<int64>(OutWidth) * static_cast<int64>(OutHeight))
    {
        return false;
    }

    const int32 StepX = FMath::Max(1, OutWidth / 64);
    const int32 StepY = FMath::Max(1, OutHeight / 36);
    int32 MinimumLuminance = 255;
    int32 MaximumLuminance = 0;
    TSet<uint16> ColorBuckets;
    for (int32 Y = 0; Y < OutHeight; Y += StepY)
    {
        for (int32 X = 0; X < OutWidth; X += StepX)
        {
            const FColor Pixel = Pixels[static_cast<int64>(Y) * OutWidth + X];
            const int32 Luminance =
                (54 * static_cast<int32>(Pixel.R)
                    + 183 * static_cast<int32>(Pixel.G)
                    + 19 * static_cast<int32>(Pixel.B))
                >> 8;
            MinimumLuminance = FMath::Min(MinimumLuminance, Luminance);
            MaximumLuminance = FMath::Max(MaximumLuminance, Luminance);
            if (Luminance > 16)
            {
                ++PresentationScreenshotNonDarkSamples;
            }
            const uint16 Bucket = static_cast<uint16>(
                (static_cast<uint16>(Pixel.R >> 5) << 6)
                | (static_cast<uint16>(Pixel.G >> 5) << 3)
                | static_cast<uint16>(Pixel.B >> 5));
            ColorBuckets.Add(Bucket);
            ++PresentationScreenshotSampledPixels;
        }
    }
    PresentationScreenshotDistinctColorBuckets = ColorBuckets.Num();
    PresentationScreenshotLuminanceRange = MaximumLuminance - MinimumLuminance;
    return PresentationScreenshotSampledPixels > 0
        && PresentationScreenshotLuminanceRange >= PresentationMinimumLuminanceRange
        && PresentationScreenshotDistinctColorBuckets >= PresentationMinimumColorBuckets
        && PresentationScreenshotNonDarkSamples
            >= FMath::Max(1, PresentationScreenshotSampledPixels / 100);
}

bool UKsa64LiveMissionSubsystem::WritePresentationManifest(
    bool bPassed,
    const FString& FailureReason)
{
    IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
    if (!PlatformFile.CreateDirectoryTree(*PresentationEvidenceDirectory))
    {
        return false;
    }
    FString Output;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Output);
    Writer->WriteObjectStart();
    Writer->WriteValue(TEXT("schema"), TEXT("ksa64.phase12b.presentation-evidence.v1"));
    Writer->WriteValue(TEXT("pass"), bPassed);
    Writer->WriteValue(TEXT("failure_reason"), FailureReason);
    Writer->WriteValue(TEXT("scenario"), TEXT("ksa-g10r.operations/gnss-loss"));
    Writer->WriteValue(TEXT("role"), TEXT("guided-operator"));
    Writer->WriteValue(TEXT("truth_filtered"), ViewModel.bTruthFiltered);
    Writer->WriteValue(TEXT("async_shutdown_complete"), !ViewModel.bSessionOpen);
    Writer->WriteValue(TEXT("screenshot_release_epoch"), PresentationCaptureRelease);
    Writer->WriteValue(TEXT("terminal_release_epoch"), ViewModel.ReleaseEpoch);
    Writer->WriteObjectStart(TEXT("screenshot"));
    Writer->WriteValue(TEXT("file"), FPaths::GetCleanFilename(PresentationScreenshotPath));
    Writer->WriteValue(TEXT("width"), PresentationWidth);
    Writer->WriteValue(TEXT("height"), PresentationHeight);
    Writer->WriteValue(TEXT("slate_inclusive"), true);
    Writer->WriteValue(
        TEXT("real_rhi"),
        !PresentationRhiName.IsEmpty()
            && PresentationRhiName.Contains(TEXT("D3D12"), ESearchCase::IgnoreCase));
    Writer->WriteValue(TEXT("rhi_name"), PresentationRhiName);
    Writer->WriteValue(TEXT("fully_decoded"), PresentationScreenshotSampledPixels > 0);
    Writer->WriteValue(TEXT("sampled_pixels"), PresentationScreenshotSampledPixels);
    Writer->WriteValue(TEXT("distinct_color_buckets"), PresentationScreenshotDistinctColorBuckets);
    Writer->WriteValue(TEXT("luminance_range"), PresentationScreenshotLuminanceRange);
    Writer->WriteValue(TEXT("non_dark_samples"), PresentationScreenshotNonDarkSamples);
    Writer->WriteValue(TEXT("minimum_color_buckets"), PresentationMinimumColorBuckets);
    Writer->WriteValue(TEXT("minimum_luminance_range"), PresentationMinimumLuminanceRange);
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("semantic"));
    Writer->WriteValue(TEXT("file"), FPaths::GetCleanFilename(PresentationSemanticPath));
    Writer->WriteValue(TEXT("schema"), TEXT("ksa64.mission-foundry-semantic-state.v1"));
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("trajectory"));
    Writer->WriteValue(TEXT("planned_reference_points"), PlannedReferencePath.Num());
    Writer->WriteValue(TEXT("onboard_estimate_points"), OnboardPredictionPath.Num());
    Writer->WriteValue(TEXT("ground_estimate_points"), GroundPredictionPath.Num());
    Writer->WriteValue(TEXT("observed_points"), ReleaseHistory.Num());
    Writer->WriteValue(TEXT("altitude_plot"), true);
    Writer->WriteValue(TEXT("ground_track_plot"), true);
    Writer->WriteValue(
        TEXT("display_mode"),
        GetDisplayMode() == EKsa64OperationsDisplayMode::Exact
            ? TEXT("exact")
            : TEXT("smooth"));
    Writer->WriteObjectEnd();
    Writer->WriteObjectStart(TEXT("performance"));
    Writer->WriteValue(TEXT("scope"), TEXT("PollBridge+typed-drains+prediction-path+advance-enqueue"));
    Writer->WriteValue(TEXT("refresh_hz"), 60);
    Writer->WriteValue(TEXT("cadence"), TEXT("simulated-fixed-step"));
    Writer->WriteValue(TEXT("fixed_timestep"), true);
    Writer->WriteValue(TEXT("fixed_delta_seconds"), TEXT("0.016666666666666667"));
    Writer->WriteValue(TEXT("logical_seconds"), 10);
    Writer->WriteValue(TEXT("warmup_frames"), PresentationWarmupFrameCount);
    Writer->WriteValue(TEXT("measured_frames"), PresentationEvidenceMeasuredFrames);
    Writer->WriteValue(TEXT("start_release"), PresentationPerformanceStartRelease);
    Writer->WriteValue(TEXT("end_release"), PresentationPerformanceEndRelease);
    Writer->WriteValue(
        TEXT("release_delta"),
        PresentationPerformanceEndRelease - PresentationPerformanceStartRelease);
    Writer->WriteValue(TEXT("expected_release_delta"), PresentationExpectedReleaseDelta);
    Writer->WriteValue(TEXT("start_publication"), PresentationPerformanceStartPublication);
    Writer->WriteValue(TEXT("end_publication"), PresentationPerformanceEndPublication);
    Writer->WriteValue(TEXT("commands_pending"), ViewModel.CommandsPending);
    Writer->WriteValue(TEXT("transport_overflow"), ViewModel.TransportOverflow);
    Writer->WriteValue(TEXT("observation_complete"), ViewModel.bObservationComplete);
    Writer->WriteValue(TEXT("advance_outstanding"), AdvanceTracker.IsOutstanding());
    Writer->WriteValue(TEXT("percentile_method"), TEXT("nearest-rank"));
    Writer->WriteValue(TEXT("p99_ns"), PresentationEvidenceP99Nanoseconds);
    Writer->WriteValue(TEXT("max_ns"), PresentationEvidenceMaximumNanoseconds);
    Writer->WriteValue(TEXT("p99_limit_ns_exclusive"), PresentationP99LimitNanoseconds);
    Writer->WriteValue(TEXT("max_limit_ns_exclusive"), PresentationMaximumLimitNanoseconds);
    Writer->WriteValue(
        TEXT("pass"),
        PresentationEvidenceP99Nanoseconds >= 0
            && PresentationEvidenceP99Nanoseconds < PresentationP99LimitNanoseconds
            && PresentationEvidenceMaximumNanoseconds >= 0
            && PresentationEvidenceMaximumNanoseconds < PresentationMaximumLimitNanoseconds
            && PresentationPerformanceEndRelease - PresentationPerformanceStartRelease
                == PresentationExpectedReleaseDelta
            && ViewModel.CommandsPending == 0
            && ViewModel.TransportOverflow == 0
            && ViewModel.bObservationComplete
            && !AdvanceTracker.IsOutstanding());
    Writer->WriteObjectEnd();
    Writer->WriteObjectEnd();
    Writer->Close();

    const FString TemporaryPath = PresentationManifestPath + TEXT(".tmp");
    PlatformFile.DeleteFile(*TemporaryPath);
    if (!FFileHelper::SaveStringToFile(
            Output,
            *TemporaryPath,
            FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM)
        || !PlatformFile.MoveFile(*PresentationManifestPath, *TemporaryPath))
    {
        PlatformFile.DeleteFile(*TemporaryPath);
        return false;
    }
    return true;
}

void UKsa64LiveMissionSubsystem::FailPresentationEvidence(const FString& Reason)
{
    if (bPresentationEvidenceFailed)
    {
        return;
    }
    bPresentationEvidenceFailed = true;
    PresentationEvidenceFailureReason = Reason;
    PresentationEvidencePhase = 250;
    UE_LOG(
        LogKsa64Operations,
        Error,
        TEXT("KSA64_PHASE12B_PRESENTATION_EVIDENCE_FAIL_PENDING: %s"),
        *Reason);
    if (ViewModel.bSessionOpen && !RequestShutdown())
    {
        ExitPresentationEvidenceFailure();
    }
}

void UKsa64LiveMissionSubsystem::ExitPresentationEvidenceFailure()
{
    if (bPresentationEvidenceExitRequested)
    {
        return;
    }
    if (Bridge.IsValid() && ViewModel.bSessionOpen)
    {
        Bridge->Close();
    }
    ViewModel.bSessionOpen = false;
    ViewModel.bAdvanceOutstanding = false;
    AdvanceTracker.Reset();
    WritePresentationManifest(false, PresentationEvidenceFailureReason);
    bPresentationEvidenceExitRequested = true;
    UE_LOG(
        LogKsa64Operations,
        Error,
        TEXT("KSA64_PHASE12B_PRESENTATION_EVIDENCE_FAIL: %s"),
        *PresentationEvidenceFailureReason);
    FPlatformMisc::RequestExitWithStatus(
        false,
        1,
        TEXT("Phase12B presentation evidence failure"));
}

void UKsa64LiveMissionSubsystem::TickPresentationEvidence(float DeltaSeconds)
{
    if (bPresentationEvidenceExitRequested)
    {
        return;
    }
    if (bPresentationEvidenceFailed)
    {
        PollBridge();
        if (!ViewModel.bSessionOpen || ViewModel.WorkerState == 2 || ViewModel.WorkerState == 3)
        {
            ExitPresentationEvidenceFailure();
        }
        return;
    }
    if (PresentationRhiName.IsEmpty())
    {
        if (!FApp::CanEverRender() || GDynamicRHI == nullptr)
        {
            FailPresentationEvidence(TEXT("presentation evidence requires an initialized real RHI"));
            return;
        }
        PresentationRhiName = GDynamicRHI->GetName();
        if (!PresentationRhiName.Contains(TEXT("D3D12"), ESearchCase::IgnoreCase))
        {
            FailPresentationEvidence(FString::Printf(
                TEXT("presentation evidence requires D3D12, found %s"),
                *PresentationRhiName));
            return;
        }
    }
    const double ExpectedFixedDeltaSeconds = 1.0 / 60.0;
    if (!FApp::UseFixedTimeStep()
        || !FMath::IsNearlyEqual(FApp::GetFixedDeltaTime(), ExpectedFixedDeltaSeconds, 1.0e-9)
        || !FMath::IsNearlyEqual(static_cast<double>(DeltaSeconds), ExpectedFixedDeltaSeconds, 1.0e-6))
    {
        FailPresentationEvidence(FString::Printf(
            TEXT("presentation evidence requires a verified 60 Hz fixed timestep: enabled=%u fixed=%.12f tick=%.12f"),
            FApp::UseFixedTimeStep() ? 1u : 0u,
            FApp::GetFixedDeltaTime(),
            static_cast<double>(DeltaSeconds)));
        return;
    }

    const bool bPerformanceFrame = PresentationEvidencePhase == 4
        || (PresentationEvidencePhase == 5
            && !bPresentationMeasurementFramesComplete);
    const uint64 ServiceStartCycles = bPerformanceFrame ? FPlatformTime::Cycles64() : 0;
    PollBridge();

    if (FPlatformTime::Seconds() - PresentationEvidenceStartedSeconds > 180.0
        && !bPresentationEvidenceSlowWarningEmitted)
    {
        bPresentationEvidenceSlowWarningEmitted = true;
        UE_LOG(
            LogKsa64Operations,
            Warning,
            TEXT("KSA64_PHASE12B_PRESENTATION_EVIDENCE_SLOW: still progressing; the run will not be terminated for duration alone"));
    }
    if (!Bridge.IsValid())
    {
        FailPresentationEvidence(TEXT("operations adapter disappeared"));
        return;
    }
    if (ViewModel.WorkerState == 3 || ViewModel.FinalizationState == 3 || ViewModel.Lifecycle == 6)
    {
        FailPresentationEvidence(FString::Printf(
            TEXT("mission worker entered a proven failure state: worker=%u finalization=%u lifecycle=%u"),
            ViewModel.WorkerState,
            ViewModel.FinalizationState,
            ViewModel.Lifecycle));
        return;
    }

    switch (PresentationEvidencePhase)
    {
    case 1:
        if (!QueuePresentationEvidenceAdvance(PresentationCaptureRelease))
        {
            return;
        }
        if (ViewModel.ReleaseEpoch == PresentationCaptureRelease
            && !AdvanceTracker.IsOutstanding())
        {
            ViewModel.PresentationPace = EKsa64OperationsPace::Paused;
            PacingController.Reset();
            PresentationEvidenceWarmupFrames = 0;
            PresentationDashboardWaitFrames = 0;
            PresentationEvidencePhase = 2;
        }
        break;
    case 2:
        if (!bDashboardInstalled || GEngine == nullptr || GEngine->GameViewport == nullptr)
        {
            ++PresentationDashboardWaitFrames;
            if (PresentationDashboardWaitFrames >= PresentationDashboardReadyFrameLimit)
            {
                FailPresentationEvidence(TEXT(
                    "real-RHI viewport/dashboard was not installed within the bounded engine-readiness window"));
            }
            break;
        }
        ++PresentationEvidenceWarmupFrames;
        if (PresentationEvidenceWarmupFrames >= 3
            && WritePresentationSemanticAndRequestScreenshot())
        {
            PresentationEvidencePhase = 3;
        }
        break;
    case 3:
    {
        if (!bPresentationScreenshotProcessed)
        {
            ++PresentationScreenshotWaitFrames;
            if (PresentationScreenshotWaitFrames >= PresentationDashboardReadyFrameLimit)
            {
                FailPresentationEvidence(TEXT(
                    "screenshot request was not processed within the bounded render-readiness window"));
            }
            break;
        }
        IPlatformFile& PlatformFile = FPlatformFileManager::Get().GetPlatformFile();
        const int64 ScreenshotBytes = PlatformFile.FileSize(*PresentationScreenshotPath);
        if (ScreenshotBytes < 24)
        {
            FailPresentationEvidence(TEXT(
                "screenshot request completed without a complete output file"));
            return;
        }
        int32 Width = 0;
        int32 Height = 0;
        if (!ValidatePresentationScreenshot(Width, Height))
        {
            FailPresentationEvidence(TEXT(
                "captured screenshot did not fully decode as a nonblank dashboard PNG"));
            return;
        }
        if (Width != PresentationWidth || Height != PresentationHeight)
        {
            FailPresentationEvidence(FString::Printf(
                TEXT("captured screenshot dimensions are %dx%d instead of 1920x1080"),
                Width,
                Height));
            return;
        }
        ViewModel.PresentationPace = EKsa64OperationsPace::Realtime;
        PacingController.Reset();
        PresentationEvidenceWarmupFrames = 0;
        bPresentationMeasurementFramesComplete = false;
        PresentationEvidencePhase = 4;
        break;
    }
    case 4:
    case 5:
    {
        const bool bRunnable = ViewModel.bSessionOpen
            && ViewModel.Lifecycle != 5
            && ViewModel.Lifecycle != 6
            && !ViewModel.bShutdownRequested;
        if (!bRunnable)
        {
            FailPresentationEvidence(TEXT("mission became unrunnable during the presentation workload"));
            return;
        }

        if (PresentationEvidencePhase == 4
            && PresentationEvidenceWarmupFrames >= PresentationWarmupFrameCount)
        {
            if (AdvanceTracker.IsOutstanding())
            {
                break;
            }
            PacingController.Reset();
            PresentationPerformanceStartRelease = ViewModel.ReleaseEpoch;
            PresentationPerformanceStartPublication = ViewModel.CommandSequence;
            PresentationEvidenceServiceNanoseconds.Reset();
            PresentationEvidenceMeasuredFrames = 0;
            bPresentationMeasurementFramesComplete = false;
            PresentationEvidencePhase = 5;
            break;
        }

        if (PresentationEvidencePhase == 5
            && bPresentationMeasurementFramesComplete)
        {
            if (AdvanceTracker.IsOutstanding())
            {
                break;
            }
            const uint32 RemainingReleases = PacingController.ReleasesDue(
                EKsa64OperationsPace::Realtime,
                ViewModel.ReleasePeriodMicros,
                bRunnable,
                false);
            if (RemainingReleases > 0)
            {
                const EKsa64OperationsAdapterResult FlushResult =
                    Bridge->AdvanceReleases(RemainingReleases);
                HandleAdapterResult(FlushResult, TEXT("presentation timing flush"));
                if (FlushResult != EKsa64OperationsAdapterResult::Ok
                    && FlushResult != EKsa64OperationsAdapterResult::Queued)
                {
                    FailPresentationEvidence(FString::Printf(
                        TEXT("presentation timing flush failed with result %u"),
                        static_cast<uint32>(FlushResult)));
                    return;
                }
                PacingController.CommitAcceptedAdvance(
                    RemainingReleases,
                    ViewModel.ReleasePeriodMicros,
                    EKsa64OperationsPace::Realtime);
                AdvanceTracker.MarkAccepted(
                    ViewModel.CommandSequence,
                    ViewModel.ReleaseEpoch);
                ViewModel.bAdvanceOutstanding = true;
                break;
            }
            PresentationEvidencePhase = 6;
            break;
        }

        // Every consecutive measured presentation frame is timed, including
        // frames whose only work is draining a previously queued advance.
        PacingController.Accumulate(DeltaSeconds, EKsa64OperationsPace::Realtime);
        if (!AdvanceTracker.IsOutstanding())
        {
            const uint32 Releases = PacingController.ReleasesDue(
                EKsa64OperationsPace::Realtime,
                ViewModel.ReleasePeriodMicros,
                bRunnable,
                false);
            if (Releases > 0)
            {
                const EKsa64OperationsAdapterResult Result =
                    Bridge->AdvanceReleases(Releases);
                HandleAdapterResult(Result, TEXT("presentation timing advance"));
                if (Result != EKsa64OperationsAdapterResult::Ok
                    && Result != EKsa64OperationsAdapterResult::Queued)
                {
                    FailPresentationEvidence(FString::Printf(
                        TEXT("presentation timing advance failed with result %u"),
                        static_cast<uint32>(Result)));
                    return;
                }
                PacingController.CommitAcceptedAdvance(
                    Releases,
                    ViewModel.ReleasePeriodMicros,
                    EKsa64OperationsPace::Realtime);
                AdvanceTracker.MarkAccepted(
                    ViewModel.CommandSequence,
                    ViewModel.ReleaseEpoch);
                ViewModel.bAdvanceOutstanding = true;
            }
        }

        const uint64 ServiceEndCycles = FPlatformTime::Cycles64();
        const int64 ElapsedNanoseconds = FMath::Max<int64>(
            0,
            FMath::RoundToInt64(
                static_cast<double>(ServiceEndCycles - ServiceStartCycles)
                * FPlatformTime::GetSecondsPerCycle64()
                * 1'000'000'000.0));
        if (PresentationEvidencePhase == 4)
        {
            ++PresentationEvidenceWarmupFrames;
        }
        else
        {
            PresentationEvidenceServiceNanoseconds.Add(ElapsedNanoseconds);
            PresentationEvidenceMeasuredFrames = static_cast<uint32>(
                PresentationEvidenceServiceNanoseconds.Num());
            bPresentationMeasurementFramesComplete =
                PresentationEvidenceMeasuredFrames >= PresentationMeasuredFrameCount;
        }
        break;
    }
    case 6:
        if (AdvanceTracker.IsOutstanding())
        {
            break;
        }
        PresentationPerformanceEndRelease = ViewModel.ReleaseEpoch;
        PresentationPerformanceEndPublication = ViewModel.CommandSequence;
        if (PresentationPerformanceEndRelease
                != PresentationPerformanceStartRelease + PresentationExpectedReleaseDelta
            || PresentationPerformanceEndPublication <= PresentationPerformanceStartPublication
            || ViewModel.CommandsPending != 0
            || ViewModel.TransportOverflow != 0
            || !ViewModel.bObservationComplete)
        {
            FailPresentationEvidence(FString::Printf(
                TEXT("presentation workload incomplete: releases=%u expected=%u publications=%llu->%llu pending=%u overflow=%u observation_complete=%u"),
                PresentationPerformanceEndRelease - PresentationPerformanceStartRelease,
                PresentationExpectedReleaseDelta,
                static_cast<unsigned long long>(PresentationPerformanceStartPublication),
                static_cast<unsigned long long>(PresentationPerformanceEndPublication),
                ViewModel.CommandsPending,
                ViewModel.TransportOverflow,
                ViewModel.bObservationComplete ? 1u : 0u));
            return;
        }
        PresentationEvidenceP99Nanoseconds =
            Ksa64OperationsPolicy::NearestRankP99Nanoseconds(
                PresentationEvidenceServiceNanoseconds);
        PresentationEvidenceMaximumNanoseconds = 0;
        for (const int64 Sample : PresentationEvidenceServiceNanoseconds)
        {
            PresentationEvidenceMaximumNanoseconds = FMath::Max(
                PresentationEvidenceMaximumNanoseconds,
                Sample);
        }
        ViewModel.PresentationPace = EKsa64OperationsPace::Paused;
        PacingController.Reset();
        if (PresentationEvidenceServiceNanoseconds.Num() != PresentationMeasuredFrameCount
            || PresentationEvidenceP99Nanoseconds < 0
            || PresentationEvidenceP99Nanoseconds >= PresentationP99LimitNanoseconds
            || PresentationEvidenceMaximumNanoseconds < 0
            || PresentationEvidenceMaximumNanoseconds >= PresentationMaximumLimitNanoseconds)
        {
            FailPresentationEvidence(FString::Printf(
                TEXT("bridge service timing exceeded limits or sample count: samples=%d p99=%lld max=%lld"),
                PresentationEvidenceServiceNanoseconds.Num(),
                static_cast<long long>(PresentationEvidenceP99Nanoseconds),
                static_cast<long long>(PresentationEvidenceMaximumNanoseconds)));
            return;
        }
        if (!RequestShutdown())
        {
            FailPresentationEvidence(TEXT("presentation worker shutdown did not queue"));
            return;
        }
        PresentationEvidencePhase = 7;
        break;
    case 7:
        if (ViewModel.WorkerState == 3 || ViewModel.FinalizationState == 3)
        {
            FailPresentationEvidence(TEXT("presentation worker failed during graceful shutdown"));
            return;
        }
        if (!ViewModel.bSessionOpen)
        {
            if (!WritePresentationManifest(true))
            {
                FailPresentationEvidence(TEXT(
                    "presentation manifest atomic write failed after shutdown"));
                return;
            }
            bPresentationEvidenceExitRequested = true;
            UE_LOG(
                LogKsa64Operations,
                Display,
                TEXT("KSA64_PHASE12B_PRESENTATION_EVIDENCE_PASS release=6080 width=1920 height=1080 frames=600 p99_ns=%lld max_ns=%lld"),
                static_cast<long long>(PresentationEvidenceP99Nanoseconds),
                static_cast<long long>(PresentationEvidenceMaximumNanoseconds));
            FPlatformMisc::RequestExitWithStatus(
                false,
                0,
                TEXT("Phase12B presentation evidence complete"));
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
    if (bSafeToClose
        && !bRetainCompletedGlobalDisplaySession
        && Bridge.IsValid())
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
