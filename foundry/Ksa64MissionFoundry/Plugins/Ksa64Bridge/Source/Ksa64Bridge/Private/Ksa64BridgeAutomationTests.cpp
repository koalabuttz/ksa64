#include "Ksa64BridgeModule.h"

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

#endif
