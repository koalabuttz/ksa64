#include "Ksa64BridgeModule.h"
#include "Ksa64BridgeTypedValidation.h"

#if WITH_DEV_AUTOMATION_TESTS

#include "Dom/JsonObject.h"
#include "HAL/FileManager.h"
#include "HAL/PlatformProcess.h"
#include "Misc/AutomationTest.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"

namespace
{
bool WriteModifiedManifest(
    const FString& Source,
    const FString& Destination,
    TFunctionRef<void(FJsonObject&)> Change)
{
    FString Text;
    TSharedPtr<FJsonObject> Object;
    if (!FFileHelper::LoadFileToString(Text, *Source))
    {
        return false;
    }
    const TSharedRef<TJsonReader<>> Reader = TJsonReaderFactory<>::Create(Text);
    if (!FJsonSerializer::Deserialize(Reader, Object) || !Object.IsValid())
    {
        return false;
    }
    Change(*Object);
    FString Output;
    const TSharedRef<TJsonWriter<>> Writer = TJsonWriterFactory<>::Create(&Output);
    return FJsonSerializer::Serialize(Object.ToSharedRef(), Writer)
        && FFileHelper::SaveStringToFile(Output, *Destination);
}

bool WaitForSnapshot(
    FKsa64BridgeModule& Module,
    Ksa64ViewerSnapshot& Snapshot,
    TFunctionRef<bool(const Ksa64ViewerSnapshot&)> Predicate)
{
    const double Deadline = FPlatformTime::Seconds() + 5.0;
    do
    {
        const int32 Result = Module.PollSnapshot(Snapshot);
        if (Result == KSA64_VIEWER_OK && Predicate(Snapshot))
        {
            return true;
        }
        if (Result != KSA64_VIEWER_OK
            && Result != KSA64_VIEWER_NO_DATA
            && Result != KSA64_VIEWER_UNCHANGED)
        {
            return false;
        }
        FPlatformProcess::Sleep(0.001f);
    }
    while (FPlatformTime::Seconds() < Deadline);
    return false;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64BridgeManifestTest,
    "KSA64.Phase12A.Bridge.ManifestAndIdentity",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64BridgeManifestTest::RunTest(const FString&)
{
    FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
    if (Module.GetStatus() != EKsa64BridgeStatus::Ready)
    {
        AddError(FString::Printf(TEXT("staged bridge is not ready: %s"), *Module.GetDiagnostic()));
        return false;
    }

    const FKsa64BridgeValidation& Accepted = Module.GetValidation();
    TestEqual(TEXT("ABI version"), Accepted.AbiVersion, KSA64_VIEWER_ABI_VERSION);
    TestEqual(TEXT("build identity"), Accepted.BuildIdentity, KSA64_VIEWER_BUILD_IDENTITY);
    TestEqual(TEXT("catalog count"), Accepted.CatalogCount, 13u);
    TestEqual(
        TEXT("catalog hash"),
        Accepted.CatalogSha256,
        FString(TEXT("b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13")));
    TestTrue(
        TEXT("catalog schema"),
        Module.GetCatalogJson().Contains(TEXT("\"schema\": \"ksa64.product-catalog.v1\"")));

    const FString TempRoot = FPaths::Combine(
        FPaths::ProjectSavedDir(),
        TEXT("Automation"),
        TEXT("Ksa64BridgeManifest"));
    IFileManager::Get().DeleteDirectory(*TempRoot, false, true);
    TestTrue(TEXT("create temporary validation directory"), IFileManager::Get().MakeDirectory(*TempRoot, true));

    const FString DllCopy = FPaths::Combine(TempRoot, FPaths::GetCleanFilename(Accepted.DllPath));
    const FString ManifestCopy =
        FPaths::Combine(TempRoot, FPaths::GetCleanFilename(Accepted.ManifestPath));
    TestTrue(
        TEXT("copy bridge DLL"),
        IFileManager::Get().Copy(*DllCopy, *Accepted.DllPath) == COPY_OK);

    FString Diagnostic;
    FKsa64BridgeValidation Rejected;
    TestTrue(
        TEXT("write ABI mismatch manifest"),
        WriteModifiedManifest(
            Accepted.ManifestPath,
            ManifestCopy,
            [](FJsonObject& Json) { Json.SetNumberField(TEXT("abi_version"), 999); }));
    TestFalse(
        TEXT("ABI mismatch rejected before load"),
        FKsa64BridgeModule::ValidateArtifactManifest(ManifestCopy, Rejected, Diagnostic));
    TestTrue(TEXT("ABI mismatch diagnostic"), Diagnostic.Contains(TEXT("ABI")));

    TestTrue(
        TEXT("write hash mismatch manifest"),
        WriteModifiedManifest(
            Accepted.ManifestPath,
            ManifestCopy,
            [](FJsonObject& Json)
            {
                Json.SetStringField(
                    TEXT("dll_sha256"),
                    TEXT("0000000000000000000000000000000000000000000000000000000000000000"));
            }));
    Diagnostic.Reset();
    TestFalse(
        TEXT("hash mismatch rejected before load"),
        FKsa64BridgeModule::ValidateArtifactManifest(ManifestCopy, Rejected, Diagnostic));
    TestTrue(TEXT("hash mismatch diagnostic"), Diagnostic.Contains(TEXT("SHA-256")));

    IFileManager::Get().DeleteDirectory(*TempRoot, false, true);
    return true;
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64BridgeLifecycleTest,
    "KSA64.Phase12A.Bridge.GuidedLifecycle",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64BridgeLifecycleTest::RunTest(const FString&)
{
    FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
    if (Module.GetStatus() != EKsa64BridgeStatus::Ready)
    {
        AddError(FString::Printf(TEXT("staged bridge is not ready: %s"), *Module.GetDiagnostic()));
        return false;
    }

    TestTrue(TEXT("open accepted guided GNSS-loss session"), Module.StartGuidedGnssLoss());
    Ksa64ViewerSnapshot Initial = {};
    TestTrue(
        TEXT("poll initial role-filtered snapshot"),
        WaitForSnapshot(
            Module,
            Initial,
            [](const Ksa64ViewerSnapshot&) { return true; }));
    TestEqual(TEXT("guided role identity"), Initial.role, 2u);
    TestEqual(TEXT("initial release"), Initial.release_epoch, 0u);
    Ksa64ViewerSnapshot Preserved = Initial;
    TestEqual(
        TEXT("repeat poll reports a healthy unchanged snapshot"),
        Module.PollSnapshot(Preserved),
        KSA64_VIEWER_UNCHANGED);
    TestEqual(TEXT("unchanged poll preserves prior release"), Preserved.release_epoch, Initial.release_epoch);
    TestEqual(TEXT("unchanged poll preserves prior checksum"), Preserved.flight_checksum, Initial.flight_checksum);
    TestEqual(
        TEXT("no fields outside operational ABI"),
        Initial.validity_mask & ~((1ull << 11) - 1ull),
        0ull);

    const uint64 InitialCommand = Initial.command_sequence;
    TestEqual(TEXT("one release queues without blocking"), Module.AdvanceOneRelease(), KSA64_VIEWER_QUEUED);
    Ksa64ViewerSnapshot Advanced = {};
    TestTrue(
        TEXT("one exact release becomes observable"),
        WaitForSnapshot(
            Module,
            Advanced,
            [InitialCommand](const Ksa64ViewerSnapshot& Snapshot)
            {
                return Snapshot.command_sequence > InitialCommand
                    && Snapshot.release_epoch == 1;
            }));
    TestEqual(TEXT("step command completed"), Advanced.command_result, KSA64_VIEWER_OK);
    TestEqual(TEXT("guided role remains immutable"), Advanced.role, 2u);
    TestEqual(
        TEXT("truth-only extension bits remain absent"),
        Advanced.validity_mask & ~((1ull << 11) - 1ull),
        0ull);

    Module.CloseSession();
    TestTrue(
        TEXT("clean shutdown returns bridge to ready"),
        Module.GetStatus() == EKsa64BridgeStatus::Ready);
    Module.CloseSession();
    TestTrue(
        TEXT("clean shutdown is idempotent"),
        Module.GetStatus() == EKsa64BridgeStatus::Ready);
    return true;
}


IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64BridgeTypedOutputValidationTest,
    "KSA64.Phase12B.Bridge.TypedOutputValidation",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64BridgeTypedOutputValidationTest::RunTest(const FString&)
{
    auto InitializeHeader = [](auto& Value)
    {
        Value.abi_version = KSA64_VIEWER_ABI_VERSION;
        Value.struct_size = static_cast<uint32>(sizeof(Value));
    };

    Ksa64ViewerOperationalViewV1 Operational = {};
    InitializeHeader(Operational);
    Operational.validity_mask =
        Ksa64BridgeTypedValidation::ValidMissionTime
        | Ksa64BridgeTypedValidation::ValidNavigation
        | Ksa64BridgeTypedValidation::ValidPrediction
        | Ksa64BridgeTypedValidation::ValidProcedure
        | Ksa64BridgeTypedValidation::ValidGnss;
    Operational.scenario_identity = KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS;
    Operational.execution_adapter_identity =
        Ksa64BridgeTypedValidation::FullExecutionAdapterIdentity;
    Operational.role = Ksa64BridgeTypedValidation::GuidedOperatorRole;
    Operational.lifecycle = 3;
    Operational.pace = 1;
    Operational.release_period_micros = 31'250;
    Operational.frame = 3;
    Operational.gnss_state = 1;
    Operational.prediction_identity = 1;
    Operational.prediction_checksum = 2;
    TestTrue(
        TEXT("accepted Guided Operator operational view"),
        Ksa64BridgeTypedValidation::Operational(
            Operational,
            KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS,
            Ksa64BridgeTypedValidation::FullExecutionAdapterIdentity));
    Operational.role = 5;
    TestFalse(
        TEXT("SIM Director role rejected"),
        Ksa64BridgeTypedValidation::Operational(
            Operational,
            KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS,
            Ksa64BridgeTypedValidation::FullExecutionAdapterIdentity));
    Operational.role = Ksa64BridgeTypedValidation::GuidedOperatorRole;
    Operational.validity_mask |= 1ull << 63;
    TestFalse(
        TEXT("unknown operational validity bit rejected"),
        Ksa64BridgeTypedValidation::Operational(
            Operational,
            KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS,
            Ksa64BridgeTypedValidation::FullExecutionAdapterIdentity));
    Operational.validity_mask &= ~(1ull << 63);
    Operational.abi_version = 999;
    TestFalse(
        TEXT("typed ABI mismatch rejected"),
        Ksa64BridgeTypedValidation::Operational(
            Operational,
            KSA64_VIEWER_SCENARIO_FULL_GNSS_LOSS,
            Ksa64BridgeTypedValidation::FullExecutionAdapterIdentity));

    Ksa64ViewerProcedureViewV1 Procedure = {};
    InitializeHeader(Procedure);
    Procedure.validity_mask = Ksa64BridgeTypedValidation::ValidProcedure;
    Procedure.procedure_identity = 1;
    Procedure.state = 1;
    Procedure.step_count = 1;
    Procedure.title_length = 1;
    Procedure.title[0] = 'P';
    Procedure.instruction_length = 1;
    Procedure.instruction[0] = 'I';
    TestTrue(TEXT("bounded procedure accepted"), Ksa64BridgeTypedValidation::Procedure(Procedure));
    Procedure.title_length = 65;
    TestFalse(TEXT("procedure title overflow rejected"), Ksa64BridgeTypedValidation::Procedure(Procedure));

    Ksa64ViewerDispositionV1 Disposition = {};
    InitializeHeader(Disposition);
    Disposition.validity_mask =
        Ksa64BridgeTypedValidation::ValidDisposition
        | Ksa64BridgeTypedValidation::ValidEvidence;
    Disposition.overall = 1;
    Disposition.objective = 1;
    Disposition.vehicle = 1;
    Disposition.procedure = 1;
    Disposition.operator_disposition = 1;
    Disposition.avionics = 1;
    Disposition.evidence = 1;
    TestTrue(TEXT("complete disposition accepted"), Ksa64BridgeTypedValidation::Disposition(Disposition));
    Disposition.evidence = 2;
    TestFalse(TEXT("evidence validity mismatch rejected"), Ksa64BridgeTypedValidation::Disposition(Disposition));

    Ksa64ViewerTimelineEventV1 Timeline = {};
    InitializeHeader(Timeline);
    Timeline.source = 2;
    Timeline.severity = 1;
    Timeline.event_identity = 1;
    Timeline.label_length = 1;
    Timeline.label[0] = 'E';
    TestTrue(TEXT("bounded timeline event accepted"), Ksa64BridgeTypedValidation::Timeline(Timeline));
    Timeline.source = 7;
    TestFalse(TEXT("unknown timeline source rejected"), Ksa64BridgeTypedValidation::Timeline(Timeline));

    Ksa64ViewerReleaseSampleV1 Sample = {};
    InitializeHeader(Sample);
    Sample.validity_mask =
        Ksa64BridgeTypedValidation::ValidMissionTime
        | Ksa64BridgeTypedValidation::ValidNavigation;
    Sample.frame = 3;
    Sample.flags = 2;
    TestTrue(TEXT("Guided Operator release sample accepted"), Ksa64BridgeTypedValidation::ReleaseSample(Sample));
    Sample.flags = 1;
    TestFalse(TEXT("SIM truth release sample rejected"), Ksa64BridgeTypedValidation::ReleaseSample(Sample));

    Ksa64ViewerPredictionPathHeaderV1 PredictionHeader = {};
    InitializeHeader(PredictionHeader);
    PredictionHeader.validity_mask = Ksa64BridgeTypedValidation::ValidPrediction;
    PredictionHeader.path_identity = 10;
    PredictionHeader.product = 3;
    PredictionHeader.model_identity = 11;
    PredictionHeader.source_estimate_identity = 12;
    PredictionHeader.source_epoch = 20;
    PredictionHeader.generation_epoch = 21;
    PredictionHeader.frame = 3;
    PredictionHeader.terminal_reason = 2;
    PredictionHeader.point_count = 2;
    PredictionHeader.cadence_releases = 32;
    PredictionHeader.path_checksum = 13;
    TestTrue(
        TEXT("bounded ground-estimate prediction accepted"),
        Ksa64BridgeTypedValidation::PredictionHeader(PredictionHeader));
    PredictionHeader.product = 4;
    TestFalse(
        TEXT("SIM truth prediction product rejected"),
        Ksa64BridgeTypedValidation::PredictionHeader(PredictionHeader));
    PredictionHeader.product = 3;
    PredictionHeader.point_count = Ksa64BridgeTypedValidation::MaximumPredictionPoints + 1;
    TestFalse(
        TEXT("oversized prediction path rejected"),
        Ksa64BridgeTypedValidation::PredictionHeader(PredictionHeader));
    PredictionHeader.point_count = 2;
    PredictionHeader.product = KSA64_VIEWER_TRAJECTORY_PRODUCT_PLANNED_REFERENCE;
    PredictionHeader.source_estimate_checksum = 14;
    TestTrue(
        TEXT("planned reference trajectory accepted for its declared source"),
        Ksa64BridgeTypedValidation::TrajectoryHeader(
            PredictionHeader,
            KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE));
    TestFalse(
        TEXT("trajectory product/source mismatch rejected"),
        Ksa64BridgeTypedValidation::TrajectoryHeader(
            PredictionHeader,
            KSA64_VIEWER_TRAJECTORY_GROUND_ESTIMATE));

    Ksa64ViewerPredictionPathPointV1 PredictionPoint = {};
    InitializeHeader(PredictionPoint);
    PredictionPoint.path_identity = 10;
    PredictionPoint.point_index = 1;
    PredictionPoint.frame = 3;
    PredictionPoint.flags = 1;
    TestTrue(
        TEXT("identity-bound prediction point accepted"),
        Ksa64BridgeTypedValidation::PredictionPoint(PredictionPoint, 1, 10, 2));
    TestFalse(
        TEXT("mismatched path identity rejected"),
        Ksa64BridgeTypedValidation::PredictionPoint(PredictionPoint, 1, 20, 2));
    TestFalse(
        TEXT("out-of-range point rejected"),
        Ksa64BridgeTypedValidation::PredictionPoint(PredictionPoint, 2, 10, 2));

    Ksa64ViewerActionProposalV1 Proposal = {};
    InitializeHeader(Proposal);
    Proposal.validity_mask = Ksa64BridgeTypedValidation::ValidAction;
    Proposal.proposal_identity = 20;
    Proposal.load_identity = 20;
    Proposal.load_type = 1;
    Proposal.earliest_commit_epoch = 10;
    Proposal.activation_epoch = 12;
    Proposal.expires_epoch = 20;
    Proposal.permitted_operations = 1;
    Proposal.label_length = 1;
    Proposal.label[0] = 'A';
    TestTrue(TEXT("bounded action proposal accepted"), Ksa64BridgeTypedValidation::ActionProposal(Proposal));
    Proposal.load_identity = 21;
    TestFalse(TEXT("proposal identity mismatch rejected"), Ksa64BridgeTypedValidation::ActionProposal(Proposal));

    Ksa64ViewerActionReceiptV1 Receipt = {};
    InitializeHeader(Receipt);
    Receipt.validity_mask = Ksa64BridgeTypedValidation::ValidAction;
    Receipt.publication_sequence = 1;
    Receipt.proposal_identity = 20;
    Receipt.load_identity = 20;
    Receipt.control_identity = 21;
    Receipt.state = 1;
    Receipt.accepted = 1;
    Receipt.operation = 1;
    TestTrue(TEXT("accepted action receipt validated"), Ksa64BridgeTypedValidation::ActionReceipt(Receipt));
    Receipt.accepted = 2;
    TestFalse(TEXT("non-boolean receipt rejected"), Ksa64BridgeTypedValidation::ActionReceipt(Receipt));

    Ksa64ViewerTransportStatusV1 Transport = {};
    InitializeHeader(Transport);
    Transport.validity_mask = MAX_uint64;
    Transport.command_capacity = Ksa64BridgeTypedValidation::CommandCapacity;
    Transport.event_capacity = Ksa64BridgeTypedValidation::EventCapacity;
    Transport.timeline_capacity = Ksa64BridgeTypedValidation::TimelineCapacity;
    Transport.sample_capacity = Ksa64BridgeTypedValidation::SampleCapacity;
    Transport.worker_state = 1;
    Transport.finalization_state = 1;
    TestTrue(TEXT("bounded transport status accepted"), Ksa64BridgeTypedValidation::Transport(Transport));
    Transport.commands_pending = Transport.command_capacity + 1;
    TestFalse(TEXT("queue over-capacity status rejected"), Ksa64BridgeTypedValidation::Transport(Transport));

    Ksa64ViewerFinishStatusV1 Finish = {};
    InitializeHeader(Finish);
    Finish.lifecycle = 3;
    Finish.finalization_state = 1;
    TestTrue(TEXT("running finish status accepted"), Ksa64BridgeTypedValidation::Finish(Finish));
    Finish.validity_mask = Ksa64BridgeTypedValidation::ValidEvidence;
    Finish.lifecycle = 5;
    Finish.finalization_state = 2;
    Finish.evidence_identity = 30;
    Finish.evidence_length = 1;
    Finish.evidence_crc32 = 31;
    TestTrue(TEXT("completed evidence status accepted"), Ksa64BridgeTypedValidation::Finish(Finish));
    Finish.validity_mask |= 1ull << 63;
    TestFalse(TEXT("unknown finish validity rejected"), Ksa64BridgeTypedValidation::Finish(Finish));
    return true;
}



IMPLEMENT_SIMPLE_AUTOMATION_TEST(
    FKsa64BridgeTrajectorySourcesTest,
    "KSA64.Phase12B.Bridge.TrajectorySources",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::EngineFilter)

bool FKsa64BridgeTrajectorySourcesTest::RunTest(const FString&)
{
    FKsa64BridgeModule& Module = FKsa64BridgeModule::Get();
    if (Module.GetStatus() != EKsa64BridgeStatus::Ready)
    {
        AddError(FString::Printf(TEXT("staged bridge is not ready: %s"), *Module.GetDiagnostic()));
        return false;
    }
    if (!Module.SupportsFeature(KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1))
    {
        AddError(TEXT("staged bridge does not advertise source-selected trajectories"));
        return false;
    }
    if (!Module.StartGuidedOperationsV1())
    {
        AddError(FString::Printf(TEXT("could not open typed operations: %s"), *Module.GetDiagnostic()));
        return false;
    }

    Ksa64ViewerPredictionPathHeaderV1 Header = {};
    const int32 HeaderResult = Module.TrajectoryPathHeaderV1(
        KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE,
        Header);
    TestEqual(TEXT("planned trajectory header accepted"), HeaderResult, KSA64_VIEWER_OK);
    if (HeaderResult == KSA64_VIEWER_OK)
    {
        TestEqual(
            TEXT("planned trajectory product identity"),
            Header.product,
            KSA64_VIEWER_TRAJECTORY_PRODUCT_PLANNED_REFERENCE);
        TestTrue(TEXT("planned trajectory has points"), Header.point_count > 0);
        Ksa64ViewerPredictionPathPointV1 First = {};
        TestEqual(
            TEXT("planned trajectory first point accepted"),
            Module.TrajectoryPathPointV1(
                KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE,
                0,
                First),
            KSA64_VIEWER_OK);
        TestEqual(TEXT("planned point binds path identity"), First.path_identity, Header.path_identity);
        TestEqual(TEXT("planned point binds requested index"), First.point_index, 0u);
        Ksa64ViewerPredictionPathPointV1 Rejected = {};
        TestEqual(
            TEXT("point outside the validated header rejected before FFI"),
            Module.TrajectoryPathPointV1(
                KSA64_VIEWER_TRAJECTORY_PLANNED_REFERENCE,
                Header.point_count,
                Rejected),
            KSA64_VIEWER_INVALID_ARGUMENT);
    }
    Ksa64ViewerPredictionPathHeaderV1 Unknown = {};
    TestEqual(
        TEXT("unknown trajectory source rejected"),
        Module.TrajectoryPathHeaderV1(99, Unknown),
        KSA64_VIEWER_INVALID_ARGUMENT);
    Module.CloseSession();
    return true;
}


#endif
