//! Fail-closed compatibility audit for the two Phase 10 nominal lineages.
//!
//! The checked-in KTT10/KPH10/KSR10 files are frozen Phase 10 evidence. A
//! fresh execution of the current accepted portable implementation is a
//! distinct lineage. Phase 12C validates both rather than rewriting either:
//! the frozen records must remain strictly decodable with their accepted
//! hashes, while current execution must retain its reviewed hashes, event
//! schedule, and bounded fixed-point deltas from the frozen evidence.

use crate::global_fixtures::GlobalFixtureSet;
use crate::phase10_mission::{
    capture_nominal_global_mission_portable, encode_kph10, encode_ksr10, encode_ktt10,
    GlobalMissionCapture,
};
use ksa64_core::phase10_telemetry::{
    global_evaluation_identity, GlobalEvaluationSummary, GlobalPlotHeader, GlobalPlotPoint,
    GlobalTelemetryFrame, GlobalTelemetryHeader, KPH10_HEADER_LENGTH, KPH10_POINT_LENGTH,
    KSR10_LENGTH, KTT10_FRAME_LENGTH, KTT10_HEADER_LENGTH,
};

pub(crate) const FROZEN_NOMINAL_KTT10: &[u8] =
    include_bytes!("../../phase10/evidence/ksa-g10r-nominal.ktt10");
pub(crate) const FROZEN_NOMINAL_KPH10: &[u8] =
    include_bytes!("../../phase10/evidence/ksa-g10r-nominal.kph10");
pub(crate) const FROZEN_NOMINAL_KSR10: &[u8] =
    include_bytes!("../../phase10/evidence/ksa-g10r-nominal.ksr10");

pub const FROZEN_NOMINAL_KTT10_SHA256_HEX: &str =
    "a50b4b32b1c0feb44a54fc9041c40833717b9032ce127af67a9d34c3488e824a";
pub const FROZEN_NOMINAL_KPH10_SHA256_HEX: &str =
    "cd664e8b72eff7aff1e3c4a5b7fb6859bb9d5178d3b6b6d4c2c06f2c61ed9cf2";
pub const FROZEN_NOMINAL_KSR10_SHA256_HEX: &str =
    "9e8691933789ce6d870d561218d6888f65acb04ef24e02796be33a704c8678aa";

pub const CURRENT_NOMINAL_KTT10_SHA256_HEX: &str =
    "94e887907602e0e4d673ef93f153e7a834538537022e56cd919d6f6523b69bff";
pub const CURRENT_NOMINAL_KPH10_SHA256_HEX: &str =
    "5869a694013d8421a73fb6c638cf8b061ce767c17b4cc14a9b2dd69196e3d286";
pub const CURRENT_NOMINAL_KSR10_SHA256_HEX: &str =
    "17b11e8c645e8e7fd6e408b6307c97aac9ded3b5ac1ca58da84e8d32cdd4883c";

pub const PHASE10_NOMINAL_SPARSE_FRAME_COUNT: usize = 697;
pub const PHASE10_NOMINAL_RELEASE_COUNT: u32 = 22_015;
pub const PHASE10_NOMINAL_FIRST_ARTIFACT_DIFF_OFFSET: usize = 124_720;
pub const PHASE10_NOMINAL_FIRST_ARTIFACT_DIFF_STEP: u32 = 15_328;
pub const PHASE10_NOMINAL_EVENT_IDENTITY: u32 = 0x2ce3_cce0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NominalArtifactKind {
    Ktt10,
    Kph10,
    Ksr10,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NominalDeltaField {
    TruthPosition,
    TruthVelocity,
    TruthAttitude,
    TruthAngularRate,
    EcefPosition,
    EcefVelocity,
    NavigationPosition,
    NavigationVelocity,
    NavigationAttitude,
    Altitude,
    Mach,
    DynamicPressure,
    TotalMass,
    MainPropellant,
    RcsPropellant,
    PlotLatitude,
    PlotLongitude,
    PlotAltitude,
    PlotDownrange,
    PlotCrossrange,
    PlotSpeed,
    SummaryTerminalState,
    SummaryEcefVelocity,
    SummaryGcrfVelocity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase10NominalCompatibilityError {
    Capture,
    Encode(NominalArtifactKind),
    Hash {
        lineage: NominalLineage,
        artifact: NominalArtifactKind,
    },
    StrictRecord {
        lineage: NominalLineage,
        artifact: NominalArtifactKind,
    },
    Identity {
        lineage: NominalLineage,
        artifact: NominalArtifactKind,
    },
    Shape {
        lineage: NominalLineage,
        artifact: NominalArtifactKind,
    },
    EventIdentity,
    ArtifactDifference {
        offset: usize,
        step: u32,
    },
    DeltaExceeded {
        field: NominalDeltaField,
        observed: u32,
        limit: u32,
    },
    SummaryInvariant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NominalLineage {
    FrozenEvidence,
    CurrentReexecution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NominalArtifactHashes {
    pub ktt10: [u8; 32],
    pub kph10: [u8; 32],
    pub ksr10: [u8; 32],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NominalTelemetryMaxDeltas {
    pub truth_position_q12: u32,
    pub truth_velocity_q24: u32,
    pub ecef_position_q12: u32,
    pub ecef_velocity_q24: u32,
    pub navigation_position_q12: u32,
    pub navigation_velocity_q24: u32,
    pub altitude_q12: u32,
    pub mach_q24: u32,
    pub dynamic_pressure_q14: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NominalPlotMaxDeltas {
    pub latitude_q28: u32,
    pub longitude_q28: u32,
    pub altitude_q12: u32,
    pub downrange_q12: u32,
    pub crossrange_q12: u32,
    pub speed_q24: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase10NominalCompatibilityAudit {
    pub frozen_hashes: NominalArtifactHashes,
    pub current_hashes: NominalArtifactHashes,
    pub sparse_frame_count: u16,
    pub release_count: u32,
    pub event_identity: u32,
    pub first_artifact_diff_offset: u32,
    pub first_artifact_diff_step: u32,
    pub telemetry_max_deltas: NominalTelemetryMaxDeltas,
    pub plot_max_deltas: NominalPlotMaxDeltas,
}

pub struct AuditedCurrentNominal {
    pub capture: GlobalMissionCapture,
    pub audit: Phase10NominalCompatibilityAudit,
}

struct DecodedArtifacts {
    header: GlobalTelemetryHeader,
    frames: Vec<GlobalTelemetryFrame>,
    plot_header: GlobalPlotHeader,
    points: Vec<GlobalPlotPoint>,
    summary: GlobalEvaluationSummary,
}

const TELEMETRY_LIMITS: NominalTelemetryMaxDeltas = NominalTelemetryMaxDeltas {
    truth_position_q12: 2,
    truth_velocity_q24: 91,
    ecef_position_q12: 2,
    ecef_velocity_q24: 91,
    navigation_position_q12: 12,
    navigation_velocity_q24: 106,
    altitude_q12: 3,
    mach_q24: 3_087,
    dynamic_pressure_q14: 7_324,
};

const PLOT_LIMITS: NominalPlotMaxDeltas = NominalPlotMaxDeltas {
    latitude_q28: 12,
    longitude_q28: 20,
    altitude_q12: 3,
    downrange_q12: 1,
    crossrange_q12: 2,
    speed_q24: 107,
};

/// Validate the immutable Phase 10 artifacts and the current implementation as
/// separate, explicitly identified lineages. The current capture is returned
/// only after hashes, strict framing, event identity, and reviewed delta bounds
/// all pass.
pub fn audit_phase10_nominal_lineages(
) -> Result<AuditedCurrentNominal, Phase10NominalCompatibilityError> {
    let fixtures = GlobalFixtureSet::embedded();
    let frozen_hashes = expected_hashes(
        FROZEN_NOMINAL_KTT10_SHA256_HEX,
        FROZEN_NOMINAL_KPH10_SHA256_HEX,
        FROZEN_NOMINAL_KSR10_SHA256_HEX,
    )?;
    require_hashes(
        NominalLineage::FrozenEvidence,
        FROZEN_NOMINAL_KTT10,
        FROZEN_NOMINAL_KPH10,
        FROZEN_NOMINAL_KSR10,
        frozen_hashes,
    )?;
    let frozen = decode_artifacts(
        NominalLineage::FrozenEvidence,
        FROZEN_NOMINAL_KTT10,
        FROZEN_NOMINAL_KPH10,
        FROZEN_NOMINAL_KSR10,
        &fixtures,
    )?;

    let capture = capture_nominal_global_mission_portable(|_| {})
        .map_err(|_| Phase10NominalCompatibilityError::Capture)?;
    let current_ktt10 = encode_ktt10(&capture)
        .map_err(|_| Phase10NominalCompatibilityError::Encode(NominalArtifactKind::Ktt10))?;
    let current_kph10 = encode_kph10(&capture)
        .map_err(|_| Phase10NominalCompatibilityError::Encode(NominalArtifactKind::Kph10))?;
    let current_ksr10 = encode_ksr10(&capture)
        .map_err(|_| Phase10NominalCompatibilityError::Encode(NominalArtifactKind::Ksr10))?;
    let current_hashes = expected_hashes(
        CURRENT_NOMINAL_KTT10_SHA256_HEX,
        CURRENT_NOMINAL_KPH10_SHA256_HEX,
        CURRENT_NOMINAL_KSR10_SHA256_HEX,
    )?;
    require_hashes(
        NominalLineage::CurrentReexecution,
        &current_ktt10,
        &current_kph10,
        &current_ksr10,
        current_hashes,
    )?;
    let current = decode_artifacts(
        NominalLineage::CurrentReexecution,
        &current_ktt10,
        &current_kph10,
        &current_ksr10,
        &fixtures,
    )?;

    if frozen.header != current.header
        || frozen.plot_header.identity != current.plot_header.identity
        || frozen.plot_header.point_count != current.plot_header.point_count
        || frozen.plot_header.stride_releases != current.plot_header.stride_releases
    {
        return Err(Phase10NominalCompatibilityError::Identity {
            lineage: NominalLineage::CurrentReexecution,
            artifact: NominalArtifactKind::Kph10,
        });
    }

    let frozen_event_identity = event_identity(&frozen.frames);
    let current_event_identity = event_identity(&current.frames);
    if frozen_event_identity != PHASE10_NOMINAL_EVENT_IDENTITY
        || current_event_identity != PHASE10_NOMINAL_EVENT_IDENTITY
    {
        return Err(Phase10NominalCompatibilityError::EventIdentity);
    }
    let first_diff = first_difference(FROZEN_NOMINAL_KTT10, &current_ktt10);
    let first_diff_step = frame_step_at_offset(&current.frames, first_diff);
    if first_diff != PHASE10_NOMINAL_FIRST_ARTIFACT_DIFF_OFFSET
        || first_diff_step != PHASE10_NOMINAL_FIRST_ARTIFACT_DIFF_STEP
    {
        return Err(Phase10NominalCompatibilityError::ArtifactDifference {
            offset: first_diff,
            step: first_diff_step,
        });
    }
    let telemetry_max_deltas = compare_telemetry(&frozen.frames, &current.frames)?;
    let plot_max_deltas = compare_plot(&frozen.points, &current.points)?;
    compare_summary(frozen.summary, current.summary)?;

    Ok(AuditedCurrentNominal {
        capture,
        audit: Phase10NominalCompatibilityAudit {
            frozen_hashes,
            current_hashes,
            sparse_frame_count: PHASE10_NOMINAL_SPARSE_FRAME_COUNT as u16,
            release_count: PHASE10_NOMINAL_RELEASE_COUNT,
            event_identity: frozen_event_identity,
            first_artifact_diff_offset: first_diff as u32,
            first_artifact_diff_step: first_diff_step,
            telemetry_max_deltas,
            plot_max_deltas,
        },
    })
}

fn decode_artifacts(
    lineage: NominalLineage,
    ktt10: &[u8],
    kph10: &[u8],
    ksr10: &[u8],
    fixtures: &GlobalFixtureSet,
) -> Result<DecodedArtifacts, Phase10NominalCompatibilityError> {
    if ktt10.len() < KTT10_HEADER_LENGTH
        || !(ktt10.len() - KTT10_HEADER_LENGTH).is_multiple_of(KTT10_FRAME_LENGTH)
    {
        return Err(Phase10NominalCompatibilityError::Shape {
            lineage,
            artifact: NominalArtifactKind::Ktt10,
        });
    }
    let header = GlobalTelemetryHeader::decode(&ktt10[..KTT10_HEADER_LENGTH]).map_err(|_| {
        Phase10NominalCompatibilityError::StrictRecord {
            lineage,
            artifact: NominalArtifactKind::Ktt10,
        }
    })?;
    if header.earth_identity != fixtures.earth.identity
        || header.transform_identity != fixtures.transforms.identity
        || header.atmosphere_identity != fixtures.atmosphere.identity
        || header.vehicle_identity != fixtures.vehicle.identity
        || header.mission_identity != fixtures.mission.identity
    {
        return Err(Phase10NominalCompatibilityError::Identity {
            lineage,
            artifact: NominalArtifactKind::Ktt10,
        });
    }
    let mut frames = Vec::new();
    for bytes in ktt10[KTT10_HEADER_LENGTH..].chunks_exact(KTT10_FRAME_LENGTH) {
        frames.push(GlobalTelemetryFrame::decode(bytes).map_err(|_| {
            Phase10NominalCompatibilityError::StrictRecord {
                lineage,
                artifact: NominalArtifactKind::Ktt10,
            }
        })?);
    }
    if frames.len() != PHASE10_NOMINAL_SPARSE_FRAME_COUNT
        || frames.first().map(|frame| frame.step) != Some(1)
        || frames.last().map(|frame| frame.step) != Some(PHASE10_NOMINAL_RELEASE_COUNT)
        || frames.windows(2).any(|pair| {
            pair[0].step >= pair[1].step || pair[0].mission_time_q16 >= pair[1].mission_time_q16
        })
    {
        return Err(Phase10NominalCompatibilityError::Shape {
            lineage,
            artifact: NominalArtifactKind::Ktt10,
        });
    }

    if ksr10.len() != KSR10_LENGTH {
        return Err(Phase10NominalCompatibilityError::Shape {
            lineage,
            artifact: NominalArtifactKind::Ksr10,
        });
    }
    let summary = GlobalEvaluationSummary::decode(ksr10).map_err(|_| {
        Phase10NominalCompatibilityError::StrictRecord {
            lineage,
            artifact: NominalArtifactKind::Ksr10,
        }
    })?;
    if summary.earth_identity != fixtures.earth.identity
        || summary.transform_identity != fixtures.transforms.identity
        || summary.atmosphere_identity != fixtures.atmosphere.identity
        || summary.common.steps == 0
    {
        return Err(Phase10NominalCompatibilityError::Identity {
            lineage,
            artifact: NominalArtifactKind::Ksr10,
        });
    }

    if kph10.len() < KPH10_HEADER_LENGTH {
        return Err(Phase10NominalCompatibilityError::Shape {
            lineage,
            artifact: NominalArtifactKind::Kph10,
        });
    }
    let plot_header = GlobalPlotHeader::decode(&kph10[..KPH10_HEADER_LENGTH]).map_err(|_| {
        Phase10NominalCompatibilityError::StrictRecord {
            lineage,
            artifact: NominalArtifactKind::Kph10,
        }
    })?;
    let expected_plot_length =
        KPH10_HEADER_LENGTH + usize::from(plot_header.point_count) * KPH10_POINT_LENGTH;
    if kph10.len() != expected_plot_length
        || usize::from(plot_header.point_count) != PHASE10_NOMINAL_SPARSE_FRAME_COUNT
        || plot_header.evaluation_identity != global_evaluation_identity(&summary)
    {
        return Err(Phase10NominalCompatibilityError::Shape {
            lineage,
            artifact: NominalArtifactKind::Kph10,
        });
    }
    let mut points = Vec::new();
    for bytes in kph10[KPH10_HEADER_LENGTH..].chunks_exact(KPH10_POINT_LENGTH) {
        points.push(GlobalPlotPoint::decode(bytes).map_err(|_| {
            Phase10NominalCompatibilityError::StrictRecord {
                lineage,
                artifact: NominalArtifactKind::Kph10,
            }
        })?);
    }
    if points.iter().zip(&frames).any(|(point, frame)| {
        point.mission_time_q16 != frame.mission_time_q16
            || point.frame != frame.frame
            || point.segment != frame.segment
            || point.events != frame.events
    }) {
        return Err(Phase10NominalCompatibilityError::Identity {
            lineage,
            artifact: NominalArtifactKind::Kph10,
        });
    }
    Ok(DecodedArtifacts {
        header,
        frames,
        plot_header,
        points,
        summary,
    })
}

fn require_hashes(
    lineage: NominalLineage,
    ktt10: &[u8],
    kph10: &[u8],
    ksr10: &[u8],
    expected: NominalArtifactHashes,
) -> Result<(), Phase10NominalCompatibilityError> {
    for (artifact, bytes, hash) in [
        (NominalArtifactKind::Ktt10, ktt10, expected.ktt10),
        (NominalArtifactKind::Kph10, kph10, expected.kph10),
        (NominalArtifactKind::Ksr10, ksr10, expected.ksr10),
    ] {
        if crate::phase11_session::sha256(bytes) != hash {
            return Err(Phase10NominalCompatibilityError::Hash { lineage, artifact });
        }
    }
    Ok(())
}

fn expected_hashes(
    ktt10: &str,
    kph10: &str,
    ksr10: &str,
) -> Result<NominalArtifactHashes, Phase10NominalCompatibilityError> {
    Ok(NominalArtifactHashes {
        ktt10: decode_sha256(ktt10)?,
        kph10: decode_sha256(kph10)?,
        ksr10: decode_sha256(ksr10)?,
    })
}

fn decode_sha256(value: &str) -> Result<[u8; 32], Phase10NominalCompatibilityError> {
    if value.len() != 64 {
        return Err(Phase10NominalCompatibilityError::StrictRecord {
            lineage: NominalLineage::FrozenEvidence,
            artifact: NominalArtifactKind::Ktt10,
        });
    }
    let mut output = [0; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
            Phase10NominalCompatibilityError::StrictRecord {
                lineage: NominalLineage::FrozenEvidence,
                artifact: NominalArtifactKind::Ktt10,
            }
        })?;
    }
    Ok(output)
}

fn event_identity(frames: &[GlobalTelemetryFrame]) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for frame in frames {
        for word in [
            frame.step,
            frame.mission_time_q16,
            frame.frame as u32,
            frame.segment as u32,
            u32::from(frame.flight_mode),
            u32::from(frame.transition_count),
            u32::from(frame.events),
        ] {
            for byte in word.to_le_bytes() {
                hash ^= u32::from(byte);
                hash = hash.wrapping_mul(0x0100_0193);
            }
        }
    }
    hash.max(1)
}

fn first_difference(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .unwrap_or(left.len().min(right.len()))
}

fn frame_step_at_offset(frames: &[GlobalTelemetryFrame], offset: usize) -> u32 {
    if offset < KTT10_HEADER_LENGTH {
        return 0;
    }
    let frame_index = (offset - KTT10_HEADER_LENGTH) / KTT10_FRAME_LENGTH;
    frames.get(frame_index).map_or(0, |frame| frame.step)
}

fn compare_telemetry(
    frozen: &[GlobalTelemetryFrame],
    current: &[GlobalTelemetryFrame],
) -> Result<NominalTelemetryMaxDeltas, Phase10NominalCompatibilityError> {
    if frozen.len() != current.len() {
        return Err(Phase10NominalCompatibilityError::Shape {
            lineage: NominalLineage::CurrentReexecution,
            artifact: NominalArtifactKind::Ktt10,
        });
    }
    let mut observed = NominalTelemetryMaxDeltas::default();
    for (frozen, current) in frozen.iter().zip(current) {
        if frozen.step != current.step
            || frozen.mission_time_q16 != current.mission_time_q16
            || frozen.frame != current.frame
            || frozen.segment != current.segment
            || frozen.flight_mode != current.flight_mode
            || frozen.events != current.events
            || frozen.transition_count != current.transition_count
            || frozen.truth_attitude_q30 != current.truth_attitude_q30
            || frozen.truth_angular_rate_q24 != current.truth_angular_rate_q24
            || frozen.navigation_attitude_q30 != current.navigation_attitude_q30
            || frozen.total_mass_q21_kg != current.total_mass_q21_kg
            || frozen.main_propellant_q21_kg != current.main_propellant_q21_kg
            || frozen.rcs_propellant_q21_kg != current.rcs_propellant_q21_kg
            || frozen.gimbal_q15 != current.gimbal_q15
            || frozen.rcs_pulses != current.rcs_pulses
            || frozen.command_flags != current.command_flags
            || frozen.command_discrete != current.command_discrete
            || frozen.alarms != current.alarms
            || frozen.checksums[7] != current.checksums[7]
        {
            return Err(Phase10NominalCompatibilityError::EventIdentity);
        }
        observed.truth_position_q12 = observed.truth_position_q12.max(max_array_delta(
            frozen.truth_position_q12,
            current.truth_position_q12,
        ));
        observed.truth_velocity_q24 = observed.truth_velocity_q24.max(max_array_delta(
            frozen.truth_velocity_q24,
            current.truth_velocity_q24,
        ));
        observed.ecef_position_q12 = observed.ecef_position_q12.max(max_array_delta(
            frozen.ecef_position_q12,
            current.ecef_position_q12,
        ));
        observed.ecef_velocity_q24 = observed.ecef_velocity_q24.max(max_array_delta(
            frozen.ecef_velocity_q24,
            current.ecef_velocity_q24,
        ));
        observed.navigation_position_q12 = observed.navigation_position_q12.max(max_array_delta(
            frozen.navigation_position_q12,
            current.navigation_position_q12,
        ));
        observed.navigation_velocity_q24 = observed.navigation_velocity_q24.max(max_array_delta(
            frozen.navigation_velocity_q24,
            current.navigation_velocity_q24,
        ));
        observed.altitude_q12 = observed
            .altitude_q12
            .max(abs_delta(frozen.altitude_q12_km, current.altitude_q12_km));
        observed.mach_q24 = observed
            .mach_q24
            .max(abs_delta(frozen.mach_q24, current.mach_q24));
        observed.dynamic_pressure_q14 = observed.dynamic_pressure_q14.max(abs_delta(
            frozen.dynamic_pressure_q14_pa,
            current.dynamic_pressure_q14_pa,
        ));
    }
    require_telemetry_bounds(observed)?;
    Ok(observed)
}

fn require_telemetry_bounds(
    observed: NominalTelemetryMaxDeltas,
) -> Result<(), Phase10NominalCompatibilityError> {
    for (field, observed, limit) in [
        (
            NominalDeltaField::TruthPosition,
            observed.truth_position_q12,
            TELEMETRY_LIMITS.truth_position_q12,
        ),
        (
            NominalDeltaField::TruthVelocity,
            observed.truth_velocity_q24,
            TELEMETRY_LIMITS.truth_velocity_q24,
        ),
        (
            NominalDeltaField::EcefPosition,
            observed.ecef_position_q12,
            TELEMETRY_LIMITS.ecef_position_q12,
        ),
        (
            NominalDeltaField::EcefVelocity,
            observed.ecef_velocity_q24,
            TELEMETRY_LIMITS.ecef_velocity_q24,
        ),
        (
            NominalDeltaField::NavigationPosition,
            observed.navigation_position_q12,
            TELEMETRY_LIMITS.navigation_position_q12,
        ),
        (
            NominalDeltaField::NavigationVelocity,
            observed.navigation_velocity_q24,
            TELEMETRY_LIMITS.navigation_velocity_q24,
        ),
        (
            NominalDeltaField::Altitude,
            observed.altitude_q12,
            TELEMETRY_LIMITS.altitude_q12,
        ),
        (
            NominalDeltaField::Mach,
            observed.mach_q24,
            TELEMETRY_LIMITS.mach_q24,
        ),
        (
            NominalDeltaField::DynamicPressure,
            observed.dynamic_pressure_q14,
            TELEMETRY_LIMITS.dynamic_pressure_q14,
        ),
    ] {
        if observed > limit {
            return Err(Phase10NominalCompatibilityError::DeltaExceeded {
                field,
                observed,
                limit,
            });
        }
    }
    Ok(())
}

fn compare_plot(
    frozen: &[GlobalPlotPoint],
    current: &[GlobalPlotPoint],
) -> Result<NominalPlotMaxDeltas, Phase10NominalCompatibilityError> {
    if frozen.len() != current.len() {
        return Err(Phase10NominalCompatibilityError::Shape {
            lineage: NominalLineage::CurrentReexecution,
            artifact: NominalArtifactKind::Kph10,
        });
    }
    let mut observed = NominalPlotMaxDeltas::default();
    for (frozen, current) in frozen.iter().zip(current) {
        if frozen.mission_time_q16 != current.mission_time_q16
            || frozen.frame != current.frame
            || frozen.segment != current.segment
            || frozen.events != current.events
        {
            return Err(Phase10NominalCompatibilityError::EventIdentity);
        }
        observed.latitude_q28 = observed
            .latitude_q28
            .max(abs_delta(frozen.latitude_q28_rad, current.latitude_q28_rad));
        observed.longitude_q28 = observed.longitude_q28.max(abs_delta(
            frozen.longitude_q28_rad,
            current.longitude_q28_rad,
        ));
        observed.altitude_q12 = observed
            .altitude_q12
            .max(abs_delta(frozen.altitude_q12_km, current.altitude_q12_km));
        observed.downrange_q12 = observed
            .downrange_q12
            .max(abs_delta(frozen.downrange_q12_km, current.downrange_q12_km));
        observed.crossrange_q12 = observed.crossrange_q12.max(abs_delta(
            frozen.crossrange_q12_km,
            current.crossrange_q12_km,
        ));
        observed.speed_q24 = observed
            .speed_q24
            .max(abs_delta(frozen.speed_q24_km_s, current.speed_q24_km_s));
    }
    for (field, observed, limit) in [
        (
            NominalDeltaField::PlotLatitude,
            observed.latitude_q28,
            PLOT_LIMITS.latitude_q28,
        ),
        (
            NominalDeltaField::PlotLongitude,
            observed.longitude_q28,
            PLOT_LIMITS.longitude_q28,
        ),
        (
            NominalDeltaField::PlotAltitude,
            observed.altitude_q12,
            PLOT_LIMITS.altitude_q12,
        ),
        (
            NominalDeltaField::PlotDownrange,
            observed.downrange_q12,
            PLOT_LIMITS.downrange_q12,
        ),
        (
            NominalDeltaField::PlotCrossrange,
            observed.crossrange_q12,
            PLOT_LIMITS.crossrange_q12,
        ),
        (
            NominalDeltaField::PlotSpeed,
            observed.speed_q24,
            PLOT_LIMITS.speed_q24,
        ),
    ] {
        if observed > limit {
            return Err(Phase10NominalCompatibilityError::DeltaExceeded {
                field,
                observed,
                limit,
            });
        }
    }
    Ok(observed)
}

fn compare_summary(
    frozen: GlobalEvaluationSummary,
    current: GlobalEvaluationSummary,
) -> Result<(), Phase10NominalCompatibilityError> {
    if frozen.common.outcome != current.common.outcome
        || frozen.common.numeric_faults != current.common.numeric_faults
        || frozen.common.steps != current.common.steps
        || frozen.common.metric_validity != current.common.metric_validity
        || frozen.common.terminal_state_a != current.common.terminal_state_a
        || frozen.common.metrics != current.common.metrics
        || frozen.common.events != current.common.events
        || frozen.common.identities != current.common.identities
        || frozen.terminal_frame != current.terminal_frame
        || frozen.terminal_segment != current.terminal_segment
        || frozen.transition_count != current.transition_count
        || frozen.earth_identity != current.earth_identity
        || frozen.transform_identity != current.transform_identity
        || frozen.atmosphere_identity != current.atmosphere_identity
        || frozen.terminal_ecef_position_q12 != current.terminal_ecef_position_q12
        || frozen.terminal_gcrf_position_q12 != current.terminal_gcrf_position_q12
        || frozen.landing_geodetic_q28_q12 != current.landing_geodetic_q28_q12
        || frozen.apogee_q12_km != current.apogee_q12_km
        || frozen.downrange_q12_km != current.downrange_q12_km
        || frozen.crossrange_q12_km != current.crossrange_q12_km
        || frozen.max_navigation_position_error_q12_km
            != current.max_navigation_position_error_q12_km
        || frozen.max_navigation_velocity_error_q24_km_s
            != current.max_navigation_velocity_error_q24_km_s
        || frozen.max_dynamic_pressure_q14_pa != current.max_dynamic_pressure_q14_pa
        || frozen.max_acceleration_q28_km_s2 != current.max_acceleration_q28_km_s2
        || frozen.max_mach_q24 != current.max_mach_q24
        || frozen.terminal_rcs_propellant_q21_kg != current.terminal_rcs_propellant_q21_kg
        || frozen.time_identity != current.time_identity
        || frozen.transition_position_error_q12_km != current.transition_position_error_q12_km
        || frozen.transition_velocity_error_q24_km_s != current.transition_velocity_error_q24_km_s
        || frozen.transition_attitude_error_q30 != current.transition_attitude_error_q30
        || frozen.transition_angular_rate_error_q24 != current.transition_angular_rate_error_q24
        || frozen.transition_checksums != current.transition_checksums
    {
        return Err(Phase10NominalCompatibilityError::SummaryInvariant);
    }
    for (field, observed, limit) in [
        (
            NominalDeltaField::SummaryTerminalState,
            max_array_delta(
                frozen.common.terminal_state_b,
                current.common.terminal_state_b,
            ),
            3,
        ),
        (
            NominalDeltaField::SummaryEcefVelocity,
            max_array_delta(
                frozen.terminal_ecef_velocity_q24,
                current.terminal_ecef_velocity_q24,
            ),
            3,
        ),
        (
            NominalDeltaField::SummaryGcrfVelocity,
            max_array_delta(
                frozen.terminal_gcrf_velocity_q24,
                current.terminal_gcrf_velocity_q24,
            ),
            3,
        ),
    ] {
        if observed > limit {
            return Err(Phase10NominalCompatibilityError::DeltaExceeded {
                field,
                observed,
                limit,
            });
        }
    }
    Ok(())
}

fn max_array_delta<const N: usize>(left: [i32; N], right: [i32; N]) -> u32 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| abs_delta(left, right))
        .max()
        .unwrap_or(0)
}

fn abs_delta(left: i32, right: i32) -> u32 {
    (i64::from(left) - i64::from(right)).unsigned_abs() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_artifacts_are_strict_and_hash_bound() {
        let fixtures = GlobalFixtureSet::embedded();
        let expected = expected_hashes(
            FROZEN_NOMINAL_KTT10_SHA256_HEX,
            FROZEN_NOMINAL_KPH10_SHA256_HEX,
            FROZEN_NOMINAL_KSR10_SHA256_HEX,
        )
        .unwrap();
        require_hashes(
            NominalLineage::FrozenEvidence,
            FROZEN_NOMINAL_KTT10,
            FROZEN_NOMINAL_KPH10,
            FROZEN_NOMINAL_KSR10,
            expected,
        )
        .unwrap();
        let decoded = decode_artifacts(
            NominalLineage::FrozenEvidence,
            FROZEN_NOMINAL_KTT10,
            FROZEN_NOMINAL_KPH10,
            FROZEN_NOMINAL_KSR10,
            &fixtures,
        )
        .unwrap();
        assert_eq!(decoded.frames.len(), PHASE10_NOMINAL_SPARSE_FRAME_COUNT);
        assert_eq!(decoded.points.len(), PHASE10_NOMINAL_SPARSE_FRAME_COUNT);
        assert_eq!(decoded.header.mission_identity, fixtures.mission.identity);
        assert_eq!(decoded.plot_header.point_count, 697);
        assert_eq!(
            event_identity(&decoded.frames),
            PHASE10_NOMINAL_EVENT_IDENTITY
        );
        assert_eq!(
            compare_telemetry(&decoded.frames, &decoded.frames).unwrap(),
            NominalTelemetryMaxDeltas::default()
        );
        assert_eq!(
            compare_plot(&decoded.points, &decoded.points).unwrap(),
            NominalPlotMaxDeltas::default()
        );
        compare_summary(decoded.summary, decoded.summary).unwrap();
    }

    #[test]
    fn corruption_fails_before_a_lineage_is_accepted() {
        let mut corrupt = FROZEN_NOMINAL_KTT10.to_vec();
        corrupt[KTT10_HEADER_LENGTH + 17] ^= 1;
        let expected = expected_hashes(
            FROZEN_NOMINAL_KTT10_SHA256_HEX,
            FROZEN_NOMINAL_KPH10_SHA256_HEX,
            FROZEN_NOMINAL_KSR10_SHA256_HEX,
        )
        .unwrap();
        assert_eq!(
            require_hashes(
                NominalLineage::FrozenEvidence,
                &corrupt,
                FROZEN_NOMINAL_KPH10,
                FROZEN_NOMINAL_KSR10,
                expected,
            ),
            Err(Phase10NominalCompatibilityError::Hash {
                lineage: NominalLineage::FrozenEvidence,
                artifact: NominalArtifactKind::Ktt10,
            })
        );
    }

    #[test]
    #[ignore = "full deterministic Phase 10 nominal compatibility audit"]
    fn current_reexecution_matches_reviewed_lineage() {
        let audited = audit_phase10_nominal_lineages().expect("reviewed Phase 10 lineages");
        assert_eq!(audited.capture.releases, PHASE10_NOMINAL_RELEASE_COUNT);
        assert_eq!(audited.audit.telemetry_max_deltas, TELEMETRY_LIMITS);
        assert_eq!(audited.audit.plot_max_deltas, PLOT_LIMITS);
        assert_eq!(
            audited.audit.first_artifact_diff_offset,
            PHASE10_NOMINAL_FIRST_ARTIFACT_DIFF_OFFSET as u32
        );
        assert_eq!(
            audited.audit.first_artifact_diff_step,
            PHASE10_NOMINAL_FIRST_ARTIFACT_DIFF_STEP
        );
    }
}
