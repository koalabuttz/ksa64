#include "Ksa64OperationsTypes.h"

#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"

FString FKsa64OperationsViewModel::ToDeterministicJson() const
{
    FString Output;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Output);

    Writer->WriteObjectStart();
    Writer->WriteValue(TEXT("schema"), TEXT("ksa64.operations-view.v1"));
    Writer->WriteValue(TEXT("bridge_ready"), bBridgeReady);
    Writer->WriteValue(TEXT("session_open"), bSessionOpen);
    Writer->WriteValue(TEXT("snapshot_valid"), bSnapshotValid);
    Writer->WriteValue(TEXT("truth_filtered"), bTruthFiltered);
    Writer->WriteValue(TEXT("advance_outstanding"), bAdvanceOutstanding);
    Writer->WriteValue(TEXT("observation_complete"), bObservationComplete);
    Writer->WriteValue(TEXT("bridge_status"), BridgeStatus);
    Writer->WriteValue(TEXT("session_status"), SessionStatus);
    Writer->WriteValue(TEXT("role"), RoleLabel);
    Writer->WriteValue(TEXT("frame"), FrameLabel);
    Writer->WriteValue(TEXT("release_epoch"), ReleaseEpoch);
    Writer->WriteValue(TEXT("mission_time_q16"), MissionTimeQ16);
    Writer->WriteValue(TEXT("definition_identity"), DefinitionIdentity);
    Writer->WriteValue(TEXT("lifecycle"), Lifecycle);
    Writer->WriteValue(TEXT("procedure_state"), ProcedureState);
    Writer->WriteValue(TEXT("procedure_step"), ProcedureStep);
    Writer->WriteValue(TEXT("procedure_identity"), ProcedureIdentity);
    Writer->WriteValue(TEXT("procedure_deadline_epoch"), ProcedureDeadlineEpoch);
    Writer->WriteValue(TEXT("action_state"), static_cast<uint8>(ActionState));
    Writer->WriteValue(TEXT("action_proposal_identity"), ActionProposalIdentity);
    Writer->WriteValue(TEXT("action_receipt_sequence"), ActionReceiptSequence);
    Writer->WriteValue(TEXT("overall_disposition"), OverallDisposition);
    Writer->WriteValue(TEXT("objective_disposition"), ObjectiveDisposition);
    Writer->WriteValue(TEXT("vehicle_disposition"), VehicleDisposition);
    Writer->WriteValue(TEXT("procedure_disposition"), ProcedureDisposition);
    Writer->WriteValue(TEXT("operator_disposition"), OperatorDisposition);
    Writer->WriteValue(TEXT("avionics_disposition"), AvionicsDisposition);
    Writer->WriteValue(TEXT("evidence_disposition"), EvidenceDisposition);
    Writer->WriteValue(TEXT("typed_actions"), Capabilities.bTypedActions);
    Writer->WriteValue(TEXT("typed_procedure"), Capabilities.bTypedProcedure);
    Writer->WriteValue(TEXT("typed_disposition"), Capabilities.bDisposition);
    Writer->WriteValue(TEXT("flight_checksum"), FlightChecksum);
    Writer->WriteValue(TEXT("navigation_checksum"), NavigationChecksum);
    Writer->WriteValue(TEXT("command_checksum"), CommandChecksum);
    Writer->WriteValue(TEXT("worker_state"), WorkerState);
    Writer->WriteValue(TEXT("finalization_state"), FinalizationState);
    Writer->WriteValue(TEXT("transport_overflow"), TransportOverflow);
    Writer->WriteValue(TEXT("evidence_identity"), EvidenceIdentity);
    Writer->WriteValue(TEXT("evidence_length"), EvidenceLength);
    Writer->WriteValue(TEXT("evidence_crc32"), EvidenceCrc32);
    Writer->WriteValue(TEXT("evidence_status"), EvidenceStatus);
    Writer->WriteValue(TEXT("evidence_sha256"), EvidenceSha256);
    Writer->WriteValue(TEXT("evidence_path"), EvidencePath);
    Writer->WriteValue(TEXT("shutdown_requested"), bShutdownRequested);
    Writer->WriteValue(TEXT("diagnostic"), LastDiagnostic);
    Writer->WriteObjectEnd();
    Writer->Close();
    return Output;
}

