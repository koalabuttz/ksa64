#pragma once

#include "CoreMinimal.h"

enum class EKsa64OperationsPace : uint8
{
    Realtime,
    Paused,
    FourX,
    SixteenX,
    Fastest
};

enum class EKsa64OperationsActionState : uint8
{
    Unavailable,
    Available,
    Reviewing,
    Staged,
    Committed,
    Executed,
    Cancelled,
    Rejected,
    Expired
};

struct FKsa64OperationsBridgeCapabilities
{
    bool bTypedOperationalView = false;
    bool bTypedProcedure = false;
    bool bTypedActions = false;
    bool bTimeline = false;
    bool bReleaseHistory = false;
    bool bPredictionPaths = false;
    bool bTransportStatus = false;
    bool bDisposition = false;
    bool bAsyncShutdown = false;
};

struct FKsa64OperationsReleasePoint
{
    uint32 ReleaseEpoch = 0;
    uint32 MissionTimeQ16 = 0;
    uint32 FrameIdentity = 0;
    int32 PositionQ12[3] = {0, 0, 0};
    int32 GroundPositionQ12[3] = {0, 0, 0};
    int32 AltitudeQ12Km = 0;
    int32 SpeedQ24KmS = 0;
    int32 DownrangeQ12Km = 0;
    int32 CrossrangeQ12Km = 0;
    bool bHasMissionTime = false;
    bool bHasPosition = false;
    bool bHasGroundEstimate = false;
};

struct FKsa64OperationsPredictionPoint
{
    uint32 PathIdentity = 0;
    uint32 ProductIdentity = 0;
    uint32 ReleaseEpoch = 0;
    uint32 FrameIdentity = 0;
    int32 PositionQ12Km[3] = {0, 0, 0};
    int32 AltitudeQ12Km = 0;
    int32 DownrangeQ12Km = 0;
    int32 CrossrangeQ12Km = 0;
};

struct FKsa64OperationsTimelineItem
{
    uint32 Sequence = 0;
    uint32 ReleaseEpoch = 0;
    FString Category;
    FString Summary;
    bool bAttention = false;
};

struct FKsa64OperationsAccessibilitySettings
{
    float TextScale = 1.0f;
    bool bReducedMotion = false;
    bool bHighContrast = false;
    bool bSoundCues = true;
};

/**
 * Immutable presentation state copied from the role-filtered bridge. Fields
 * which are not present in the negotiated bridge remain explicitly
 * unavailable; Unreal never reconstructs them from canonical records.
 */
struct FKsa64OperationsViewModel
{
    bool bBridgeReady = false;
    bool bSessionOpen = false;
    bool bSnapshotValid = false;
    bool bTruthFiltered = true;
    bool bAdvanceOutstanding = false;
    bool bObservationComplete = true;

    FString BridgeStatus = TEXT("BRIDGE OFFLINE");
    FString SessionStatus = TEXT("NO SESSION");
    FString RoleLabel = TEXT("GUIDED OPERATOR");
    FString FrameLabel = TEXT("FRAME —");
    FString ProcedureLabel = TEXT("PROCEDURE DATA UNAVAILABLE");
    FString ProcedureGuard = TEXT("Typed procedure view not negotiated");
    FString NavigationLabel = TEXT("NAVIGATION —");
    FString CommunicationsLabel = TEXT("GROUND LINK —");
    FString UplinkLabel = TEXT("Typed action service unavailable");
    FString DispositionLabel = TEXT("OUTCOME PENDING");
    FString ActionProposalLabel = TEXT("No typed action proposal");
    FString ActionReceiptLabel = TEXT("No action receipt");
    FString LastDiagnostic;

    uint64 ValidityMask = 0;
    uint64 CommandSequence = 0;
    int32 CommandResult = 0;
    uint32 DefinitionIdentity = 0;
    uint32 Lifecycle = 0;
    uint32 BridgePace = 0;
    uint32 ReleaseEpoch = 0;
    uint32 ReleasePeriodMicros = 31'250;
    uint32 FrameIdentity = 0;
    uint32 MissionTimeQ16 = 0;
    int32 NavigationPositionQ12[3] = {0, 0, 0};
    int32 NavigationVelocityQ24[3] = {0, 0, 0};
    int32 GroundPositionQ12[3] = {0, 0, 0};
    int32 GroundVelocityQ24[3] = {0, 0, 0};
    uint32 GnssState = 0;
    uint32 FlightChecksum = 0;
    uint32 NavigationChecksum = 0;
    uint32 CommandChecksum = 0;
    uint32 ProcedureState = 0;
    uint32 ProcedureStep = 0;
    uint32 StagedLoadIdentity = 0;
    uint32 ActionCount = 0;
    uint32 EventCount = 0;
    uint32 RejectedLoads = 0;
    uint32 Safe = 0;
    uint32 PredictionIdentity = 0;
    int32 PredictionApogeeQ12Km = 0;
    uint32 PredictionTimeToApogeeQ16 = 0;
    uint32 PredictionTimeToImpactQ16 = 0;
    uint32 ProcedureIdentity = 0;
    uint32 ProcedureEnteredEpoch = 0;
    uint32 ProcedureDeadlineEpoch = 0;
    uint32 ProcedureStepCount = 0;
    uint32 ActionProposalIdentity = 0;
    uint32 ActionLoadIdentity = 0;
    uint32 ActionEarliestCommitEpoch = 0;
    uint32 ActionActivationEpoch = 0;
    uint32 ActionExpiresEpoch = 0;
    uint32 ActionPermittedOperations = 0;
    uint64 ActionReceiptSequence = 0;
    uint32 ActionReceiptState = 0;
    uint32 ActionReceiptReason = 0;
    uint32 ActionReceiptAccepted = 0;
    uint32 OverallDisposition = 0;
    uint32 ObjectiveDisposition = 0;
    uint32 VehicleDisposition = 0;
    uint32 ProcedureDisposition = 0;
    uint32 OperatorDisposition = 0;
    uint32 AvionicsDisposition = 0;
    uint32 EvidenceDisposition = 0;
    uint32 CommandsPending = 0;
    uint32 CommandCapacity = 0;
    uint32 TimelinePending = 0;
    uint32 TimelineCapacity = 0;
    uint32 SamplesPending = 0;
    uint32 SampleCapacity = 0;
    uint32 WorkerState = 0;
    uint32 FinalizationState = 0;
    uint32 TransportOverflow = 0;
    uint64 EvidenceLength = 0;
    uint32 EvidenceIdentity = 0;
    uint32 EvidenceCrc32 = 0;
    FString EvidenceStatus = TEXT("EVIDENCE PENDING");
    FString EvidenceSha256;
    FString EvidencePath;
    bool bShutdownRequested = false;

    EKsa64OperationsPace PresentationPace = EKsa64OperationsPace::Paused;
    EKsa64OperationsActionState ActionState = EKsa64OperationsActionState::Unavailable;
    FKsa64OperationsBridgeCapabilities Capabilities;

    FString ToDeterministicJson() const;
};

