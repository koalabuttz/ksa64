export const GLOBAL_DISPLAY_V1_CAPABILITY = 1n << 8n;
export const GLOBAL_DISPLAY_MAX_SOURCES = 4;
export const GLOBAL_DISPLAY_MAX_PATH_POINTS = 4096;
export const GLOBAL_DISPLAY_MAX_REPLAY_ENTRIES = 512;
export const GLOBAL_DISPLAY_SOURCE_SIM_TRUTH = 1 << 3;
export const GLOBAL_DISPLAY_PUBLIC_SOURCE_MASK = (1 << 3) - 1;
export const GLOBAL_DISPLAY_SOURCE_MASK = (1 << 4) - 1;
export const GLOBAL_POSE_VALID_ACTIVE_POSITION = 1 << 0;
export const GLOBAL_POSE_VALID_ACTIVE_VELOCITY = 1 << 1;
export const GLOBAL_POSE_VALID_ACTIVE_ATTITUDE = 1 << 2;
export const GLOBAL_POSE_VALID_ANGULAR_RATE = 1 << 3;
export const GLOBAL_POSE_VALID_ECEF_POSITION = 1 << 4;
export const GLOBAL_POSE_VALID_ECEF_VELOCITY = 1 << 5;
export const GLOBAL_POSE_VALID_ECEF_ATTITUDE = 1 << 6;
export const GLOBAL_POSE_VALID_GCRF_POSITION = 1 << 7;
export const GLOBAL_POSE_VALID_GCRF_VELOCITY = 1 << 8;
export const GLOBAL_POSE_VALID_GCRF_ATTITUDE = 1 << 9;
export const GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION = 1 << 10;
export const GLOBAL_POSE_VALID_LAUNCH_ENU_VELOCITY = 1 << 11;
export const GLOBAL_POSE_VALID_LAUNCH_ENU_ATTITUDE = 1 << 12;
export const GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION = 1 << 13;
export const GLOBAL_POSE_VALID_RECOVERY_ENU_VELOCITY = 1 << 14;
export const GLOBAL_POSE_VALID_RECOVERY_ENU_ATTITUDE = 1 << 15;
export const GLOBAL_POSE_VALID_MASK = (1 << 16) - 1;
export const GLOBAL_DISCONTINUITY_MASK = (1 << 9) - 1;
export const GLOBAL_PATH_FLAG_STALE = 1 << 0;
export const GLOBAL_PATH_FLAG_INCOMPLETE = 1 << 1;
export const GLOBAL_PATH_FLAG_TERMINAL = 1 << 2;
export const GLOBAL_PATH_FLAG_RESYNC_REQUIRED = 1 << 3;
export const GLOBAL_PATH_FLAG_MASK = (1 << 4) - 1;

export type GlobalDisplayFrameId = 1 | 2 | 3;
export type GlobalDisplaySegment = 1 | 2 | 3 | 4 | 5;
export type GlobalDisplaySourceId = 1 | 2 | 3 | 4;
export type GlobalDisplayPathLod = 1 | 2 | 3;
export type GlobalDisplayReplayEntryKind = 1 | 2 | 3 | 4 | 5;
export type I16x2 = readonly [number, number];
export type I32x3 = readonly [number, number, number];
export type I32x4 = readonly [number, number, number, number];

export interface GlobalDisplayAnchorV1 { readonly identity: number; readonly geodeticQ28Q12: I32x3; }
export interface GlobalDisplayDefinitionV1 {
  readonly displayIdentity: number; readonly earthIdentity: number; readonly transformIdentity: number;
  readonly missionIdentity: number; readonly epochUnixDay: number; readonly epochTaiMinusUtc: number;
  readonly semiMajorQ12Km: number; readonly semiMinorQ12Km: number; readonly inverseFlatteningQ20: number;
  readonly launchAnchor: GlobalDisplayAnchorV1; readonly recoveryAnchor: GlobalDisplayAnchorV1;
  readonly availableSourceMask: number; readonly availableFrameMask: number; readonly cameraDomainMask: number;
}
export interface GlobalDisplayResolvedPoseV1 {
  readonly positionQ12Km: I32x3; readonly velocityQ24KmS: I32x3; readonly attitudeQ30: I32x4;
}
export interface GlobalDisplaySourcePoseV1 {
  readonly source: GlobalDisplaySourceId; readonly activeFrame: GlobalDisplayFrameId; readonly validityMask: number;
  readonly modelIdentity: number; readonly estimateIdentity: number; readonly checksum: number; readonly ageReleases: number;
  readonly active: GlobalDisplayResolvedPoseV1; readonly ecef: GlobalDisplayResolvedPoseV1;
  readonly gcrf: GlobalDisplayResolvedPoseV1; readonly launchEnu: GlobalDisplayResolvedPoseV1;
  readonly recoveryEnu: GlobalDisplayResolvedPoseV1; readonly angularRateQ24: I32x3;
}
export interface GlobalDisplaySampleV1 {
  readonly sequence: bigint; readonly releaseEpoch: number; readonly missionTimeQ16: number;
  readonly activeFrame: GlobalDisplayFrameId; readonly segment: GlobalDisplaySegment; readonly flightMode: number;
  readonly transitionCount: number; readonly eventMask: number; readonly discontinuityMask: number;
  readonly continuityIdentity: number; readonly geodeticQ28Q12: I32x3; readonly altitudeQ12Km: number;
  readonly machQ24: number; readonly dynamicPressureQ14Pa: number; readonly totalMassQ21Kg: number;
  readonly mainPropellantQ21Kg: number; readonly rcsPropellantQ21Kg: number; readonly gimbalQ15: I16x2;
  readonly rcsPulses: Uint8Array; readonly commandFlags: number; readonly commandDiscrete: number;
  readonly alarms: number; readonly sources: readonly GlobalDisplaySourcePoseV1[];
}
export interface GlobalDisplayPathPointV1 {
  readonly releaseEpoch: number; readonly missionTimeQ16: number; readonly segment: GlobalDisplaySegment;
  readonly eventMask: number; readonly positionQ12Km: I32x3;
}
export interface GlobalDisplayPathChunkV1 {
  readonly pathIdentity: number; readonly source: GlobalDisplaySourceId; readonly displayFrame: GlobalDisplayFrameId;
  readonly lod: GlobalDisplayPathLod; readonly flags: number; readonly modelIdentity: number;
  readonly estimateIdentity: number; readonly sourceChecksum: number; readonly continuityIdentity: number;
  readonly chunkIndex: number; readonly chunkCount: number; readonly points: readonly GlobalDisplayPathPointV1[];
}
export interface GlobalDisplayTransitionV1 {
  readonly releaseEpoch: number; readonly missionTimeQ16: number; readonly fromFrame: GlobalDisplayFrameId;
  readonly toFrame: GlobalDisplayFrameId; readonly fromSegment: GlobalDisplaySegment; readonly toSegment: GlobalDisplaySegment;
  readonly reason: number; readonly transitionIdentity: number; readonly transformIdentity: number; readonly anchorIdentity: number;
  readonly positionDeltaQ12Km: I32x3; readonly velocityDeltaQ24KmS: I32x3; readonly attitudeDeltaQ30: number;
  readonly angularRateDeltaQ24: I32x3; readonly checksum: number;
}
export interface GlobalDisplayReplayEntryV1 {
  readonly releaseEpoch: number; readonly missionTimeQ16: number; readonly kind: GlobalDisplayReplayEntryKind;
  readonly sourceIdentity: number; readonly eventIdentity: number; readonly detailIdentity: number;
}
export interface GlobalReplayIndexV1 {
  readonly indexIdentity: number; readonly sessionDefinitionIdentity: number; readonly firstRelease: number;
  readonly lastRelease: number; readonly terminalDisposition: number; readonly dispositionAxes: Uint8Array;
  readonly entries: readonly GlobalDisplayReplayEntryV1[];
}

class Reader {
  private readonly view: DataView; private at = 12;
  constructor(private readonly payload: Uint8Array, magic: string) {
    if (payload.byteLength < 12) throw new Error("truncated global display payload");
    this.view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    if (new TextDecoder().decode(payload.subarray(0, 4)) !== magic) throw new Error("global display payload magic mismatch");
    if (this.view.getUint16(4, true) !== 1 || this.view.getUint16(6, true) !== 12) throw new Error("unsupported global display payload version");
    if (this.view.getUint32(8, true) !== payload.byteLength) throw new Error("global display payload length mismatch");
  }
  u8(): number { return this.take(1)[0] ?? 0; }
  u16(): number { const at = this.claim(2); return this.view.getUint16(at, true); }
  i16(): number { const at = this.claim(2); return this.view.getInt16(at, true); }
  u32(): number { const at = this.claim(4); return this.view.getUint32(at, true); }
  i32(): number { const at = this.claim(4); return this.view.getInt32(at, true); }
  u64(): bigint { const at = this.claim(8); return this.view.getBigUint64(at, true); }
  i32x3(): I32x3 { return [this.i32(), this.i32(), this.i32()]; }
  i32x4(): I32x4 { return [this.i32(), this.i32(), this.i32(), this.i32()]; }
  bytes(count: number): Uint8Array { return this.take(count).slice(); }
  reserved(count: number): void { if (this.take(count).some((value) => value !== 0)) throw new Error("nonzero global display reserved byte"); }
  finish(): void { if (this.at !== this.payload.byteLength) throw new Error("trailing global display bytes"); }
  private claim(count: number): number { const start = this.at; if (start + count > this.payload.byteLength) throw new Error("truncated global display payload"); this.at += count; return start; }
  private take(count: number): Uint8Array { const start = this.claim(count); return this.payload.subarray(start, start + count); }
}
function frame(value: number): GlobalDisplayFrameId { if (value < 1 || value > 3) throw new Error("invalid global display frame"); return value as GlobalDisplayFrameId; }
function segment(value: number): GlobalDisplaySegment { if (value < 1 || value > 5) throw new Error("invalid global display segment"); return value as GlobalDisplaySegment; }
function source(value: number, role: number): GlobalDisplaySourceId {
  if (value < 1 || value > 4 || (value === 4 && role !== 5)) throw new Error("private truth crossed an unauthorized role boundary");
  return value as GlobalDisplaySourceId;
}
function resolvedPose(reader: Reader): GlobalDisplayResolvedPoseV1 {
  return { positionQ12Km: reader.i32x3(), velocityQ24KmS: reader.i32x3(), attitudeQ30: reader.i32x4() };
}
function sourcePose(reader: Reader, role: number): GlobalDisplaySourcePoseV1 {
  const poseSource = source(reader.u8(), role); const activeFrame = frame(reader.u8()); reader.reserved(2);
  const validityMask = reader.u32(); const modelIdentity = reader.u32(); const estimateIdentity = reader.u32();
  const checksum = reader.u32(); const ageReleases = reader.u32(); const active = resolvedPose(reader);
  const ecef = resolvedPose(reader); const gcrf = resolvedPose(reader); const launchEnu = resolvedPose(reader);
  const recoveryEnu = resolvedPose(reader); const angularRateQ24 = reader.i32x3();
  if (modelIdentity === 0 || validityMask === 0 || (validityMask & ~GLOBAL_POSE_VALID_MASK) !== 0 ||
      (validityMask & GLOBAL_POSE_VALID_ACTIVE_POSITION) === 0 ||
      ((validityMask & (1 << 2)) !== 0 && active.attitudeQ30.every((value) => value === 0))) {
    throw new Error("invalid global display source pose");
  }
  return { source: poseSource, activeFrame, validityMask, modelIdentity, estimateIdentity, checksum,
    ageReleases, active, ecef, gcrf, launchEnu, recoveryEnu, angularRateQ24 };
}
function anchor(reader: Reader): GlobalDisplayAnchorV1 { return { identity: reader.u32(), geodeticQ28Q12: reader.i32x3() }; }

export function decodeGlobalDisplayDefinitionPayload(bytes: Uint8Array, role: number): GlobalDisplayDefinitionV1 {
  const reader = new Reader(bytes, "PGD1");
  const displayIdentity = reader.u32(); const earthIdentity = reader.u32(); const transformIdentity = reader.u32();
  const missionIdentity = reader.u32(); const epochUnixDay = reader.i32(); const epochTaiMinusUtc = reader.i16(); reader.reserved(2);
  const semiMajorQ12Km = reader.i32(); const semiMinorQ12Km = reader.i32(); const inverseFlatteningQ20 = reader.i32();
  const launchAnchor = anchor(reader); const recoveryAnchor = anchor(reader); const availableSourceMask = reader.u32();
  const availableFrameMask = reader.u8(); reader.reserved(1); const cameraDomainMask = reader.u16(); reader.finish();
  if (displayIdentity === 0 || earthIdentity === 0 || transformIdentity === 0 || missionIdentity === 0 ||
      launchAnchor.identity === 0 || recoveryAnchor.identity === 0 || semiMajorQ12Km <= 0 || semiMinorQ12Km <= 0 ||
      inverseFlatteningQ20 <= 0 || availableSourceMask === 0 || (availableSourceMask & ~GLOBAL_DISPLAY_SOURCE_MASK) !== 0 ||
      availableFrameMask === 0 || (availableFrameMask & ~7) !== 0 || cameraDomainMask === 0 ||
      (role !== 5 && (availableSourceMask & GLOBAL_DISPLAY_SOURCE_SIM_TRUTH) !== 0)) throw new Error("invalid global display definition");
  return { displayIdentity, earthIdentity, transformIdentity, missionIdentity, epochUnixDay, epochTaiMinusUtc,
    semiMajorQ12Km, semiMinorQ12Km, inverseFlatteningQ20, launchAnchor, recoveryAnchor,
    availableSourceMask, availableFrameMask, cameraDomainMask };
}
function sample(reader: Reader, role: number): GlobalDisplaySampleV1 {
  const sequence = reader.u64(); const releaseEpoch = reader.u32(); const missionTimeQ16 = reader.u32();
  const activeFrame = frame(reader.u8()); const activeSegment = segment(reader.u8()); const flightMode = reader.u8();
  const transitionCount = reader.u8(); const eventMask = reader.u16(); const sourceCount = reader.u16();
  if (sourceCount === 0 || sourceCount > GLOBAL_DISPLAY_MAX_SOURCES) throw new Error("invalid global display source count");
  const discontinuityMask = reader.u32(); const continuityIdentity = reader.u32(); const geodeticQ28Q12 = reader.i32x3();
  const altitudeQ12Km = reader.i32(); const machQ24 = reader.i32(); const dynamicPressureQ14Pa = reader.i32();
  const totalMassQ21Kg = reader.i32(); const mainPropellantQ21Kg = reader.i32(); const rcsPropellantQ21Kg = reader.i32();
  const gimbalQ15: I16x2 = [reader.i16(), reader.i16()]; const rcsPulses = reader.bytes(12);
  const commandFlags = reader.u8(); const commandDiscrete = reader.u8(); const alarms = reader.u16();
  const sources: GlobalDisplaySourcePoseV1[] = []; let seen = 0;
  for (let index = 0; index < sourceCount; index += 1) { const value = sourcePose(reader, role); const bit = 1 << (value.source - 1); if ((seen & bit) !== 0) throw new Error("duplicate global display source"); seen |= bit; sources.push(value); }
  if (sequence === 0n || continuityIdentity === 0 || flightMode > 7 || transitionCount > 4 ||
      (discontinuityMask & ~GLOBAL_DISCONTINUITY_MASK) !== 0) throw new Error("invalid global display sample");
  return { sequence, releaseEpoch, missionTimeQ16, activeFrame, segment: activeSegment, flightMode, transitionCount,
    eventMask, discontinuityMask, continuityIdentity, geodeticQ28Q12, altitudeQ12Km, machQ24,
    dynamicPressureQ14Pa, totalMassQ21Kg, mainPropellantQ21Kg, rcsPropellantQ21Kg, gimbalQ15,
    rcsPulses, commandFlags, commandDiscrete, alarms, sources };
}
export function decodeGlobalDisplaySamplesPayload(bytes: Uint8Array, role: number): readonly GlobalDisplaySampleV1[] {
  const reader = new Reader(bytes, "PGS1"); const count = reader.u16(); reader.reserved(2);
  if (count === 0) throw new Error("empty global display sample batch");
  const values: GlobalDisplaySampleV1[] = []; let prior = 0n;
  for (let index = 0; index < count; index += 1) { const value = sample(reader, role); if (prior !== 0n && value.sequence <= prior) throw new Error("invalid global display sample sequence"); prior = value.sequence; values.push(value); }
  reader.finish(); return values;
}

export function decodeGlobalDisplayPathPayload(bytes: Uint8Array, role: number): GlobalDisplayPathChunkV1 {
  const reader = new Reader(bytes, "PGP1"); const pathIdentity = reader.u32(); const pathSource = source(reader.u8(), role);
  const displayFrame = frame(reader.u8()); const lodRaw = reader.u8(); if (lodRaw < 1 || lodRaw > 3) throw new Error("invalid global display path LOD");
  reader.reserved(1); const flags = reader.u16(); const chunkIndex = reader.u16(); const chunkCount = reader.u16();
  const count = reader.u16(); const modelIdentity = reader.u32(); const estimateIdentity = reader.u32();
  const sourceChecksum = reader.u32(); const continuityIdentity = reader.u32();
  if (count === 0 || count > GLOBAL_DISPLAY_MAX_PATH_POINTS) throw new Error("invalid global display path length");
  const points: GlobalDisplayPathPointV1[] = []; let priorRelease = -1; let priorTime = -1;
  for (let index = 0; index < count; index += 1) {
    const releaseEpoch = reader.u32(); const missionTimeQ16 = reader.u32(); const pointSegment = segment(reader.u8());
    reader.reserved(1); const eventMask = reader.u16(); const positionQ12Km = reader.i32x3();
    if (releaseEpoch <= priorRelease || missionTimeQ16 <= priorTime) throw new Error("invalid global display path sequence");
    priorRelease = releaseEpoch; priorTime = missionTimeQ16; points.push({ releaseEpoch, missionTimeQ16, segment: pointSegment, eventMask, positionQ12Km });
  }
  reader.finish();
  if (pathIdentity === 0 || modelIdentity === 0 || continuityIdentity === 0 || chunkCount === 0 ||
      chunkIndex >= chunkCount || (flags & ~GLOBAL_PATH_FLAG_MASK) !== 0) throw new Error("invalid global display path");
  return { pathIdentity, source: pathSource, displayFrame, lod: lodRaw as GlobalDisplayPathLod, flags,
    modelIdentity, estimateIdentity, sourceChecksum, continuityIdentity, chunkIndex, chunkCount, points };
}
export function decodeGlobalDisplayTransitionPayload(bytes: Uint8Array): GlobalDisplayTransitionV1 {
  const reader = new Reader(bytes, "PGT1"); const releaseEpoch = reader.u32(); const missionTimeQ16 = reader.u32();
  const fromFrame = frame(reader.u8()); const toFrame = frame(reader.u8()); const fromSegment = segment(reader.u8());
  const toSegment = segment(reader.u8()); const reason = reader.u8(); reader.reserved(3);
  const value: GlobalDisplayTransitionV1 = { releaseEpoch, missionTimeQ16, fromFrame, toFrame, fromSegment, toSegment, reason,
    transitionIdentity: reader.u32(), transformIdentity: reader.u32(), anchorIdentity: reader.u32(),
    positionDeltaQ12Km: reader.i32x3(), velocityDeltaQ24KmS: reader.i32x3(), attitudeDeltaQ30: reader.i32(),
    angularRateDeltaQ24: reader.i32x3(), checksum: reader.u32() };
  reader.finish();
  if (releaseEpoch === 0 || fromFrame === toFrame || fromSegment === toSegment || reason === 0 ||
      value.transitionIdentity === 0 || value.transformIdentity === 0 || value.checksum === 0) throw new Error("invalid global display transition");
  return value;
}
export function decodeGlobalReplayIndexPayload(bytes: Uint8Array): GlobalReplayIndexV1 {
  const reader = new Reader(bytes, "PGI1"); const indexIdentity = reader.u32(); const sessionDefinitionIdentity = reader.u32();
  const firstRelease = reader.u32(); const lastRelease = reader.u32(); const terminalDisposition = reader.u8();
  const dispositionAxes = reader.bytes(6); reader.reserved(1); const count = reader.u16(); reader.reserved(2);
  if (count > GLOBAL_DISPLAY_MAX_REPLAY_ENTRIES) throw new Error("global replay index exceeds bound");
  const entries: GlobalDisplayReplayEntryV1[] = []; let priorRelease = firstRelease; let priorTime = 0;
  for (let index = 0; index < count; index += 1) {
    const releaseEpoch = reader.u32(); const missionTimeQ16 = reader.u32(); const kindRaw = reader.u8();
    if (kindRaw < 1 || kindRaw > 5) throw new Error("invalid global replay entry kind");
    reader.reserved(3); const value: GlobalDisplayReplayEntryV1 = { releaseEpoch, missionTimeQ16,
      kind: kindRaw as GlobalDisplayReplayEntryKind, sourceIdentity: reader.u32(), eventIdentity: reader.u32(), detailIdentity: reader.u32() };
    if (releaseEpoch < firstRelease || releaseEpoch > lastRelease || value.eventIdentity === 0 || releaseEpoch < priorRelease || missionTimeQ16 < priorTime) throw new Error("invalid global replay entry sequence");
    priorRelease = releaseEpoch; priorTime = missionTimeQ16; entries.push(value);
  }
  reader.finish();
  if (indexIdentity === 0 || sessionDefinitionIdentity === 0 || firstRelease > lastRelease || terminalDisposition > 5) throw new Error("invalid global replay index");
  return { indexIdentity, sessionDefinitionIdentity, firstRelease, lastRelease, terminalDisposition, dispositionAxes, entries };
}
