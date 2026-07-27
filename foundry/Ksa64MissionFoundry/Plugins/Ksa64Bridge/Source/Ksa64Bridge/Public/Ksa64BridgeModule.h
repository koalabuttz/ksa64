#pragma once

#include "CoreMinimal.h"
#include "Modules/ModuleInterface.h"
#include "ksa64_viewer_bridge.h"

enum class EKsa64BridgeStatus : uint8
{
    Unavailable,
    Ready,
    SessionOpen,
    Faulted
};

struct FKsa64BridgeValidation
{
    FString ManifestPath;
    FString DllPath;
    FString DllSha256;
    FString CatalogSha256;
    FString SourceCommit;
    uint32 AbiVersion = 0;
    uint32 BuildIdentity = 0;
    uint32 CatalogCount = 0;
    bool bSourceTreeClean = false;
};

/**
 * Presentation-only Phase 12A boundary. All calls enqueue work or inspect
 * immutable, role-filtered data owned by the Rust bridge.
 */
class KSA64BRIDGE_API FKsa64BridgeModule final : public IModuleInterface
{
public:
    static FKsa64BridgeModule& Get();
    static bool IsAvailable();

    virtual void StartupModule() override;
    virtual void ShutdownModule() override;

    EKsa64BridgeStatus GetStatus() const { return Status; }
    const FString& GetDiagnostic() const { return Diagnostic; }
    const FString& GetCatalogJson() const { return CatalogJson; }
    const FKsa64BridgeValidation& GetValidation() const { return Validation; }

    bool StartGuidedGnssLoss();
    int32 AdvanceOneRelease();
    int32 PollSnapshot(Ksa64ViewerSnapshot& OutSnapshot) const;
    void CloseSession();

    /** Validation-only helper used by automation before any DLL is loaded. */
    static bool ValidateArtifactManifest(
        const FString& ManifestPath,
        FKsa64BridgeValidation& OutValidation,
        FString& OutDiagnostic);

private:
    struct FApi;

    bool LoadBridge();
    bool LoadAndCheckCatalog();
    FString ReadLibraryDiagnostic() const;
    void UnloadBridge();
    void SetFault(const FString& Message);

    TUniquePtr<FApi> Api;
    void* DllHandle = nullptr;
    Ksa64ViewerHandle* Session = nullptr;
    EKsa64BridgeStatus Status = EKsa64BridgeStatus::Unavailable;
    FString Diagnostic;
    FString CatalogJson;
    FKsa64BridgeValidation Validation;
};
