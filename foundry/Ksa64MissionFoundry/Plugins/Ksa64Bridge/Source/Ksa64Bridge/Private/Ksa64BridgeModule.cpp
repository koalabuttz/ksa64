#include "Ksa64BridgeModule.h"

#include "Algo/AllOf.h"
#include "Ksa64BridgeTypedValidation.h"
#include "Ksa64BridgeSha256.h"

#include "Dom/JsonObject.h"
#include "HAL/FileManager.h"
#include "HAL/PlatformProcess.h"
#include "Interfaces/IPluginManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Modules/ModuleManager.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"



DEFINE_LOG_CATEGORY_STATIC(LogKsa64Bridge, Log, All);

namespace
{
constexpr TCHAR LegacyManifestSchema[] = TEXT("ksa64.viewer-bridge-manifest.v1");
constexpr TCHAR PortableManifestSchema[] = TEXT("ksa64.viewer-bridge-artifact.v2");
constexpr TCHAR CatalogSchema[] = TEXT("ksa64.product-catalog.v1");
constexpr TCHAR AcceptedCatalogHash[] =
    TEXT("b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13");
constexpr uint32 AcceptedCatalogCount = 13;
constexpr uint32 LegacyPhase12ABuildIdentity = 0x120A0001u;
constexpr uint32 KnownFeatureMask =
    KSA64_VIEWER_FEATURE_PANIC_PROBE
    | KSA64_VIEWER_FEATURE_OPERATIONS_V1
    | KSA64_VIEWER_FEATURE_TYPED_ACTIONS_V1
    | KSA64_VIEWER_FEATURE_ASYNC_STATUS_V1
    | KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1;
constexpr uint64 GuidedOperationalValidityMask = (1ull << 11) - 1ull;
FString BridgePlatformDirectory()
{
#if PLATFORM_WINDOWS
    return TEXT("Win64");
#elif PLATFORM_LINUX
    return TEXT("Linux");
#elif PLATFORM_MAC
    return TEXT("Mac");
#else
    return TEXT("Unsupported");
#endif
}

FString BridgeTargetTriple()
{
#if PLATFORM_WINDOWS
    return TEXT("x86_64-pc-windows-msvc");
#elif PLATFORM_LINUX
    return TEXT("x86_64-unknown-linux-gnu");
#elif PLATFORM_MAC
    return TEXT("aarch64-apple-darwin");
#else
    return TEXT("unsupported");
#endif
}

FString BridgeOperatingSystem()
{
#if PLATFORM_WINDOWS
    return TEXT("windows");
#elif PLATFORM_LINUX
    return TEXT("linux");
#elif PLATFORM_MAC
    return TEXT("macos");
#else
    return TEXT("unsupported");
#endif
}

FString BridgeArchitecture()
{
#if PLATFORM_MAC
    return TEXT("aarch64");
#else
    return TEXT("x86_64");
#endif
}

FString BridgeLibraryPrefix()
{
#if PLATFORM_WINDOWS
    return TEXT("ksa64_viewer_bridge-");
#else
    return TEXT("libksa64_viewer_bridge-");
#endif
}

FString BridgeLibraryExtension()
{
#if PLATFORM_WINDOWS
    return TEXT(".dll");
#elif PLATFORM_MAC
    return TEXT(".dylib");
#else
    return TEXT(".so");
#endif
}

bool IsSafeLibraryFilename(const FString& Filename)
{
    return FPaths::GetCleanFilename(Filename) == Filename
        && Filename.StartsWith(BridgeLibraryPrefix(), ESearchCase::CaseSensitive)
        && Filename.EndsWith(BridgeLibraryExtension(), ESearchCase::CaseSensitive);
}

static_assert(sizeof(Ksa64ViewerAbiInfo) == 132);
static_assert(sizeof(Ksa64ViewerStartRequestV1) == 48);
static_assert(sizeof(Ksa64ViewerOperationalViewV1) == 208);
static_assert(sizeof(Ksa64ViewerProcedureViewV1) == 376);
static_assert(sizeof(Ksa64ViewerDispositionV1) == 72);
static_assert(sizeof(Ksa64ViewerActionProposalV1) == 144);
static_assert(sizeof(Ksa64ViewerActionReceiptV1) == 80);
static_assert(sizeof(Ksa64ViewerTimelineEventV1) == 136);
static_assert(sizeof(Ksa64ViewerReleaseSampleV1) == 112);
static_assert(sizeof(Ksa64ViewerPredictionPathHeaderV1) == 88);
static_assert(sizeof(Ksa64ViewerPredictionPathPointV1) == 56);
static_assert(sizeof(Ksa64ViewerTransportStatusV1) == 96);
static_assert(sizeof(Ksa64ViewerFinishStatusV1) == 64);
static_assert(sizeof(Ksa64ViewerSpan) == 24);
static_assert(sizeof(Ksa64ViewerOwnedBuffer) == 32);
static_assert(sizeof(Ksa64ViewerEvent) == 24);
static_assert(sizeof(Ksa64ViewerSnapshot) == 184);
static_assert(KSA64_VIEWER_MAX_CALLER_SPAN == 16ull * 1024ull * 1024ull);

bool ReadJson(const FString& Path, TSharedPtr<FJsonObject>& Out, FString& Diagnostic)
{
    FString Text;
    if (!FFileHelper::LoadFileToString(Text, *Path))
    {
        Diagnostic = FString::Printf(TEXT("could not read manifest: %s"), *Path);
        return false;
    }
    const TSharedRef<TJsonReader<>> Reader = TJsonReaderFactory<>::Create(Text);
    if (!FJsonSerializer::Deserialize(Reader, Out) || !Out.IsValid())
    {
        Diagnostic = TEXT("manifest is not a JSON object");
        return false;
    }
    return true;
}

bool ExactUint32(
    const TSharedPtr<FJsonObject>& Object,
    const TCHAR* Field,
    uint32& Out,
    FString& Diagnostic)
{
    double Number = 0.0;
    if (!Object->TryGetNumberField(Field, Number)
        || Number < 0.0
        || Number > static_cast<double>(MAX_uint32)
        || FMath::Floor(Number) != Number)
    {
        Diagnostic = FString::Printf(TEXT("manifest field '%s' is not an exact uint32"), Field);
        return false;
    }
    Out = static_cast<uint32>(Number);
    return true;
}

bool ExactStructSize(
    const TSharedPtr<FJsonObject>& Sizes,
    const TCHAR* Field,
    SIZE_T Expected,
    FString& Diagnostic)
{
    uint32 Actual = 0;
    if (!ExactUint32(Sizes, Field, Actual, Diagnostic))
    {
        return false;
    }
    if (Actual != Expected)
    {
        Diagnostic = FString::Printf(
            TEXT("manifest structure '%s' is %u bytes; this client expects %llu"),
            Field,
            Actual,
            static_cast<unsigned long long>(Expected));
        return false;
    }
    return true;
}

bool IsLowerHex(const FString& Value, int32 RequiredLength)
{
    if (Value.Len() != RequiredLength)
    {
        return false;
    }
    for (TCHAR C : Value)
    {
        if (!((C >= TEXT('0') && C <= TEXT('9')) || (C >= TEXT('a') && C <= TEXT('f'))))
        {
            return false;
        }
    }
    return true;
}

FString HexBytes(const uint8* Bytes, SIZE_T Length)
{
    FString Result;
    Result.Reset(static_cast<int32>(Length * 2));
    for (SIZE_T Index = 0; Index < Length; ++Index)
    {
        Result += FString::Printf(TEXT("%02x"), static_cast<uint32>(Bytes[Index]));
    }
    return Result;
}

FString FixedUtf8(const uint8* Bytes, SIZE_T Capacity)
{
    SIZE_T Length = 0;
    while (Length < Capacity && Bytes[Length] != 0)
    {
        ++Length;
    }
    FUTF8ToTCHAR Converted(reinterpret_cast<const ANSICHAR*>(Bytes), static_cast<int32>(Length));
    return FString(Converted.Length(), Converted.Get());
}

bool Sha256Bytes(const TArray<uint8>& Bytes, FString& OutHex, FString& Diagnostic)
{
    OutHex = Ksa64BridgeHash::Sha256Hex(Bytes.GetData(), static_cast<uint64>(Bytes.Num()));
    if (!IsLowerHex(OutHex, 64))
    {
        Diagnostic = TEXT("portable SHA-256 calculation failed");
        return false;
    }
    return true;
}

bool Sha256File(const FString& Path, FString& OutHex, FString& Diagnostic)
{
    TArray<uint8> Bytes;
    if (!FFileHelper::LoadFileToArray(Bytes, *Path))
    {
        Diagnostic = FString::Printf(TEXT("could not read bridge library: %s"), *Path);
        return false;
    }
    if (Bytes.Num() == 0)
    {
        Diagnostic = TEXT("bridge library is empty");
        return false;
    }
    return Sha256Bytes(Bytes, OutHex, Diagnostic);
}
template <typename T>
bool LoadRequiredExport(void* Dll, const TCHAR* Name, T& Out, FString& Diagnostic)
{
    Out = reinterpret_cast<T>(FPlatformProcess::GetDllExport(Dll, Name));
    if (Out == nullptr)
    {
        Diagnostic = FString::Printf(TEXT("bridge DLL is missing export '%s'"), Name);
        return false;
    }
    return true;
}

template <typename T>
void LoadOptionalExport(void* Dll, const TCHAR* Name, T& Out)
{
    Out = reinterpret_cast<T>(FPlatformProcess::GetDllExport(Dll, Name));
}
}

struct FKsa64BridgeModule::FApi
{
    using FGetAbiInfo = int32(KSA64_VIEWER_CALL*)(Ksa64ViewerAbiInfo*);
    using FCatalog = int32(KSA64_VIEWER_CALL*)(Ksa64ViewerOwnedBuffer*);
    using FStart = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerSpan*, Ksa64ViewerHandle**);
    using FStartV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerStartRequestV1*, Ksa64ViewerHandle**);
    using FDestroy = int32(KSA64_VIEWER_CALL*)(Ksa64ViewerHandle*);
    using FPause = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*);
    using FResume = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*);
    using FSetPace = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32);
    using FStep = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*);
    using FAdvance = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32);
    using FAbort = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32);
    using FPollSnapshot = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerSnapshot*);
    using FPollEvent = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerEvent*);
    using FOutput = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerOwnedBuffer*);
    using FSubmitStage = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, const Ksa64ViewerSpan*, uint32);
    using FSubmit = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, const Ksa64ViewerSpan*);
    using FFreeBuffer = int32(KSA64_VIEWER_CALL*)(Ksa64ViewerOwnedBuffer*);
    using FPollOperationalV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerOperationalViewV1*);
    using FProcedureV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerProcedureViewV1*);
    using FDispositionV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerDispositionV1*);
    using FPollTimelineV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerTimelineEventV1*);
    using FPollReleaseSampleV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerReleaseSampleV1*);
    using FPredictionHeaderV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerPredictionPathHeaderV1*);
    using FPredictionPointV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32, Ksa64ViewerPredictionPathPointV1*);
    using FTrajectoryHeaderV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32, Ksa64ViewerPredictionPathHeaderV1*);
    using FTrajectoryPointV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32, uint32, Ksa64ViewerPredictionPathPointV1*);
    using FActionProposalV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerActionProposalV1*);
    using FSubmitActionV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32, uint32);
    using FActionIdentityV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, uint32);
    using FPollActionReceiptV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerActionReceiptV1*);
    using FTransportStatusV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerTransportStatusV1*);
    using FFinishStatusV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*, Ksa64ViewerFinishStatusV1*);
    using FRequestShutdownV1 = int32(KSA64_VIEWER_CALL*)(const Ksa64ViewerHandle*);
    using FGlobalDisplayApiV1 = int32(KSA64_VIEWER_CALL*)(Ksa64GlobalDisplayApiV1*);

    FGetAbiInfo GetAbiInfo = nullptr;
    FCatalog Catalog = nullptr;
    FCatalog LibraryDiagnostic = nullptr;
    FStart Start = nullptr;
    FDestroy Destroy = nullptr;
    FPause Pause = nullptr;
    FResume Resume = nullptr;
    FSetPace SetPace = nullptr;
    FStep Step = nullptr;
    FAdvance Advance = nullptr;
    FAbort Abort = nullptr;
    FPollSnapshot PollSnapshot = nullptr;
    FPollEvent PollEvent = nullptr;
    FOutput RecommendedLoad = nullptr;
    FOutput CommitRequest = nullptr;
    FOutput CompletedKsb11 = nullptr;
    FOutput Diagnostic = nullptr;
    FSubmitStage SubmitStage = nullptr;
    FSubmit SubmitCommit = nullptr;
    FSubmit SubmitCancel = nullptr;
    FFreeBuffer FreeBuffer = nullptr;

    FStartV1 StartV1 = nullptr;
    FPollOperationalV1 PollOperationalV1 = nullptr;
    FProcedureV1 ProcedureV1 = nullptr;
    FDispositionV1 DispositionV1 = nullptr;
    FPollTimelineV1 PollTimelineV1 = nullptr;
    FPollReleaseSampleV1 PollReleaseSampleV1 = nullptr;
    FPredictionHeaderV1 PredictionHeaderV1 = nullptr;
    FPredictionPointV1 PredictionPointV1 = nullptr;
    FTrajectoryHeaderV1 TrajectoryHeaderV1 = nullptr;
    FTrajectoryPointV1 TrajectoryPointV1 = nullptr;
    FActionProposalV1 ActionProposalV1 = nullptr;
    FSubmitActionV1 SubmitActionV1 = nullptr;
    FActionIdentityV1 CommitActionV1 = nullptr;
    FActionIdentityV1 CancelActionV1 = nullptr;
    FPollActionReceiptV1 PollActionReceiptV1 = nullptr;
    FTransportStatusV1 TransportStatusV1 = nullptr;
    FFinishStatusV1 FinishStatusV1 = nullptr;
    FRequestShutdownV1 RequestShutdownV1 = nullptr;
    FGlobalDisplayApiV1 GlobalDisplayEntry = nullptr;
    Ksa64GlobalDisplayApiV1 GlobalDisplay = {};

    bool Bind(void* Dll, FString& OutDiagnostic)
    {
#define KSA64_BIND(Field, Name) \
    if (!LoadRequiredExport(Dll, TEXT(Name), Field, OutDiagnostic)) return false
        KSA64_BIND(GetAbiInfo, "ksa64_viewer_get_abi_info");
        KSA64_BIND(Catalog, "ksa64_viewer_catalog");
        KSA64_BIND(LibraryDiagnostic, "ksa64_viewer_library_diagnostic");
        KSA64_BIND(Start, "ksa64_viewer_start");
        KSA64_BIND(Destroy, "ksa64_viewer_destroy");
        KSA64_BIND(Pause, "ksa64_viewer_pause");
        KSA64_BIND(Resume, "ksa64_viewer_resume");
        KSA64_BIND(SetPace, "ksa64_viewer_set_pace");
        KSA64_BIND(Step, "ksa64_viewer_step");
        KSA64_BIND(Advance, "ksa64_viewer_advance");
        KSA64_BIND(Abort, "ksa64_viewer_abort");
        KSA64_BIND(PollSnapshot, "ksa64_viewer_poll_snapshot");
        KSA64_BIND(PollEvent, "ksa64_viewer_poll_event");
        KSA64_BIND(RecommendedLoad, "ksa64_viewer_recommended_load");
        KSA64_BIND(CommitRequest, "ksa64_viewer_commit_request");
        KSA64_BIND(CompletedKsb11, "ksa64_viewer_completed_ksb11");
        KSA64_BIND(Diagnostic, "ksa64_viewer_diagnostic");
        KSA64_BIND(SubmitStage, "ksa64_viewer_submit_stage");
        KSA64_BIND(SubmitCommit, "ksa64_viewer_submit_commit");
        KSA64_BIND(SubmitCancel, "ksa64_viewer_submit_cancel");
        KSA64_BIND(FreeBuffer, "ksa64_viewer_free_buffer");
#undef KSA64_BIND
#define KSA64_OPTIONAL(Field, Name) LoadOptionalExport(Dll, TEXT(Name), Field)
        KSA64_OPTIONAL(StartV1, "ksa64_viewer_start_v1");
        KSA64_OPTIONAL(PollOperationalV1, "ksa64_viewer_poll_operational_v1");
        KSA64_OPTIONAL(ProcedureV1, "ksa64_viewer_procedure_v1");
        KSA64_OPTIONAL(DispositionV1, "ksa64_viewer_disposition_v1");
        KSA64_OPTIONAL(PollTimelineV1, "ksa64_viewer_poll_timeline_v1");
        KSA64_OPTIONAL(PollReleaseSampleV1, "ksa64_viewer_poll_release_sample_v1");
        KSA64_OPTIONAL(PredictionHeaderV1, "ksa64_viewer_prediction_path_header_v1");
        KSA64_OPTIONAL(PredictionPointV1, "ksa64_viewer_prediction_path_point_v1");
        KSA64_OPTIONAL(TrajectoryHeaderV1, "ksa64_viewer_trajectory_path_header_v1");
        KSA64_OPTIONAL(TrajectoryPointV1, "ksa64_viewer_trajectory_path_point_v1");
        KSA64_OPTIONAL(ActionProposalV1, "ksa64_viewer_action_proposal_v1");
        KSA64_OPTIONAL(SubmitActionV1, "ksa64_viewer_submit_action_proposal_v1");
        KSA64_OPTIONAL(CommitActionV1, "ksa64_viewer_commit_action_v1");
        KSA64_OPTIONAL(CancelActionV1, "ksa64_viewer_cancel_action_v1");
        KSA64_OPTIONAL(PollActionReceiptV1, "ksa64_viewer_poll_action_receipt_v1");
        KSA64_OPTIONAL(TransportStatusV1, "ksa64_viewer_transport_status_v1");
        KSA64_OPTIONAL(FinishStatusV1, "ksa64_viewer_finish_status_v1");
        KSA64_OPTIONAL(RequestShutdownV1, "ksa64_viewer_request_shutdown_v1");
        KSA64_OPTIONAL(GlobalDisplayEntry, "ksa64_viewer_global_display_api_v1");
        if (GlobalDisplayEntry != nullptr)
        {
            GlobalDisplay = {};
            GlobalDisplay.api_version = KSA64_GLOBAL_DISPLAY_API_VERSION;
            GlobalDisplay.struct_size = sizeof(GlobalDisplay);
            const int32 GlobalResult = GlobalDisplayEntry(&GlobalDisplay);
            const bool bValidGlobalTable =
                GlobalResult == KSA64_VIEWER_OK
                && GlobalDisplay.api_version == KSA64_GLOBAL_DISPLAY_API_VERSION
                && GlobalDisplay.struct_size == sizeof(GlobalDisplay)
                && GlobalDisplay.replay_start_request_size == sizeof(Ksa64GlobalDisplayReplayStartRequestV1)
                && GlobalDisplay.availability_size == sizeof(Ksa64GlobalDisplayAvailabilityV1)
                && GlobalDisplay.path_request_size == sizeof(Ksa64GlobalDisplayPathRequestV1)
                && GlobalDisplay.sample_range_request_size == sizeof(Ksa64GlobalDisplaySampleRangeRequestV1)
                && GlobalDisplay.owned_buffer_size == sizeof(Ksa64ViewerOwnedBuffer)
                && (GlobalDisplay.feature_flags & KSA64_GLOBAL_DISPLAY_API_IMPLEMENTED) != 0
                && (GlobalDisplay.feature_flags & KSA64_GLOBAL_DISPLAY_API_ROLE_FILTERED) != 0
                && GlobalDisplay.start_nominal_replay != nullptr
                && GlobalDisplay.availability != nullptr
                && GlobalDisplay.definition_payload != nullptr
                && GlobalDisplay.poll_sample_payload != nullptr
                && GlobalDisplay.sample_range_payload != nullptr
                && GlobalDisplay.poll_transition_payload != nullptr
                && GlobalDisplay.replay_index_payload != nullptr
                && GlobalDisplay.path_chunk_payload != nullptr;
            if (!bValidGlobalTable)
            {
                GlobalDisplayEntry = nullptr;
                GlobalDisplay = {};
            }
        }
#undef KSA64_OPTIONAL
        return true;
    }

    bool HasOperationsV1() const
    {
        return StartV1 && PollOperationalV1 && ProcedureV1 && DispositionV1
            && PollTimelineV1 && PollReleaseSampleV1 && PredictionHeaderV1
            && PredictionPointV1;
    }
    bool HasTrajectorySourcesV1() const
    {
        return TrajectoryHeaderV1 && TrajectoryPointV1;
    }
    bool HasTypedActionsV1() const
    {
        return ActionProposalV1 && SubmitActionV1 && CommitActionV1
            && CancelActionV1 && PollActionReceiptV1;
    }
    bool HasAsyncStatusV1() const
    {
        return TransportStatusV1 && FinishStatusV1 && RequestShutdownV1;
    }
    bool HasGlobalDisplayV1() const
    {
        return GlobalDisplayEntry != nullptr
            && GlobalDisplay.start_nominal_replay != nullptr
            && GlobalDisplay.availability != nullptr
            && GlobalDisplay.definition_payload != nullptr
            && GlobalDisplay.poll_sample_payload != nullptr
            && GlobalDisplay.sample_range_payload != nullptr
            && GlobalDisplay.poll_transition_payload != nullptr
            && GlobalDisplay.replay_index_payload != nullptr
            && GlobalDisplay.path_chunk_payload != nullptr;
    }
};

FKsa64BridgeModule::FKsa64BridgeModule() = default;
FKsa64BridgeModule::~FKsa64BridgeModule() = default;

IMPLEMENT_MODULE(FKsa64BridgeModule, Ksa64Bridge)

FKsa64BridgeModule& FKsa64BridgeModule::Get()
{
    return FModuleManager::LoadModuleChecked<FKsa64BridgeModule>(TEXT("Ksa64Bridge"));
}

bool FKsa64BridgeModule::IsAvailable()
{
    return FModuleManager::Get().IsModuleLoaded(TEXT("Ksa64Bridge"));
}

void FKsa64BridgeModule::StartupModule()
{
    Diagnostic = TEXT("KSA64 viewer bridge has not been staged");
    if (LoadBridge())
    {
        UE_LOG(LogKsa64Bridge, Display, TEXT("%s"), *Diagnostic);
    }
    else
    {
        UE_LOG(LogKsa64Bridge, Warning, TEXT("%s"), *Diagnostic);
    }
}

void FKsa64BridgeModule::ShutdownModule()
{
    CloseSession();
    UnloadBridge();
}

bool FKsa64BridgeModule::ValidateArtifactManifest(
    const FString& ManifestPath,
    FKsa64BridgeValidation& OutValidation,
    FString& OutDiagnostic)
{
    OutValidation = {};
    OutDiagnostic.Reset();

    const FString FullManifest = FPaths::ConvertRelativePathToFull(ManifestPath);
    TSharedPtr<FJsonObject> Root;
    if (!ReadJson(FullManifest, Root, OutDiagnostic)) return false;

    FString Schema;
    if (!Root->TryGetStringField(TEXT("schema"), Schema))
    {
        OutDiagnostic = TEXT("bridge manifest is missing schema");
        return false;
    }

    FString LibraryFilename;
    FString LibraryHash;
    FString CatalogHash;
    FString SourceCommit;
    FString TargetTriple;
    FString CargoProfile;
    uint32 AbiVersion = 0;
    uint32 BuildIdentity = 0;
    uint32 CatalogCount = AcceptedCatalogCount;
    bool bLegacy = Schema == LegacyManifestSchema;

    if (bLegacy)
    {
        FString CatalogIdentity;
        FString HeaderFilename;
        FString HeaderHash;
        bool bSourceTreeClean = false;
        if (!Root->TryGetStringField(TEXT("dll_filename"), LibraryFilename)
            || !Root->TryGetStringField(TEXT("dll_sha256"), LibraryHash)
            || !Root->TryGetStringField(TEXT("catalog_sha256"), CatalogHash)
            || !Root->TryGetStringField(TEXT("catalog_schema"), CatalogIdentity)
            || !Root->TryGetStringField(TEXT("source_commit"), SourceCommit)
            || !Root->TryGetStringField(TEXT("target_triple"), TargetTriple)
            || !Root->TryGetStringField(TEXT("cargo_profile"), CargoProfile)
            || !Root->TryGetStringField(TEXT("header_filename"), HeaderFilename)
            || !Root->TryGetStringField(TEXT("header_sha256"), HeaderHash)
            || !Root->TryGetBoolField(TEXT("source_tree_clean"), bSourceTreeClean)
            || !ExactUint32(Root, TEXT("abi_version"), AbiVersion, OutDiagnostic)
            || !ExactUint32(Root, TEXT("build_identity"), BuildIdentity, OutDiagnostic)
            || !ExactUint32(Root, TEXT("catalog_count"), CatalogCount, OutDiagnostic))
        {
            if (OutDiagnostic.IsEmpty()) OutDiagnostic = TEXT("legacy bridge manifest is missing a required typed field");
            return false;
        }
        if (!bSourceTreeClean || CatalogIdentity != CatalogSchema
            || HeaderFilename != TEXT("ksa64_viewer_bridge.h")
            || !IsLowerHex(HeaderHash, 64))
        {
            OutDiagnostic = TEXT("legacy bridge manifest identity is malformed or unqualified");
            return false;
        }
    }
    else if (Schema == PortableManifestSchema)
    {
        FString OperatingSystem;
        FString Architecture;
        if (!Root->TryGetStringField(TEXT("library_file"), LibraryFilename)
            || !Root->TryGetStringField(TEXT("sha256"), LibraryHash)
            || !Root->TryGetStringField(TEXT("catalog_identity"), CatalogHash)
            || !Root->TryGetStringField(TEXT("source_commit"), SourceCommit)
            || !Root->TryGetStringField(TEXT("target_triple"), TargetTriple)
            || !Root->TryGetStringField(TEXT("profile"), CargoProfile)
            || !Root->TryGetStringField(TEXT("operating_system"), OperatingSystem)
            || !Root->TryGetStringField(TEXT("architecture"), Architecture)
            || !ExactUint32(Root, TEXT("abi_version"), AbiVersion, OutDiagnostic)
            || !ExactUint32(Root, TEXT("build_identity"), BuildIdentity, OutDiagnostic))
        {
            if (OutDiagnostic.IsEmpty()) OutDiagnostic = TEXT("portable bridge manifest is missing a required typed field");
            return false;
        }
        if (OperatingSystem != BridgeOperatingSystem() || Architecture != BridgeArchitecture())
        {
            OutDiagnostic = TEXT("portable bridge manifest does not match this Unreal platform");
            return false;
        }
    }
    else
    {
        OutDiagnostic = TEXT("bridge manifest schema is unsupported");
        return false;
    }

    if (TargetTriple != BridgeTargetTriple() || CargoProfile != TEXT("viewer"))
    {
        OutDiagnostic = TEXT("bridge manifest target/profile does not match this Unreal platform");
        return false;
    }
    const bool bValidSourceCommit = bLegacy
        ? IsLowerHex(SourceCommit, 40)
        : (IsLowerHex(SourceCommit, 12) || IsLowerHex(SourceCommit, 40));
    if (!bValidSourceCommit || !IsLowerHex(LibraryHash, 64) || CatalogHash != AcceptedCatalogHash)
    {
        OutDiagnostic = TEXT("bridge manifest source, SHA-256, or catalog identity is invalid");
        return false;
    }
    if (!IsSafeLibraryFilename(LibraryFilename))
    {
        OutDiagnostic = TEXT("bridge manifest library filename is not a safe platform basename");
        return false;
    }

    const bool bAcceptedBuildIdentity = bLegacy
        ? (BuildIdentity == KSA64_VIEWER_BUILD_IDENTITY || BuildIdentity == LegacyPhase12ABuildIdentity)
        : BuildIdentity == KSA64_VIEWER_BUILD_IDENTITY;
    if (AbiVersion != KSA64_VIEWER_ABI_VERSION || !bAcceptedBuildIdentity || CatalogCount != AcceptedCatalogCount)
    {
        OutDiagnostic = TEXT("bridge manifest ABI, build, or catalog count is incompatible");
        return false;
    }

    const FString ExpectedFilename = FString::Printf(
        TEXT("%s%s-%08x%s"), *BridgeLibraryPrefix(), *SourceCommit.Left(12), BuildIdentity, *BridgeLibraryExtension());
    if (LibraryFilename != ExpectedFilename)
    {
        OutDiagnostic = TEXT("bridge library filename is not commit/build/platform qualified");
        return false;
    }

    const TSharedPtr<FJsonObject>* Sizes = nullptr;
    if (!Root->TryGetObjectField(TEXT("structure_sizes"), Sizes)
        || !Sizes || !Sizes->IsValid()
        || !ExactStructSize(*Sizes, TEXT("abi_info"), sizeof(Ksa64ViewerAbiInfo), OutDiagnostic)
        || !ExactStructSize(*Sizes, TEXT("span"), sizeof(Ksa64ViewerSpan), OutDiagnostic)
        || !ExactStructSize(*Sizes, TEXT("owned_buffer"), sizeof(Ksa64ViewerOwnedBuffer), OutDiagnostic)
        || !ExactStructSize(*Sizes, TEXT("event"), sizeof(Ksa64ViewerEvent), OutDiagnostic)
        || !ExactStructSize(*Sizes, TEXT("snapshot"), sizeof(Ksa64ViewerSnapshot), OutDiagnostic))
    {
        if (OutDiagnostic.IsEmpty()) OutDiagnostic = TEXT("bridge manifest structure_sizes object is missing");
        return false;
    }

    const FString LibraryPath = FPaths::Combine(FPaths::GetPath(FullManifest), LibraryFilename);
    FString ActualHash;
    if (!Sha256File(LibraryPath, ActualHash, OutDiagnostic)) return false;
    if (ActualHash != LibraryHash)
    {
        OutDiagnostic = TEXT("bridge library SHA-256 does not match its manifest");
        return false;
    }

    OutValidation.ManifestPath = FullManifest;
    OutValidation.DllPath = LibraryPath; // Legacy field name retained for ABI/source compatibility.
    OutValidation.DllSha256 = ActualHash;
    OutValidation.CatalogSha256 = CatalogHash;
    OutValidation.SourceCommit = SourceCommit;
    OutValidation.TargetTriple = TargetTriple;
    OutValidation.AbiVersion = AbiVersion;
    OutValidation.BuildIdentity = BuildIdentity;
    OutValidation.CatalogCount = CatalogCount;
    OutValidation.bSourceTreeClean = true;
    return true;
}
bool FKsa64BridgeModule::LoadBridge()
{
    const TSharedPtr<IPlugin> Plugin = IPluginManager::Get().FindPlugin(TEXT("Ksa64Bridge"));
    if (!Plugin.IsValid())
    {
        SetFault(TEXT("Ksa64Bridge plugin descriptor is unavailable"));
        return false;
    }

    const FString Binaries = FPaths::Combine(Plugin->GetBaseDir(), TEXT("Binaries"), BridgePlatformDirectory());
    TArray<FString> Manifests;
    IFileManager::Get().FindFiles(
        Manifests,
        *FPaths::Combine(Binaries, TEXT("*.manifest.json")),
        true,
        false);
    Manifests.Sort();
    if (Manifests.Num() != 1)
    {
        Diagnostic = FString::Printf(
            TEXT("expected one commit-qualified KSA64 bridge manifest; found %d"),
            Manifests.Num());
        Status = EKsa64BridgeStatus::Unavailable;
        return false;
    }

    FString ValidationDiagnostic;
    if (!ValidateArtifactManifest(
            FPaths::Combine(Binaries, Manifests[0]),
            Validation,
            ValidationDiagnostic))
    {
        SetFault(ValidationDiagnostic);
        return false;
    }

    DllHandle = FPlatformProcess::GetDllHandle(*Validation.DllPath);
    if (DllHandle == nullptr)
    {
        SetFault(TEXT("validated KSA64 bridge library could not be loaded"));
        return false;
    }

    Api = MakeUnique<FApi>();
    if (!Api->Bind(DllHandle, Diagnostic))
    {
        SetFault(Diagnostic);
        UnloadBridge();
        return false;
    }

    Ksa64ViewerAbiInfo Info = {};
    Info.abi_version = KSA64_VIEWER_ABI_VERSION;
    Info.struct_size = static_cast<uint32>(sizeof(Info));
    const int32 AbiResult = Api->GetAbiInfo(&Info);
    if (AbiResult != KSA64_VIEWER_OK)
    {
        const FString LibraryMessage = ReadLibraryDiagnostic();
        SetFault(LibraryMessage.IsEmpty()
            ? FString::Printf(TEXT("loaded KSA64 bridge ABI query failed with code %d"), AbiResult)
            : FString::Printf(TEXT("loaded KSA64 bridge ABI query failed: %s"), *LibraryMessage));
        UnloadBridge();
        return false;
    }
    if (Info.abi_version != Validation.AbiVersion
        || Info.build_identity != Validation.BuildIdentity
        || Info.snapshot_size != sizeof(Ksa64ViewerSnapshot)
        || Info.event_size != sizeof(Ksa64ViewerEvent)
        || Info.span_size != sizeof(Ksa64ViewerSpan)
        || Info.owned_buffer_size != sizeof(Ksa64ViewerOwnedBuffer)
        || Info.release_hz != 32
        || Info.command_capacity == 0
        || Info.event_capacity == 0
        || Info.maximum_advance_releases != KSA64_VIEWER_MAX_ADVANCE_RELEASES
        || (Info.feature_flags & ~KnownFeatureMask) != 0
        || ((Info.feature_flags & KSA64_VIEWER_FEATURE_OPERATIONS_V1) != 0 && !Api->HasOperationsV1())
        || ((Info.feature_flags & KSA64_VIEWER_FEATURE_TYPED_ACTIONS_V1) != 0 && !Api->HasTypedActionsV1())
        || ((Info.feature_flags & KSA64_VIEWER_FEATURE_ASYNC_STATUS_V1) != 0 && !Api->HasAsyncStatusV1())
        || ((Info.feature_flags & KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1) != 0 && !Api->HasTrajectorySourcesV1())
        || Info.catalog_count != AcceptedCatalogCount
        || FixedUtf8(Info.source_commit, sizeof(Info.source_commit)) != Validation.SourceCommit.Left(12)
        || FixedUtf8(Info.target_triple, sizeof(Info.target_triple)) != Validation.TargetTriple
        || HexBytes(Info.catalog_sha256, sizeof(Info.catalog_sha256)) != Validation.CatalogSha256)
    {
        SetFault(TEXT("loaded KSA64 bridge failed ABI/layout/identity negotiation"));
        UnloadBridge();
        return false;
    }

    FeatureFlags = Info.feature_flags;
    if (!LoadAndCheckCatalog())
    {
        UnloadBridge();
        return false;
    }

    Status = EKsa64BridgeStatus::Ready;
    Diagnostic = FString::Printf(
        TEXT("KSA64 viewer bridge ready: %s"),
        *Validation.DllSha256.Left(12));
    return true;
}

bool FKsa64BridgeModule::LoadAndCheckCatalog()
{
    Ksa64ViewerOwnedBuffer Buffer = {};
    Buffer.abi_version = KSA64_VIEWER_ABI_VERSION;
    Buffer.struct_size = static_cast<uint32>(sizeof(Buffer));
    const int32 Result = Api->Catalog(&Buffer);
    if (Result != KSA64_VIEWER_OK || Buffer.data == nullptr || Buffer.length == 0)
    {
        SetFault(TEXT("KSA64 bridge did not return its product catalog"));
        return false;
    }

    if (Buffer.length > static_cast<uint64>(MAX_int32))
    {
        Api->FreeBuffer(&Buffer);
        SetFault(TEXT("KSA64 bridge returned an oversized product catalog"));
        return false;
    }

    TArray<uint8> Bytes;
    Bytes.Append(Buffer.data, static_cast<int32>(Buffer.length));
    const int32 FreeResult = Api->FreeBuffer(&Buffer);
    if (FreeResult != KSA64_VIEWER_OK)
    {
        SetFault(TEXT("KSA64 bridge catalog buffer ownership check failed"));
        return false;
    }

    FString ActualHash;
    if (!Sha256Bytes(Bytes, ActualHash, Diagnostic)
        || ActualHash != Validation.CatalogSha256)
    {
        SetFault(TEXT("loaded KSA64 bridge catalog hash is not accepted"));
        return false;
    }

    FUTF8ToTCHAR Converted(reinterpret_cast<const ANSICHAR*>(Bytes.GetData()), Bytes.Num());
    CatalogJson = FString(Converted.Length(), Converted.Get());
    TSharedPtr<FJsonObject> Catalog;
    const TSharedRef<TJsonReader<>> Reader = TJsonReaderFactory<>::Create(CatalogJson);
    if (!FJsonSerializer::Deserialize(Reader, Catalog) || !Catalog.IsValid())
    {
        SetFault(TEXT("loaded KSA64 bridge catalog is not valid JSON"));
        return false;
    }

    FString Schema;
    const TArray<TSharedPtr<FJsonValue>>* Experiences = nullptr;
    if (!Catalog->TryGetStringField(TEXT("schema"), Schema)
        || Schema != CatalogSchema
        || !Catalog->TryGetArrayField(TEXT("experiences"), Experiences)
        || Experiences == nullptr
        || Experiences->Num() != static_cast<int32>(AcceptedCatalogCount))
    {
        SetFault(TEXT("loaded KSA64 bridge catalog identity/count is incompatible"));
        return false;
    }
    return true;
}

FString FKsa64BridgeModule::ReadLibraryDiagnostic() const
{
    if (!Api.IsValid() || Api->LibraryDiagnostic == nullptr || Api->FreeBuffer == nullptr)
    {
        return {};
    }
    Ksa64ViewerOwnedBuffer Buffer = {};
    Buffer.abi_version = KSA64_VIEWER_ABI_VERSION;
    Buffer.struct_size = static_cast<uint32>(sizeof(Buffer));
    if (Api->LibraryDiagnostic(&Buffer) != KSA64_VIEWER_OK)
    {
        return {};
    }
    if (Buffer.data == nullptr
        || Buffer.length == 0
        || Buffer.length > static_cast<uint64>(MAX_int32))
    {
        if (Buffer.data != nullptr && Buffer.allocation_id != 0)
        {
            Api->FreeBuffer(&Buffer);
        }
        return {};
    }
    FUTF8ToTCHAR Converted(
        reinterpret_cast<const ANSICHAR*>(Buffer.data),
        static_cast<int32>(Buffer.length));
    const FString Message(Converted.Length(), Converted.Get());
    if (Api->FreeBuffer(&Buffer) != KSA64_VIEWER_OK)
    {
        return {};
    }
    return Message;
}

bool FKsa64BridgeModule::StartGuidedGnssLoss()
{
    if (Status != EKsa64BridgeStatus::Ready || Session != nullptr)
    {
        Diagnostic = TEXT("guided session requires a ready bridge with no open session");
        return false;
    }

    static constexpr ANSICHAR Role[] = "guided-operator";
    Ksa64ViewerSpan RoleSpan = {};
    RoleSpan.abi_version = KSA64_VIEWER_ABI_VERSION;
    RoleSpan.struct_size = static_cast<uint32>(sizeof(RoleSpan));
    RoleSpan.data = reinterpret_cast<const uint8*>(Role);
    RoleSpan.length = static_cast<uint64>(sizeof(Role) - 1);
    const int32 Result = Api->Start(&RoleSpan, &Session);
    if (Result != KSA64_VIEWER_OK || Session == nullptr)
    {
        Session = nullptr;
        Diagnostic = FString::Printf(TEXT("guided session start failed with code %d"), Result);
        return false;
    }
    ActiveTypedScenarioIdentity = KSA64_VIEWER_SCENARIO_LEGACY_GNSS_FIXTURE;
    ActiveTypedAdapterIdentity =
        Ksa64BridgeTypedValidation::ExpectedAdapterForScenario(ActiveTypedScenarioIdentity);
    ValidatedPredictionPathIdentity = 0;
    ValidatedPredictionPointCount = 0;
    FMemory::Memzero(ValidatedTrajectoryPathIdentities);
    FMemory::Memzero(ValidatedTrajectoryPointCounts);
    Status = EKsa64BridgeStatus::SessionOpen;
    Diagnostic = TEXT("guided GNSS-loss session opened");
    return true;
}

bool FKsa64BridgeModule::StartGuidedOperationsV1(uint32 ScenarioIdentity)
{
    if (Status != EKsa64BridgeStatus::Ready || Session != nullptr)
    {
        Diagnostic = TEXT("guided operations require a ready bridge with no open session");
        return false;
    }
    if (!SupportsFeature(KSA64_VIEWER_FEATURE_OPERATIONS_V1)
        || !Api.IsValid()
        || Api->StartV1 == nullptr)
    {
        Diagnostic = TEXT("typed Phase 12B operations are unavailable in this bridge");
        return false;
    }
    const uint32 ExpectedAdapterIdentity =
        Ksa64BridgeTypedValidation::ExpectedAdapterForScenario(ScenarioIdentity);
    if (ExpectedAdapterIdentity == 0)
    {
        Diagnostic = TEXT("guided operations rejected an unknown scenario identity");
        return false;
    }

    Ksa64ViewerStartRequestV1 Request = {};
    Request.abi_version = KSA64_VIEWER_ABI_VERSION;
    Request.struct_size = static_cast<uint32>(sizeof(Request));
    Request.scenario_identity = ScenarioIdentity;
    Request.role = 2;
    // Unreal owns presentation pacing and sends explicit bounded release counts.
    // Fast is therefore the execution-side mode that honors those counts;
    // realtime, pause, and single-step remain presentation scheduling policies.
    Request.initial_pace = 1;
    Request.flags = 1;
    const int32 Result = Api->StartV1(&Request, &Session);
    if (Result != KSA64_VIEWER_OK || Session == nullptr)
    {
        Session = nullptr;
        Diagnostic = FString::Printf(TEXT("guided operations start failed with code %d"), Result);
        return false;
    }
    ActiveTypedScenarioIdentity = ScenarioIdentity;
    ActiveTypedAdapterIdentity = ExpectedAdapterIdentity;
    ValidatedPredictionPathIdentity = 0;
    ValidatedPredictionPointCount = 0;
    FMemory::Memzero(ValidatedTrajectoryPathIdentities);
    FMemory::Memzero(ValidatedTrajectoryPointCounts);
    Status = EKsa64BridgeStatus::SessionOpen;
    Diagnostic = TEXT("full guided GNSS-loss operations session opened");
    return true;
}

bool FKsa64BridgeModule::StartNominalGlobalReplayV1(uint32 Role)
{
    if (Status != EKsa64BridgeStatus::Ready || Session != nullptr)
    {
        Diagnostic = TEXT("nominal global replay requires a ready bridge with no open session");
        return false;
    }
    if (Role != 5u || !Api.IsValid() || !Api->HasGlobalDisplayV1())
    {
        Diagnostic = TEXT("read-only SIM Director nominal replay is unavailable in this bridge");
        return false;
    }

    Ksa64GlobalDisplayReplayStartRequestV1 Request = {};
    Request.api_version = KSA64_GLOBAL_DISPLAY_API_VERSION;
    Request.struct_size = static_cast<uint32>(sizeof(Request));
    Request.role = Role;
    Request.flags = KSA64_GLOBAL_DISPLAY_REPLAY_READ_ONLY;
    const int32 Result = Api->GlobalDisplay.start_nominal_replay(&Request, &Session);
    if (Result != KSA64_VIEWER_OK || Session == nullptr)
    {
        Session = nullptr;
        Diagnostic = FString::Printf(TEXT("nominal global replay start failed with code %d"), Result);
        return false;
    }
    ActiveTypedScenarioIdentity = 0;
    ActiveTypedAdapterIdentity = 0;
    ValidatedPredictionPathIdentity = 0;
    ValidatedPredictionPointCount = 0;
    FMemory::Memzero(ValidatedTrajectoryPathIdentities);
    FMemory::Memzero(ValidatedTrajectoryPointCounts);
    Status = EKsa64BridgeStatus::SessionOpen;
    Diagnostic = TEXT("nominal Phase 10 global replay validating");
    return true;
}

int32 FKsa64BridgeModule::AdvanceReleases(uint32 MaximumReleases)
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr)
    {
        return KSA64_VIEWER_LIFECYCLE;
    }
    if (MaximumReleases == 0 || MaximumReleases > KSA64_VIEWER_MAX_ADVANCE_RELEASES)
    {
        return KSA64_VIEWER_INVALID_ARGUMENT;
    }
    return Api->Advance(Session, MaximumReleases);
}

int32 FKsa64BridgeModule::PollOperationalV1(Ksa64ViewerOperationalViewV1& OutView) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->PollOperationalV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerOperationalViewV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->PollOperationalV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::Operational(
            Value,
            ActiveTypedScenarioIdentity,
            ActiveTypedAdapterIdentity))
        return KSA64_VIEWER_INTERNAL;
    OutView = Value;
    return Result;
}

int32 FKsa64BridgeModule::ProcedureV1(Ksa64ViewerProcedureViewV1& OutView) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->ProcedureV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerProcedureViewV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->ProcedureV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::Procedure(Value))
        return KSA64_VIEWER_INTERNAL;
    OutView = Value;
    return Result;
}

int32 FKsa64BridgeModule::DispositionV1(Ksa64ViewerDispositionV1& OutView) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->DispositionV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerDispositionV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->DispositionV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::Disposition(Value))
        return KSA64_VIEWER_INTERNAL;
    OutView = Value;
    return Result;
}

int32 FKsa64BridgeModule::PollTimelineV1(Ksa64ViewerTimelineEventV1& OutEvent) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->PollTimelineV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerTimelineEventV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->PollTimelineV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::Timeline(Value))
        return KSA64_VIEWER_INTERNAL;
    OutEvent = Value;
    return Result;
}

int32 FKsa64BridgeModule::PollReleaseSampleV1(Ksa64ViewerReleaseSampleV1& OutSample) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->PollReleaseSampleV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerReleaseSampleV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->PollReleaseSampleV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::ReleaseSample(Value))
        return KSA64_VIEWER_INTERNAL;
    OutSample = Value;
    return Result;
}

int32 FKsa64BridgeModule::PredictionPathHeaderV1(Ksa64ViewerPredictionPathHeaderV1& OutHeader) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->PredictionHeaderV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerPredictionPathHeaderV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->PredictionHeaderV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
    {
        ValidatedPredictionPathIdentity = 0;
        ValidatedPredictionPointCount = 0;
        return Result;
    }
    if (!Ksa64BridgeTypedValidation::PredictionHeader(Value))
    {
        ValidatedPredictionPathIdentity = 0;
        ValidatedPredictionPointCount = 0;
        return KSA64_VIEWER_INTERNAL;
    }
    ValidatedPredictionPathIdentity = Value.path_identity;
    ValidatedPredictionPointCount = Value.point_count;
    OutHeader = Value;
    return Result;
}

int32 FKsa64BridgeModule::PredictionPathPointV1(uint32 PointIndex, Ksa64ViewerPredictionPathPointV1& OutPoint) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->PredictionPointV1)
        return KSA64_VIEWER_UNSUPPORTED;
    if (ValidatedPredictionPathIdentity == 0 || ValidatedPredictionPointCount == 0)
        return KSA64_VIEWER_NO_DATA;
    if (PointIndex >= ValidatedPredictionPointCount
        || PointIndex >= Ksa64BridgeTypedValidation::MaximumPredictionPoints)
        return KSA64_VIEWER_INVALID_ARGUMENT;
    Ksa64ViewerPredictionPathPointV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->PredictionPointV1(Session, PointIndex, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::PredictionPoint(
            Value,
            PointIndex,
            ValidatedPredictionPathIdentity,
            ValidatedPredictionPointCount))
    {
        ValidatedPredictionPathIdentity = 0;
        ValidatedPredictionPointCount = 0;
        return KSA64_VIEWER_INTERNAL;
    }
    OutPoint = Value;
    return Result;
}

int32 FKsa64BridgeModule::TrajectoryPathHeaderV1(
    uint32 Source,
    Ksa64ViewerPredictionPathHeaderV1& OutHeader) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen
        || Session == nullptr
        || !SupportsFeature(KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1)
        || !Api->TrajectoryHeaderV1)
        return KSA64_VIEWER_UNSUPPORTED;
    if (!Ksa64BridgeTypedValidation::IsTrajectorySource(Source))
        return KSA64_VIEWER_INVALID_ARGUMENT;
    Ksa64ViewerPredictionPathHeaderV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->TrajectoryHeaderV1(Session, Source, &Value);
    if (Result != KSA64_VIEWER_OK)
    {
        ValidatedTrajectoryPathIdentities[Source] = 0;
        ValidatedTrajectoryPointCounts[Source] = 0;
        return Result;
    }
    if (!Ksa64BridgeTypedValidation::TrajectoryHeader(Value, Source))
    {
        ValidatedTrajectoryPathIdentities[Source] = 0;
        ValidatedTrajectoryPointCounts[Source] = 0;
        return KSA64_VIEWER_INTERNAL;
    }
    ValidatedTrajectoryPathIdentities[Source] = Value.path_identity;
    ValidatedTrajectoryPointCounts[Source] = Value.point_count;
    OutHeader = Value;
    return Result;
}

int32 FKsa64BridgeModule::TrajectoryPathPointV1(
    uint32 Source,
    uint32 PointIndex,
    Ksa64ViewerPredictionPathPointV1& OutPoint) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen
        || Session == nullptr
        || !SupportsFeature(KSA64_VIEWER_FEATURE_TRAJECTORY_SOURCES_V1)
        || !Api->TrajectoryPointV1)
        return KSA64_VIEWER_UNSUPPORTED;
    if (!Ksa64BridgeTypedValidation::IsTrajectorySource(Source))
        return KSA64_VIEWER_INVALID_ARGUMENT;
    const uint32 PathIdentity = ValidatedTrajectoryPathIdentities[Source];
    const uint32 PointCount = ValidatedTrajectoryPointCounts[Source];
    if (PathIdentity == 0 || PointCount == 0)
        return KSA64_VIEWER_NO_DATA;
    if (PointIndex >= PointCount
        || PointIndex >= Ksa64BridgeTypedValidation::MaximumPredictionPoints)
        return KSA64_VIEWER_INVALID_ARGUMENT;
    Ksa64ViewerPredictionPathPointV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->TrajectoryPointV1(Session, Source, PointIndex, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::PredictionPoint(
            Value,
            PointIndex,
            PathIdentity,
            PointCount))
    {
        ValidatedTrajectoryPathIdentities[Source] = 0;
        ValidatedTrajectoryPointCounts[Source] = 0;
        return KSA64_VIEWER_INTERNAL;
    }
    OutPoint = Value;
    return Result;
}

int32 FKsa64BridgeModule::ActionProposalV1(Ksa64ViewerActionProposalV1& OutProposal) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->ActionProposalV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerActionProposalV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->ActionProposalV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::ActionProposal(Value))
        return KSA64_VIEWER_INTERNAL;
    OutProposal = Value;
    return Result;
}

int32 FKsa64BridgeModule::SubmitActionProposalV1(uint32 ProposalIdentity, uint32 CompletedEventMask)
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->SubmitActionV1)
        return KSA64_VIEWER_UNSUPPORTED;
    return Api->SubmitActionV1(Session, ProposalIdentity, CompletedEventMask);
}

int32 FKsa64BridgeModule::CommitActionV1(uint32 ProposalIdentity)
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->CommitActionV1)
        return KSA64_VIEWER_UNSUPPORTED;
    return Api->CommitActionV1(Session, ProposalIdentity);
}

int32 FKsa64BridgeModule::CancelActionV1(uint32 ProposalIdentity)
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->CancelActionV1)
        return KSA64_VIEWER_UNSUPPORTED;
    return Api->CancelActionV1(Session, ProposalIdentity);
}

int32 FKsa64BridgeModule::PollActionReceiptV1(Ksa64ViewerActionReceiptV1& OutReceipt) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->PollActionReceiptV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerActionReceiptV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->PollActionReceiptV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::ActionReceipt(Value))
        return KSA64_VIEWER_INTERNAL;
    OutReceipt = Value;
    return Result;
}

int32 FKsa64BridgeModule::TransportStatusV1(Ksa64ViewerTransportStatusV1& OutStatus) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->TransportStatusV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerTransportStatusV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->TransportStatusV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::Transport(Value))
        return KSA64_VIEWER_INTERNAL;
    OutStatus = Value;
    return Result;
}

int32 FKsa64BridgeModule::FinishStatusV1(Ksa64ViewerFinishStatusV1& OutStatus) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->FinishStatusV1)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerFinishStatusV1 Value = {};
    Value.abi_version = KSA64_VIEWER_ABI_VERSION;
    Value.struct_size = static_cast<uint32>(sizeof(Value));
    const int32 Result = Api->FinishStatusV1(Session, &Value);
    if (Result != KSA64_VIEWER_OK)
        return Result;
    if (!Ksa64BridgeTypedValidation::Finish(Value))
        return KSA64_VIEWER_INTERNAL;
    OutStatus = Value;
    return Result;
}

int32 FKsa64BridgeModule::RequestShutdownV1()
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr || !Api->RequestShutdownV1)
        return KSA64_VIEWER_UNSUPPORTED;
    return Api->RequestShutdownV1(Session);
}

bool FKsa64BridgeModule::RequestAsyncClose()
{
    if (Session == nullptr || Status != EKsa64BridgeStatus::SessionOpen)
    {
        return true;
    }
    if (bAsyncClosePending)
    {
        return true;
    }

    Ksa64ViewerTransportStatusV1 Transport = {};
    if (TransportStatusV1(Transport) == KSA64_VIEWER_OK
        && (Transport.worker_state == 2 || Transport.worker_state == 3))
    {
        CloseSession();
        return true;
    }
    const int32 Result = RequestShutdownV1();
    if (Result != KSA64_VIEWER_OK && Result != KSA64_VIEWER_QUEUED)
    {
        return false;
    }
    bAsyncClosePending = true;
    AsyncCloseTickerHandle = FTSTicker::GetCoreTicker().AddTicker(
        FTickerDelegate::CreateRaw(this, &FKsa64BridgeModule::TickAsyncClose));
    return true;
}

bool FKsa64BridgeModule::TickAsyncClose(float)
{
    if (Session == nullptr || Status != EKsa64BridgeStatus::SessionOpen)
    {
        bAsyncClosePending = false;
        AsyncCloseTickerHandle.Reset();
        return false;
    }
    Ksa64ViewerTransportStatusV1 Transport = {};
    const int32 Result = TransportStatusV1(Transport);
    if (Result == KSA64_VIEWER_OK
        && (Transport.worker_state == 2 || Transport.worker_state == 3))
    {
        bAsyncClosePending = false;
        AsyncCloseTickerHandle.Reset();
        CloseSession();
        return false;
    }
    if (Result != KSA64_VIEWER_OK
        && Result != KSA64_VIEWER_NO_DATA
        && Result != KSA64_VIEWER_UNCHANGED)
    {
        UE_LOG(LogKsa64Bridge, Warning, TEXT("async close status poll failed: %d"), Result);
    }
    return true;
}

bool FKsa64BridgeModule::SupportsGlobalDisplayV1() const
{
    return Status == EKsa64BridgeStatus::SessionOpen
        && Session != nullptr
        && Api.IsValid()
        && Api->HasGlobalDisplayV1();
}

int32 FKsa64BridgeModule::GlobalDisplayAvailability(
    Ksa64GlobalDisplayAvailabilityV1& OutAvailability) const
{
    if (!SupportsGlobalDisplayV1()) return KSA64_VIEWER_UNSUPPORTED;
    Ksa64GlobalDisplayAvailabilityV1 Candidate = {};
    Candidate.api_version = KSA64_GLOBAL_DISPLAY_API_VERSION;
    Candidate.struct_size = sizeof(Candidate);
    const int32 Result = Api->GlobalDisplay.availability(Session, &Candidate);
    if (Result != KSA64_VIEWER_OK) return Result;
    const bool bReservedZero = Algo::AllOf(
        Candidate.reserved,
        [](uint32 Value) { return Value == 0; });
    if (!bReservedZero
        || Candidate.api_version != KSA64_GLOBAL_DISPLAY_API_VERSION
        || Candidate.struct_size != sizeof(Candidate)
        || Candidate.display_identity == 0
        || Candidate.role < 1 || Candidate.role > 5
        || (Candidate.flags & ~KSA64_GLOBAL_DISPLAY_AVAILABILITY_ACCEPTED_EXACT) != 0
        || Candidate.available_source_mask == 0
        || (Candidate.available_source_mask & ~0x0fu) != 0
        || Candidate.available_frame_mask == 0
        || (Candidate.available_frame_mask & ~0x07u) != 0
        || (Candidate.sample_count != 0
            && Candidate.oldest_sample_release > Candidate.newest_sample_release))
    {
        return KSA64_VIEWER_INTERNAL;
    }
    OutAvailability = Candidate;
    return KSA64_VIEWER_OK;
}

int32 FKsa64BridgeModule::CopyGlobalPayload(
    Ksa64GlobalPayloadFn Function,
    TArray<uint8>& OutPayload) const
{
    OutPayload.Reset();
    if (!SupportsGlobalDisplayV1() || Function == nullptr)
        return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerOwnedBuffer Buffer = {};
    Buffer.abi_version = KSA64_VIEWER_ABI_VERSION;
    Buffer.struct_size = sizeof(Buffer);
    const int32 Result = Function(Session, &Buffer);
    if (Result != KSA64_VIEWER_OK)
    {
        if (Buffer.data != nullptr) Api->FreeBuffer(&Buffer);
        return Result;
    }
    if (Buffer.data == nullptr
        || Buffer.length == 0
        || Buffer.length > 256u * 1024u
        || Buffer.length > static_cast<uint64>(MAX_int32))
    {
        if (Buffer.data != nullptr) Api->FreeBuffer(&Buffer);
        return KSA64_VIEWER_INTERNAL;
    }
    OutPayload.Append(Buffer.data, static_cast<int32>(Buffer.length));
    const int32 FreeResult = Api->FreeBuffer(&Buffer);
    if (FreeResult != KSA64_VIEWER_OK)
    {
        OutPayload.Reset();
        return FreeResult;
    }
    return KSA64_VIEWER_OK;
}

int32 FKsa64BridgeModule::GlobalDisplayDefinition(TArray<uint8>& OutPayload) const
{
    return CopyGlobalPayload(
        SupportsGlobalDisplayV1() ? Api->GlobalDisplay.definition_payload : nullptr,
        OutPayload);
}

int32 FKsa64BridgeModule::PollGlobalDisplaySample(TArray<uint8>& OutPayload) const
{
    return CopyGlobalPayload(
        SupportsGlobalDisplayV1() ? Api->GlobalDisplay.poll_sample_payload : nullptr,
        OutPayload);
}

int32 FKsa64BridgeModule::GlobalDisplaySampleRange(
    const Ksa64GlobalDisplaySampleRangeRequestV1& Request,
    TArray<uint8>& OutPayload) const
{
    OutPayload.Reset();
    if (!SupportsGlobalDisplayV1()) return KSA64_VIEWER_UNSUPPORTED;
    const bool bReservedZero = Algo::AllOf(
        Request.reserved,
        [](uint32 Value) { return Value == 0; });
    if (!bReservedZero
        || Request.api_version != KSA64_GLOBAL_DISPLAY_API_VERSION
        || Request.struct_size != sizeof(Request)
        || Request.flags != 0
        || Request.max_count == 0
        || Request.max_count > 256u)
    {
        return KSA64_VIEWER_INVALID_ARGUMENT;
    }

    Ksa64ViewerOwnedBuffer Buffer = {};
    Buffer.abi_version = KSA64_VIEWER_ABI_VERSION;
    Buffer.struct_size = sizeof(Buffer);
    const int32 Result =
        Api->GlobalDisplay.sample_range_payload(Session, &Request, &Buffer);
    if (Result != KSA64_VIEWER_OK)
    {
        if (Buffer.data != nullptr) Api->FreeBuffer(&Buffer);
        return Result;
    }
    if (Buffer.data == nullptr
        || Buffer.length == 0
        || Buffer.length > 256u * 1024u
        || Buffer.length > static_cast<uint64>(MAX_int32))
    {
        if (Buffer.data != nullptr) Api->FreeBuffer(&Buffer);
        return KSA64_VIEWER_INTERNAL;
    }
    OutPayload.Append(Buffer.data, static_cast<int32>(Buffer.length));
    const int32 FreeResult = Api->FreeBuffer(&Buffer);
    if (FreeResult != KSA64_VIEWER_OK) OutPayload.Reset();
    return FreeResult;
}

int32 FKsa64BridgeModule::PollGlobalDisplayTransition(TArray<uint8>& OutPayload) const
{
    return CopyGlobalPayload(
        SupportsGlobalDisplayV1() ? Api->GlobalDisplay.poll_transition_payload : nullptr,
        OutPayload);
}

int32 FKsa64BridgeModule::GlobalReplayIndex(TArray<uint8>& OutPayload) const
{
    return CopyGlobalPayload(
        SupportsGlobalDisplayV1() ? Api->GlobalDisplay.replay_index_payload : nullptr,
        OutPayload);
}

int32 FKsa64BridgeModule::GlobalPathChunk(
    const Ksa64GlobalDisplayPathRequestV1& Request,
    TArray<uint8>& OutPayload) const
{
    OutPayload.Reset();
    if (!SupportsGlobalDisplayV1()) return KSA64_VIEWER_UNSUPPORTED;
    Ksa64ViewerOwnedBuffer Buffer = {};
    Buffer.abi_version = KSA64_VIEWER_ABI_VERSION;
    Buffer.struct_size = sizeof(Buffer);
    const int32 Result =
        Api->GlobalDisplay.path_chunk_payload(Session, &Request, &Buffer);
    if (Result != KSA64_VIEWER_OK)
    {
        if (Buffer.data != nullptr) Api->FreeBuffer(&Buffer);
        return Result;
    }
    if (Buffer.data == nullptr
        || Buffer.length == 0
        || Buffer.length > 256u * 1024u
        || Buffer.length > static_cast<uint64>(MAX_int32))
    {
        if (Buffer.data != nullptr) Api->FreeBuffer(&Buffer);
        return KSA64_VIEWER_INTERNAL;
    }
    OutPayload.Append(Buffer.data, static_cast<int32>(Buffer.length));
    const int32 FreeResult = Api->FreeBuffer(&Buffer);
    if (FreeResult != KSA64_VIEWER_OK) OutPayload.Reset();
    return FreeResult;
}

int32 FKsa64BridgeModule::GetCompletedKsb11(TArray<uint8>& OutBytes) const
{
    OutBytes.Reset();
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr)
        return KSA64_VIEWER_LIFECYCLE;
    Ksa64ViewerOwnedBuffer Buffer = {};
    Buffer.abi_version = KSA64_VIEWER_ABI_VERSION;
    Buffer.struct_size = static_cast<uint32>(sizeof(Buffer));
    const int32 Result = Api->CompletedKsb11(Session, &Buffer);
    if (Result != KSA64_VIEWER_OK) return Result;
    if (Buffer.data == nullptr || Buffer.length > static_cast<uint64>(MAX_int32))
    {
        if (Buffer.data != nullptr) Api->FreeBuffer(&Buffer);
        return KSA64_VIEWER_INTERNAL;
    }
    OutBytes.Append(Buffer.data, static_cast<int32>(Buffer.length));
    return Api->FreeBuffer(&Buffer);
}

int32 FKsa64BridgeModule::AdvanceOneRelease()
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr)
    {
        return KSA64_VIEWER_LIFECYCLE;
    }
    return Api->Step(Session);
}

int32 FKsa64BridgeModule::PollEvent(Ksa64ViewerEvent& OutEvent) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen
        || Session == nullptr
        || !Api.IsValid()
        || Api->PollEvent == nullptr)
    {
        return KSA64_VIEWER_LIFECYCLE;
    }
    Ksa64ViewerEvent Candidate = {};
    Candidate.abi_version = KSA64_VIEWER_ABI_VERSION;
    Candidate.struct_size = static_cast<uint32>(sizeof(Candidate));
    const int32 Result = Api->PollEvent(Session, &Candidate);
    if (Result == KSA64_VIEWER_OK)
    {
        if (!Ksa64BridgeTypedValidation::Event(Candidate))
        {
            return KSA64_VIEWER_INTERNAL;
        }
        OutEvent = Candidate;
    }
    return Result;
}

int32 FKsa64BridgeModule::PollSnapshot(Ksa64ViewerSnapshot& OutSnapshot) const
{
    if (Status != EKsa64BridgeStatus::SessionOpen || Session == nullptr)
    {
        return KSA64_VIEWER_LIFECYCLE;
    }
    Ksa64ViewerSnapshot Candidate = {};
    Candidate.abi_version = KSA64_VIEWER_ABI_VERSION;
    Candidate.struct_size = static_cast<uint32>(sizeof(Candidate));
    const int32 Result = Api->PollSnapshot(Session, &Candidate);
    if (Result == KSA64_VIEWER_OK)
    {
        if (Candidate.role != 2
            || (Candidate.validity_mask & ~GuidedOperationalValidityMask) != 0)
        {
            return KSA64_VIEWER_INTERNAL;
        }
        OutSnapshot = Candidate;
    }
    return Result;
}

void FKsa64BridgeModule::CloseSession()
{
    if (AsyncCloseTickerHandle.IsValid())
    {
        FTSTicker::GetCoreTicker().RemoveTicker(AsyncCloseTickerHandle);
        AsyncCloseTickerHandle.Reset();
    }
    bAsyncClosePending = false;
    ActiveTypedScenarioIdentity = 0;
    ActiveTypedAdapterIdentity = 0;
    ValidatedPredictionPathIdentity = 0;
    ValidatedPredictionPointCount = 0;
    FMemory::Memzero(ValidatedTrajectoryPathIdentities);
    FMemory::Memzero(ValidatedTrajectoryPointCounts);
    if (Session != nullptr && Api.IsValid() && Api->Destroy != nullptr)
    {
        Api->Destroy(Session);
        Session = nullptr;
    }
    if (Status == EKsa64BridgeStatus::SessionOpen)
    {
        Status = EKsa64BridgeStatus::Ready;
    }
}

void FKsa64BridgeModule::UnloadBridge()
{
    FeatureFlags = 0;
    Api.Reset();
    if (DllHandle != nullptr)
    {
        FPlatformProcess::FreeDllHandle(DllHandle);
        DllHandle = nullptr;
    }
    if (Status != EKsa64BridgeStatus::Faulted)
    {
        Status = EKsa64BridgeStatus::Unavailable;
    }
}

void FKsa64BridgeModule::SetFault(const FString& Message)
{
    Diagnostic = Message;
    Status = EKsa64BridgeStatus::Faulted;
}
