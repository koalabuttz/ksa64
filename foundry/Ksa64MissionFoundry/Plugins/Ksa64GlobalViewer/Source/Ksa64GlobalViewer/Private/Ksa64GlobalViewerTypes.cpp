#include "Ksa64GlobalViewerTypes.h"

#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"

FString FKsa64GlobalSemanticState::ToDeterministicJson() const
{
    FString Output;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Output);
    Writer->WriteObjectStart();
    Writer->WriteValue(TEXT("schema"), Schema);
    Writer->WriteValue(TEXT("layout"), static_cast<uint32>(Layout));
    Writer->WriteValue(TEXT("experience_mode"), static_cast<uint32>(ExperienceMode));
    Writer->WriteValue(TEXT("replay_pace"), static_cast<uint32>(ReplayPace));
    Writer->WriteValue(TEXT("requested_camera"), static_cast<uint32>(RequestedCamera));
    Writer->WriteValue(TEXT("resolved_camera"), static_cast<uint32>(ResolvedCamera));
    Writer->WriteValue(
        TEXT("display_availability"),
        static_cast<uint32>(DisplayAvailability));
    Writer->WriteValue(TEXT("release_epoch"), ReleaseEpoch);
    Writer->WriteValue(TEXT("mission_time_q16"), MissionTimeQ16);
    Writer->WriteValue(TEXT("frame_identity"), FrameIdentity);
    Writer->WriteValue(TEXT("segment_identity"), SegmentIdentity);
    Writer->WriteValue(TEXT("event_mask"), EventMask);
    Writer->WriteValue(TEXT("discontinuity_mask"), DiscontinuityMask);
    Writer->WriteValue(
        TEXT("continuity_identity"),
        static_cast<int64>(ContinuityIdentity));
    Writer->WriteValue(TEXT("source_mask"), SourceMask);
    Writer->WriteValue(TEXT("visible_source_mask"), VisibleSourceMask);
    Writer->WriteArrayStart(TEXT("visible_sources"));
    for (const FKsa64GlobalVisibleSourceSemantic& Source : VisibleSources)
    {
        Writer->WriteObjectStart();
        Writer->WriteValue(TEXT("source"), Source.Source);
        Writer->WriteValue(TEXT("model_identity"), Source.ModelIdentity);
        Writer->WriteValue(TEXT("estimate_identity"), Source.EstimateIdentity);
        Writer->WriteValue(TEXT("source_checksum"), Source.SourceChecksum);
        Writer->WriteValue(TEXT("age_releases"), Source.AgeReleases);
        Writer->WriteArrayStart(TEXT("position_q12_km"));
        for (const int32 Axis : Source.PositionQ12Km) Writer->WriteValue(Axis);
        Writer->WriteArrayEnd();
        if (Source.bAttitudeValid)
        {
            Writer->WriteArrayStart(TEXT("attitude_q30"));
            for (const int32 Component : Source.AttitudeQ30)
            {
                Writer->WriteValue(Component);
            }
            Writer->WriteArrayEnd();
        }
        Writer->WriteObjectEnd();
    }
    Writer->WriteArrayEnd();
    Writer->WriteArrayStart(TEXT("visible_paths"));
    for (const FKsa64GlobalVisiblePathSemantic& Path : VisiblePaths)
    {
        Writer->WriteObjectStart();
        Writer->WriteValue(TEXT("identity"), Path.Identity);
        Writer->WriteValue(TEXT("source"), Path.Source);
        Writer->WriteValue(TEXT("model_identity"), Path.ModelIdentity);
        Writer->WriteValue(TEXT("estimate_identity"), Path.EstimateIdentity);
        Writer->WriteValue(TEXT("source_checksum"), Path.SourceChecksum);
        Writer->WriteValue(TEXT("continuity_identity"), Path.ContinuityIdentity);
        Writer->WriteValue(TEXT("flags"), Path.Flags);
        Writer->WriteValue(TEXT("anchor_identity"), Path.AnchorIdentity);
        Writer->WriteValue(TEXT("strip_index"), Path.StripIndex);
        Writer->WriteValue(TEXT("lod_seconds"), Path.LodSeconds);
        Writer->WriteValue(TEXT("point_count"), Path.PointCount);
        Writer->WriteValue(TEXT("point_checksum"), Path.PointChecksum);
        Writer->WriteObjectEnd();
    }
    Writer->WriteArrayEnd();
    Writer->WriteValue(TEXT("observed_path_points"), ObservedPathPoints);
    Writer->WriteValue(TEXT("planned_path_points"), PlannedPathPoints);
    Writer->WriteValue(TEXT("onboard_path_points"), OnboardPathPoints);
    Writer->WriteValue(TEXT("ground_path_points"), GroundPathPoints);
    Writer->WriteValue(TEXT("transition_markers"), TransitionMarkers);
    Writer->WriteValue(TEXT("replay_oldest_release"), ReplayOldestRelease);
    Writer->WriteValue(TEXT("replay_newest_release"), ReplayNewestRelease);
    Writer->WriteValue(TEXT("replay_selected_release"), ReplaySelectedRelease);
    Writer->WriteValue(TEXT("replay_bookmark_count"), ReplayBookmarkCount);
    Writer->WriteValue(TEXT("overall_disposition"), OverallDisposition);
    Writer->WriteValue(TEXT("objective_disposition"), ObjectiveDisposition);
    Writer->WriteValue(TEXT("vehicle_disposition"), VehicleDisposition);
    Writer->WriteValue(TEXT("procedure_disposition"), ProcedureDisposition);
    Writer->WriteValue(TEXT("operator_disposition"), OperatorDisposition);
    Writer->WriteValue(TEXT("avionics_disposition"), AvionicsDisposition);
    Writer->WriteValue(TEXT("evidence_disposition"), EvidenceDisposition);
    Writer->WriteArrayStart(TEXT("display_origin_q12_km"));
    for (const int64 Axis : DisplayOriginQ12Km)
    {
        Writer->WriteValue(FString::Printf(TEXT("%lld"), static_cast<long long>(Axis)));
    }
    Writer->WriteArrayEnd();
    Writer->WriteValue(TEXT("scene_ready"), bSceneReady);
    Writer->WriteValue(TEXT("acceptance_eligible"), bAcceptanceEligible);
    Writer->WriteValue(TEXT("session_open"), bSessionOpen);
    Writer->WriteValue(TEXT("exact_snap"), bExactSnap);
    Writer->WriteValue(TEXT("operations_desk_visible"), bOperationsDeskVisible);
    Writer->WriteValue(TEXT("auto_director_suspended"), bAutoDirectorSuspended);
    Writer->WriteValue(TEXT("truth_permitted"), bTruthPermitted);
    Writer->WriteValue(TEXT("truth_visible"), bTruthVisible);
    Writer->WriteValue(TEXT("attitude_available"), bAttitudeAvailable);
    Writer->WriteValue(TEXT("vehicle_locator_visible"), bVehicleLocatorVisible);
    Writer->WriteValue(TEXT("true_scale_inset_visible"), bTrueScaleInsetVisible);
    Writer->WriteValue(TEXT("observation_complete"), bObservationComplete);
    Writer->WriteValue(TEXT("frame_label"), FrameLabel);
    Writer->WriteValue(TEXT("role_label"), RoleLabel);
    Writer->WriteValue(TEXT("status_label"), StatusLabel);
    Writer->WriteValue(TEXT("source_label"), SourceLabel);
    Writer->WriteValue(TEXT("disposition_label"), DispositionLabel);
    Writer->WriteObjectEnd();
    Writer->Close();
    return Output;
}
