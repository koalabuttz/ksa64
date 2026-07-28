#include "Ksa64GlobalDisplayCodec.h"

namespace
{
constexpr int32 PayloadHeaderLength = 12;
constexpr int32 MaximumPayloadLength = 256 * 1024;
constexpr uint32 PoseValidityMask = (1u << 16) - 1u;
constexpr uint32 DiscontinuityMask = (1u << 9) - 1u;
constexpr uint16 PathFlagMask = (1u << 4) - 1u;

class FReader final
{
public:
    FReader(const TArray<uint8>& In, const ANSICHAR ExpectedMagic[5], FString& Error)
        : Bytes(In), OutError(Error)
    {
        if (Bytes.Num() < PayloadHeaderLength || Bytes.Num() > MaximumPayloadLength)
        {
            Fail(TEXT("payload length is outside the GlobalDisplayV1 bound"));
            return;
        }
        if (FMemory::Memcmp(Bytes.GetData(), ExpectedMagic, 4) != 0)
        {
            Fail(TEXT("payload magic does not match its requested product"));
            return;
        }
        uint16 Version = 0;
        uint16 HeaderLength = 0;
        uint32 DeclaredLength = 0;
        FMemory::Memcpy(&Version, Bytes.GetData() + 4, sizeof(Version));
        FMemory::Memcpy(&HeaderLength, Bytes.GetData() + 6, sizeof(HeaderLength));
        FMemory::Memcpy(&DeclaredLength, Bytes.GetData() + 8, sizeof(DeclaredLength));
#if !PLATFORM_LITTLE_ENDIAN
        Version = BYTESWAP_ORDER16(Version);
        HeaderLength = BYTESWAP_ORDER16(HeaderLength);
        DeclaredLength = BYTESWAP_ORDER32(DeclaredLength);
#endif
        if (Version != 1 || HeaderLength != PayloadHeaderLength
            || DeclaredLength != static_cast<uint32>(Bytes.Num()))
        {
            Fail(TEXT("payload version, header length, or declared length is invalid"));
            return;
        }
        Offset = PayloadHeaderLength;
        bValid = true;
    }

    bool IsValid() const { return bValid; }
    bool IsFinished()
    {
        if (!bValid) return false;
        if (Offset != Bytes.Num())
        {
            Fail(TEXT("payload contains trailing or missing bytes"));
            return false;
        }
        return true;
    }
    uint8 U8()
    {
        const uint8* Value = Take(1);
        return Value != nullptr ? Value[0] : 0;
    }
    uint16 U16()
    {
        uint16 Value = 0;
        ReadScalar(Value);
        return Value;
    }
    int16 I16() { return static_cast<int16>(U16()); }
    uint32 U32()
    {
        uint32 Value = 0;
        ReadScalar(Value);
        return Value;
    }
    int32 I32() { return static_cast<int32>(U32()); }
    uint64 U64()
    {
        uint64 Value = 0;
        ReadScalar(Value);
        return Value;
    }
    bool Reserved(int32 Count)
    {
        const uint8* Value = Take(Count);
        if (Value == nullptr) return false;
        for (int32 Index = 0; Index < Count; ++Index)
        {
            if (Value[Index] != 0)
            {
                Fail(TEXT("reserved payload bytes are nonzero"));
                return false;
            }
        }
        return true;
    }
    bool BytesInto(uint8* Destination, int32 Count)
    {
        const uint8* Value = Take(Count);
        if (Value == nullptr) return false;
        FMemory::Memcpy(Destination, Value, Count);
        return true;
    }

private:
    const uint8* Take(int32 Count)
    {
        if (!bValid || Count < 0 || Offset > Bytes.Num() - Count)
        {
            Fail(TEXT("payload is truncated"));
            return nullptr;
        }
        const uint8* Value = Bytes.GetData() + Offset;
        Offset += Count;
        return Value;
    }
    template <typename T>
    void ReadScalar(T& Out)
    {
        const uint8* Value = Take(sizeof(T));
        if (Value == nullptr) return;
        FMemory::Memcpy(&Out, Value, sizeof(T));
#if !PLATFORM_LITTLE_ENDIAN
        if constexpr (sizeof(T) == 2) Out = static_cast<T>(BYTESWAP_ORDER16(Out));
        if constexpr (sizeof(T) == 4) Out = static_cast<T>(BYTESWAP_ORDER32(Out));
        if constexpr (sizeof(T) == 8) Out = static_cast<T>(BYTESWAP_ORDER64(Out));
#endif
    }
    void Fail(const TCHAR* Message)
    {
        if (OutError.IsEmpty()) OutError = Message;
        bValid = false;
    }

    const TArray<uint8>& Bytes;
    FString& OutError;
    int32 Offset = 0;
    bool bValid = false;
};

bool ValidFrame(uint8 Value) { return Value >= 1 && Value <= 3; }
bool ValidSegment(uint8 Value) { return Value >= 1 && Value <= 5; }
bool ValidSource(uint8 Value) { return Value >= 1 && Value <= 4; }

void ReadI32Array(FReader& Reader, int32* Values, int32 Count)
{
    for (int32 Index = 0; Index < Count; ++Index) Values[Index] = Reader.I32();
}

void ReadPose(FReader& Reader, FKsa64GlobalResolvedPoseProduct& Out)
{
    ReadI32Array(Reader, Out.PositionQ12Km, 3);
    ReadI32Array(Reader, Out.VelocityQ24KmS, 3);
    ReadI32Array(Reader, Out.AttitudeQ30, 4);
}

bool ReadSource(
    FReader& Reader,
    uint32 PermittedSourceMask,
    uint32& InOutSeenMask,
    FKsa64GlobalSourcePoseProduct& Out,
    FString& OutError)
{
    Out.Source = Reader.U8();
    Out.ActiveFrame = Reader.U8();
    Reader.Reserved(2);
    Out.ValidityMask = Reader.U32();
    Out.ModelIdentity = Reader.U32();
    Out.EstimateIdentity = Reader.U32();
    Out.Checksum = Reader.U32();
    Out.AgeReleases = Reader.U32();
    ReadPose(Reader, Out.Active);
    ReadPose(Reader, Out.Ecef);
    ReadPose(Reader, Out.Gcrf);
    ReadPose(Reader, Out.LaunchEnu);
    ReadPose(Reader, Out.RecoveryEnu);
    ReadI32Array(Reader, Out.AngularRateQ24, 3);
    if (!Reader.IsValid()) return false;
    const uint32 SourceBit = ValidSource(Out.Source) ? 1u << (Out.Source - 1u) : 0u;
    if (SourceBit == 0 || !ValidFrame(Out.ActiveFrame)
        || (PermittedSourceMask & SourceBit) == 0
        || (InOutSeenMask & SourceBit) != 0
        || Out.ValidityMask == 0
        || (Out.ValidityMask & ~PoseValidityMask) != 0
        || (Out.ValidityMask & 1u) == 0
        || Out.ModelIdentity == 0)
    {
        OutError = TEXT("source pose identity, role, frame, or validity is invalid");
        return false;
    }
    InOutSeenMask |= SourceBit;
    return true;
}
}

bool FKsa64GlobalDisplayCodec::DecodeDefinition(
    const TArray<uint8>& Payload,
    FKsa64GlobalDisplayDefinitionProduct& Out,
    FString& OutError)
{
    Out = {};
    OutError.Reset();
    FReader Reader(Payload, "PGD1", OutError);
    if (!Reader.IsValid()) return false;
    Out.DisplayIdentity = Reader.U32();
    Out.EarthIdentity = Reader.U32();
    Out.TransformIdentity = Reader.U32();
    Out.MissionIdentity = Reader.U32();
    Out.EpochUnixDay = Reader.I32();
    Out.EpochTaiMinusUtc = Reader.I16();
    Reader.Reserved(2);
    Out.SemiMajorQ12Km = Reader.I32();
    Out.SemiMinorQ12Km = Reader.I32();
    Out.InverseFlatteningQ20 = Reader.I32();
    Out.LaunchAnchorIdentity = Reader.U32();
    ReadI32Array(Reader, Out.LaunchGeodeticQ28Q12, 3);
    ReadI32Array(Reader, Out.LaunchAnchorEcefQ12Km, 3);
    Out.RecoveryAnchorIdentity = Reader.U32();
    ReadI32Array(Reader, Out.RecoveryGeodeticQ28Q12, 3);
    ReadI32Array(Reader, Out.RecoveryAnchorEcefQ12Km, 3);
    Out.AvailableSourceMask = Reader.U32();
    Out.AvailableFrameMask = Reader.U8();
    Reader.Reserved(1);
    Out.CameraDomainMask = Reader.U16();
    if (!Reader.IsFinished()) return false;
    if (Out.DisplayIdentity == 0 || Out.EarthIdentity == 0
        || Out.TransformIdentity == 0 || Out.MissionIdentity == 0
        || Out.LaunchAnchorIdentity == 0 || Out.RecoveryAnchorIdentity == 0
        || (Out.LaunchAnchorEcefQ12Km[0] == 0 && Out.LaunchAnchorEcefQ12Km[1] == 0
            && Out.LaunchAnchorEcefQ12Km[2] == 0)
        || (Out.RecoveryAnchorEcefQ12Km[0] == 0 && Out.RecoveryAnchorEcefQ12Km[1] == 0
            && Out.RecoveryAnchorEcefQ12Km[2] == 0)
        || Out.SemiMajorQ12Km <= 0 || Out.SemiMinorQ12Km <= 0
        || Out.InverseFlatteningQ20 <= 0
        || Out.AvailableSourceMask == 0 || (Out.AvailableSourceMask & ~0x0fu) != 0
        || Out.AvailableFrameMask == 0 || (Out.AvailableFrameMask & ~0x07u) != 0
        || Out.CameraDomainMask == 0)
    {
        OutError = TEXT("global display definition contains an invalid identity or envelope");
        return false;
    }
    return true;
}

bool FKsa64GlobalDisplayCodec::DecodeSamples(
    const TArray<uint8>& Payload,
    uint32 PermittedSourceMask,
    TArray<FKsa64GlobalDisplaySampleProduct>& Out,
    FString& OutError)
{
    Out.Reset();
    OutError.Reset();
    FReader Reader(Payload, "PGS1", OutError);
    if (!Reader.IsValid()) return false;
    const uint16 Count = Reader.U16();
    Reader.Reserved(2);
    if (Count == 0)
    {
        OutError = TEXT("sample batch is empty");
        return false;
    }
    uint64 PreviousSequence = 0;
    Out.Reserve(Count);
    for (uint16 SampleIndex = 0; SampleIndex < Count; ++SampleIndex)
    {
        FKsa64GlobalDisplaySampleProduct Sample;
        Sample.Sequence = Reader.U64();
        Sample.ReleaseEpoch = Reader.U32();
        Sample.MissionTimeQ16 = Reader.U32();
        Sample.ActiveFrame = Reader.U8();
        Sample.Segment = Reader.U8();
        Sample.FlightMode = Reader.U8();
        Sample.TransitionCount = Reader.U8();
        Sample.EventMask = Reader.U16();
        const uint16 SourceCount = Reader.U16();
        Sample.DiscontinuityMask = Reader.U32();
        Sample.ContinuityIdentity = Reader.U32();
        ReadI32Array(Reader, Sample.GeodeticQ28Q12, 3);
        Sample.AltitudeQ12Km = Reader.I32();
        Sample.MachQ24 = Reader.I32();
        Sample.DynamicPressureQ14Pa = Reader.I32();
        Sample.TotalMassQ21Kg = Reader.I32();
        Sample.MainPropellantQ21Kg = Reader.I32();
        Sample.RcsPropellantQ21Kg = Reader.I32();
        Sample.GimbalQ15[0] = Reader.I16();
        Sample.GimbalQ15[1] = Reader.I16();
        Reader.BytesInto(Sample.RcsPulses, 12);
        Sample.CommandFlags = Reader.U8();
        Sample.CommandDiscrete = Reader.U8();
        Sample.Alarms = Reader.U16();
        if (!Reader.IsValid() || Sample.Sequence == 0
            || (PreviousSequence != 0 && Sample.Sequence <= PreviousSequence)
            || !ValidFrame(Sample.ActiveFrame) || !ValidSegment(Sample.Segment)
            || Sample.FlightMode > 7 || Sample.TransitionCount > 4
            || SourceCount == 0 || SourceCount > 4
            || Sample.ContinuityIdentity == 0
            || (Sample.DiscontinuityMask & ~DiscontinuityMask) != 0)
        {
            OutError = TEXT("global display sample header is invalid");
            return false;
        }
        uint32 SeenSources = 0;
        Sample.Sources.Reserve(SourceCount);
        for (uint16 SourceIndex = 0; SourceIndex < SourceCount; ++SourceIndex)
        {
            FKsa64GlobalSourcePoseProduct Source;
            if (!ReadSource(
                    Reader,
                    PermittedSourceMask,
                    SeenSources,
                    Source,
                    OutError))
            {
                return false;
            }
            Sample.Sources.Add(Source);
        }
        PreviousSequence = Sample.Sequence;
        Out.Add(MoveTemp(Sample));
    }
    return Reader.IsFinished();
}

bool FKsa64GlobalDisplayCodec::DecodePath(
    const TArray<uint8>& Payload,
    uint32 PermittedSourceMask,
    FKsa64GlobalPathChunkProduct& Out,
    FString& OutError)
{
    Out = {};
    OutError.Reset();
    FReader Reader(Payload, "PGP1", OutError);
    if (!Reader.IsValid()) return false;
    Out.PathIdentity = Reader.U32();
    Out.Source = Reader.U8();
    Out.DisplayFrame = Reader.U8();
    Out.Lod = Reader.U8();
    Reader.Reserved(1);
    Out.Flags = Reader.U16();
    Out.ChunkIndex = Reader.U16();
    Out.ChunkCount = Reader.U16();
    const uint16 PointCount = Reader.U16();
    Out.ModelIdentity = Reader.U32();
    Out.EstimateIdentity = Reader.U32();
    Out.SourceChecksum = Reader.U32();
    Out.ContinuityIdentity = Reader.U32();
    const uint32 SourceBit = ValidSource(Out.Source) ? 1u << (Out.Source - 1u) : 0u;
    if (!Reader.IsValid() || Out.PathIdentity == 0 || SourceBit == 0
        || (PermittedSourceMask & SourceBit) == 0
        || !ValidFrame(Out.DisplayFrame) || Out.Lod < 1 || Out.Lod > 3
        || (Out.Flags & ~PathFlagMask) != 0
        || Out.ChunkCount == 0 || Out.ChunkIndex >= Out.ChunkCount
        || PointCount == 0 || PointCount > 4096
        || Out.ModelIdentity == 0 || Out.ContinuityIdentity == 0)
    {
        OutError = TEXT("global path header is invalid");
        return false;
    }
    Out.Points.Reserve(PointCount);
    uint32 PreviousRelease = 0;
    uint32 PreviousTime = 0;
    for (uint16 Index = 0; Index < PointCount; ++Index)
    {
        FKsa64GlobalPathPointProduct Point;
        Point.ReleaseEpoch = Reader.U32();
        Point.MissionTimeQ16 = Reader.U32();
        Point.Segment = Reader.U8();
        Reader.Reserved(1);
        Point.EventMask = Reader.U16();
        Point.AnchorIdentity = Reader.U32();
        ReadI32Array(Reader, Point.PositionQ12Km, 3);
        if (!Reader.IsValid() || !ValidSegment(Point.Segment)
            || (Out.DisplayFrame == 1 && Point.AnchorIdentity == 0)
            || (Out.DisplayFrame != 1 && Point.AnchorIdentity != 0)
            || (Index > 0 && (Point.ReleaseEpoch <= PreviousRelease
                || Point.MissionTimeQ16 <= PreviousTime)))
        {
            OutError = TEXT("global path point order or segment is invalid");
            return false;
        }
        PreviousRelease = Point.ReleaseEpoch;
        PreviousTime = Point.MissionTimeQ16;
        Out.Points.Add(Point);
    }
    return Reader.IsFinished();
}

bool FKsa64GlobalDisplayCodec::DecodeTransition(
    const TArray<uint8>& Payload,
    FKsa64GlobalTransitionProduct& Out,
    FString& OutError)
{
    Out = {};
    OutError.Reset();
    FReader Reader(Payload, "PGT1", OutError);
    if (!Reader.IsValid()) return false;
    Out.ReleaseEpoch = Reader.U32();
    Out.MissionTimeQ16 = Reader.U32();
    Out.FromFrame = Reader.U8();
    Out.ToFrame = Reader.U8();
    Out.FromSegment = Reader.U8();
    Out.ToSegment = Reader.U8();
    Out.Reason = Reader.U8();
    Reader.Reserved(3);
    Out.TransitionIdentity = Reader.U32();
    Out.TransformIdentity = Reader.U32();
    Out.AnchorIdentity = Reader.U32();
    Out.PositionMaxDeltaRaw = Reader.I32();
    Out.VelocityMaxDeltaRaw = Reader.I32();
    Out.AttitudeMaxDeltaRaw = Reader.I32();
    Out.AngularRateMaxDeltaRaw = Reader.I32();
    Out.Checksum = Reader.U32();
    if (!Reader.IsFinished()) return false;
    if (Out.ReleaseEpoch == 0 || !ValidFrame(Out.FromFrame)
        || !ValidFrame(Out.ToFrame) || Out.FromFrame == Out.ToFrame
        || !ValidSegment(Out.FromSegment) || !ValidSegment(Out.ToSegment)
        || Out.FromSegment == Out.ToSegment || Out.Reason == 0
        || Out.TransitionIdentity == 0 || Out.TransformIdentity == 0
        || Out.Checksum == 0)
    {
        OutError = TEXT("global transition identity or frame is invalid");
        return false;
    }
    return true;
}

bool FKsa64GlobalDisplayCodec::DecodeReplayIndex(
    const TArray<uint8>& Payload,
    FKsa64GlobalReplayIndexProduct& Out,
    FString& OutError)
{
    Out = {};
    OutError.Reset();
    FReader Reader(Payload, "PGI1", OutError);
    if (!Reader.IsValid()) return false;
    Out.IndexIdentity = Reader.U32();
    Out.SessionDefinitionIdentity = Reader.U32();
    Out.FirstRelease = Reader.U32();
    Out.LastRelease = Reader.U32();
    Out.TerminalDisposition = Reader.U8();
    Reader.BytesInto(Out.DispositionAxes, 6);
    Reader.Reserved(1);
    const uint16 EntryCount = Reader.U16();
    Reader.Reserved(2);
    if (!Reader.IsValid() || Out.IndexIdentity == 0
        || Out.SessionDefinitionIdentity == 0
        || Out.FirstRelease > Out.LastRelease
        || Out.TerminalDisposition > 5 || EntryCount > 512)
    {
        OutError = TEXT("global replay index header is invalid");
        return false;
    }
    uint32 PreviousRelease = Out.FirstRelease;
    uint32 PreviousTime = 0;
    Out.Entries.Reserve(EntryCount);
    for (uint16 Index = 0; Index < EntryCount; ++Index)
    {
        FKsa64GlobalReplayEntryProduct Entry;
        Entry.ReleaseEpoch = Reader.U32();
        Entry.MissionTimeQ16 = Reader.U32();
        Entry.Kind = Reader.U8();
        Reader.Reserved(3);
        Entry.SourceIdentity = Reader.U32();
        Entry.EventIdentity = Reader.U32();
        Entry.DetailIdentity = Reader.U32();
        if (!Reader.IsValid() || Entry.Kind < 1 || Entry.Kind > 5
            || Entry.ReleaseEpoch < Out.FirstRelease
            || Entry.ReleaseEpoch > Out.LastRelease
            || Entry.EventIdentity == 0
            || (Index > 0 && (Entry.ReleaseEpoch < PreviousRelease
                || Entry.MissionTimeQ16 < PreviousTime)))
        {
            OutError = TEXT("global replay bookmark is invalid or out of order");
            return false;
        }
        PreviousRelease = Entry.ReleaseEpoch;
        PreviousTime = Entry.MissionTimeQ16;
        Out.Entries.Add(Entry);
    }
    return Reader.IsFinished();
}
