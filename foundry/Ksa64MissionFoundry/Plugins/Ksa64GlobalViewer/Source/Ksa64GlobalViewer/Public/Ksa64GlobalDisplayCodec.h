#pragma once

#include "CoreMinimal.h"

namespace Ksa64GlobalPathFlags
{
constexpr uint16 Stale = 1u << 0;
constexpr uint16 Incomplete = 1u << 1;
constexpr uint16 Terminal = 1u << 2;
constexpr uint16 ResyncRequired = 1u << 3;
constexpr uint16 Mask = (1u << 4) - 1u;
}

/** Rust-owned WGS 84/display definition. Values remain in accepted fixed-point units. */
struct FKsa64GlobalDisplayDefinitionProduct
{
    uint32 DisplayIdentity = 0;
    uint32 EarthIdentity = 0;
    uint32 TransformIdentity = 0;
    uint32 MissionIdentity = 0;
    int32 EpochUnixDay = 0;
    int16 EpochTaiMinusUtc = 0;
    int32 SemiMajorQ12Km = 0;
    int32 SemiMinorQ12Km = 0;
    int32 InverseFlatteningQ20 = 0;
    uint32 LaunchAnchorIdentity = 0;
    int32 LaunchGeodeticQ28Q12[3] = {};
    int32 LaunchAnchorEcefQ12Km[3] = {};
    uint32 RecoveryAnchorIdentity = 0;
    int32 RecoveryGeodeticQ28Q12[3] = {};
    int32 RecoveryAnchorEcefQ12Km[3] = {};
    uint32 AvailableSourceMask = 0;
    uint8 AvailableFrameMask = 0;
    uint16 CameraDomainMask = 0;
};

struct FKsa64GlobalResolvedPoseProduct
{
    int32 PositionQ12Km[3] = {};
    int32 VelocityQ24KmS[3] = {};
    int32 AttitudeQ30[4] = {};
};

struct FKsa64GlobalSourcePoseProduct
{
    uint8 Source = 0;
    uint8 ActiveFrame = 0;
    uint32 ValidityMask = 0;
    uint32 ModelIdentity = 0;
    uint32 EstimateIdentity = 0;
    uint32 Checksum = 0;
    uint32 AgeReleases = 0;
    FKsa64GlobalResolvedPoseProduct Active;
    FKsa64GlobalResolvedPoseProduct Ecef;
    FKsa64GlobalResolvedPoseProduct Gcrf;
    FKsa64GlobalResolvedPoseProduct LaunchEnu;
    FKsa64GlobalResolvedPoseProduct RecoveryEnu;
    int32 AngularRateQ24[3] = {};
};

struct FKsa64GlobalDisplaySampleProduct
{
    uint64 Sequence = 0;
    uint32 ReleaseEpoch = 0;
    uint32 MissionTimeQ16 = 0;
    uint8 ActiveFrame = 0;
    uint8 Segment = 0;
    uint8 FlightMode = 0;
    uint8 TransitionCount = 0;
    uint16 EventMask = 0;
    uint32 DiscontinuityMask = 0;
    uint32 ContinuityIdentity = 0;
    int32 GeodeticQ28Q12[3] = {};
    int32 AltitudeQ12Km = 0;
    int32 MachQ24 = 0;
    int32 DynamicPressureQ14Pa = 0;
    int32 TotalMassQ21Kg = 0;
    int32 MainPropellantQ21Kg = 0;
    int32 RcsPropellantQ21Kg = 0;
    int16 GimbalQ15[2] = {};
    uint8 RcsPulses[12] = {};
    uint8 CommandFlags = 0;
    uint8 CommandDiscrete = 0;
    uint16 Alarms = 0;
    TArray<FKsa64GlobalSourcePoseProduct> Sources;
};

struct FKsa64GlobalPathPointProduct
{
    uint32 ReleaseEpoch = 0;
    uint32 MissionTimeQ16 = 0;
    uint8 Segment = 0;
    uint16 EventMask = 0;
    uint32 AnchorIdentity = 0;
    int32 PositionQ12Km[3] = {};
};

struct FKsa64GlobalPathChunkProduct
{
    uint32 PathIdentity = 0;
    uint8 Source = 0;
    uint8 DisplayFrame = 0;
    uint8 Lod = 0;
    uint16 Flags = 0;
    uint16 ChunkIndex = 0;
    uint16 ChunkCount = 0;
    uint32 ModelIdentity = 0;
    uint32 EstimateIdentity = 0;
    uint32 SourceChecksum = 0;
    uint32 ContinuityIdentity = 0;
    TArray<FKsa64GlobalPathPointProduct> Points;
};

struct FKsa64GlobalTransitionProduct
{
    uint32 ReleaseEpoch = 0;
    uint32 MissionTimeQ16 = 0;
    uint8 FromFrame = 0;
    uint8 ToFrame = 0;
    uint8 FromSegment = 0;
    uint8 ToSegment = 0;
    uint8 Reason = 0;
    uint32 TransitionIdentity = 0;
    uint32 TransformIdentity = 0;
    uint32 AnchorIdentity = 0;
    int32 PositionMaxDeltaRaw = 0;
    int32 VelocityMaxDeltaRaw = 0;
    int32 AttitudeMaxDeltaRaw = 0;
    int32 AngularRateMaxDeltaRaw = 0;
    uint32 Checksum = 0;
};

struct FKsa64GlobalReplayEntryProduct
{
    uint32 ReleaseEpoch = 0;
    uint32 MissionTimeQ16 = 0;
    uint8 Kind = 0;
    uint32 SourceIdentity = 0;
    uint32 EventIdentity = 0;
    uint32 DetailIdentity = 0;
};

struct FKsa64GlobalReplayIndexProduct
{
    uint32 IndexIdentity = 0;
    uint32 SessionDefinitionIdentity = 0;
    uint32 FirstRelease = 0;
    uint32 LastRelease = 0;
    uint8 TerminalDisposition = 0;
    uint8 DispositionAxes[6] = {};
    TArray<FKsa64GlobalReplayEntryProduct> Entries;
};

/**
 * Strict decoder for the noncanonical GlobalDisplayV1 payloads emitted by Rust.
 * It rejects malformed lengths, reserved bytes, identities, enums, order, and
 * role-forbidden truth before a renderer can mutate scene state.
 */
class KSA64GLOBALVIEWER_API FKsa64GlobalDisplayCodec final
{
public:
    static bool DecodeDefinition(
        const TArray<uint8>& Payload,
        FKsa64GlobalDisplayDefinitionProduct& Out,
        FString& OutError);
    static bool DecodeSamples(
        const TArray<uint8>& Payload,
        uint32 PermittedSourceMask,
        TArray<FKsa64GlobalDisplaySampleProduct>& Out,
        FString& OutError);
    static bool DecodePath(
        const TArray<uint8>& Payload,
        uint32 PermittedSourceMask,
        FKsa64GlobalPathChunkProduct& Out,
        FString& OutError);
    static bool DecodeTransition(
        const TArray<uint8>& Payload,
        FKsa64GlobalTransitionProduct& Out,
        FString& OutError);
    static bool DecodeReplayIndex(
        const TArray<uint8>& Payload,
        FKsa64GlobalReplayIndexProduct& Out,
        FString& OutError);
};
