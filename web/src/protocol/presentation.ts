import { Kps1MessageKind, type Kps1Frame } from "./kps1";
import {
  decodeGlobalDisplayDefinitionPayload, decodeGlobalDisplayPathPayload,
  decodeGlobalDisplaySamplesPayload, decodeGlobalDisplayTransitionPayload,
  decodeGlobalReplayIndexPayload, type GlobalDisplayDefinitionV1,
  type GlobalDisplayPathChunkV1, type GlobalDisplaySampleV1,
  type GlobalDisplayTransitionV1, type GlobalReplayIndexV1,
} from "./globalDisplay";

export type NumericRole = 1 | 2 | 3 | 4 | 5 | 6;
export type NumericLifecycle = 1 | 2 | 3 | 4 | 5 | 6 | 7;
export type NumericPace = 1 | 2 | 3 | 4;
export type ActionOperation = 1 | 2 | 3 | 4;

export interface NavigationView {
  readonly position: readonly [number, number, number];
  readonly velocity: readonly [number, number, number];
  readonly checksum: number;
}

export interface PredictionSummaryView {
  readonly identity: number;
  readonly checksum: number;
  readonly sourceEstimateIdentity: number;
  readonly frameIdentity: number;
  readonly apogeeQ12Km: number;
  readonly perigeeQ12Km: number;
  readonly timeToApogeeQ16: number;
  readonly timeToImpactQ16: number;
  readonly impactPosition: readonly [number, number, number];
  readonly terminalReason: number;
}

export interface OperationalSnapshot {
  readonly presentationModelIdentity: number;
  readonly definitionIdentity: number;
  readonly publicationSequence: bigint;
  readonly validityMask: bigint;
  readonly role: NumericRole;
  readonly lifecycle: NumericLifecycle;
  readonly pace: NumericPace;
  readonly gnssState: number;
  readonly safe: boolean;
  readonly truthPresent: boolean;
  readonly releaseEpoch: number;
  readonly releasePeriodMicros: number;
  readonly frameIdentity: number;
  readonly missionTimeQ16: number;
  readonly onboard: NavigationView;
  readonly ground: NavigationView;
  readonly prediction: PredictionSummaryView;
  readonly flightChecksum: number;
  readonly commandChecksum: number;
  readonly procedureChain: number;
  readonly journalChain: number;
  readonly actionChain: number;
  readonly stagedLoadIdentity: number;
  readonly actionCount: number;
  readonly rejectedLoads: number;
}

export interface ProcedurePredicate {
  readonly identity: number;
  readonly satisfied: boolean;
}

export interface ProcedureView {
  readonly identity: number;
  readonly activeStep: number;
  readonly stepCount: number;
  readonly state: number;
  readonly hintsAvailable: boolean;
  readonly enteredEpoch: number;
  readonly deadlineEpoch: number;
  readonly title: string;
  readonly instruction: string;
  readonly predicates: readonly ProcedurePredicate[];
}

export interface DispositionView {
  readonly overall: number;
  readonly objective: number;
  readonly vehicle: number;
  readonly procedure: number;
  readonly operator: number;
  readonly avionics: number;
  readonly evidence: number;
  readonly reasonIdentity: number;
}

export interface ActionProposalView {
  readonly proposalIdentity: number;
  readonly loadIdentity: number;
  readonly loadType: number;
  readonly permittedOperations: number;
  readonly stageEpoch: number;
  readonly earliestCommitEpoch: number;
  readonly activationEpoch: number;
  readonly expiresEpoch: number;
  readonly payloadChecksum: number;
  readonly completedEventMask: number;
  readonly label: string;
}

export interface ActionReceiptView {
  readonly publicationSequence: bigint;
  readonly proposalIdentity: number;
  readonly loadIdentity: number;
  readonly controlIdentity: number;
  readonly receiptEpoch: number;
  readonly effectiveEpoch: number;
  readonly state: number;
  readonly reason: number;
  readonly accepted: boolean;
  readonly operation: ActionOperation;
  readonly receiptChecksum: number;
}

export interface TimelineEventView {
  readonly sequence: bigint;
  readonly releaseEpoch: number;
  readonly sourceIdentity: number;
  readonly severity: number;
  readonly eventIdentity: number;
  readonly detailIdentity: number;
  readonly label: string;
}

export interface PresentationEventView {
  readonly sequence: bigint;
  readonly releaseEpoch: number;
  readonly kind: number;
  readonly detailIdentity: number;
}

export interface ReleaseSampleView {
  readonly sequence: bigint;
  readonly validityMask: bigint;
  readonly releaseEpoch: number;
  readonly missionTimeQ16: number;
  readonly frameIdentity: number;
  readonly onboardPosition: readonly [number, number, number];
  readonly onboardVelocity: readonly [number, number, number];
  readonly groundPosition: readonly [number, number, number];
  readonly groundVelocity: readonly [number, number, number];
  readonly predictedImpact: readonly [number, number, number];
  readonly predictedApogeeQ12Km: number;
  readonly altitudeQ12Km: number;
  readonly speedQ24KmS: number;
  readonly downrangeQ12Km: number;
  readonly crossrangeQ12Km: number;
}

export interface PredictionPathPoint {
  readonly releaseEpoch: number;
  readonly frameIdentity: number;
  readonly position: readonly [number, number, number];
  readonly altitudeQ12Km: number;
  readonly downrangeQ12Km: number;
  readonly crossrangeQ12Km: number;
}

export interface PredictionPathView {
  readonly pathIdentity: number;
  readonly productIdentity: number;
  readonly modelIdentity: number;
  readonly sourceEstimateIdentity: number;
  readonly sourceEstimateChecksum: number;
  readonly sourceEpoch: number;
  readonly generationEpoch: number;
  readonly frameIdentity: number;
  readonly terminalReason: number;
  readonly cadenceReleases: number;
  readonly pathChecksum: number;
  readonly points: readonly PredictionPathPoint[];
}

export interface TransportStatusView {
  readonly staleness: number;
  readonly workerState: number;
  readonly finalizationState: number;
  readonly commandCapacity: number;
  readonly commandsPending: number;
  readonly eventCapacity: number;
  readonly eventsPending: number;
  readonly timelineCapacity: number;
  readonly timelinePending: number;
  readonly sampleCapacity: number;
  readonly samplesPending: number;
  readonly eventOverflow: boolean;
  readonly timelineOverflow: boolean;
  readonly sampleOverflow: boolean;
  readonly lastCommandResult: number;
}

export interface EvidenceMetadata {
  readonly evidenceIdentity: number;
  readonly evidenceCrc32: number;
  readonly totalLength: bigint;
  readonly chunkLength: number;
  readonly chunkCount: number;
  readonly complete: boolean;
  readonly contentKind: number;
}

export interface EvidenceChunk {
  readonly evidenceIdentity: number;
  readonly chunkIndex: number;
  readonly chunkCount: number;
  readonly logicalOffset: bigint;
  readonly bytes: Uint8Array;
}

export interface PresentationErrorView {
  readonly code: number;
  readonly fatal: boolean;
  readonly detailIdentity: number;
  readonly message: string;
}

export type DecodedPresentationPayload =
  | { readonly kind: "snapshot"; readonly value: OperationalSnapshot }
  | { readonly kind: "procedure"; readonly value: ProcedureView }
  | { readonly kind: "disposition"; readonly value: DispositionView }
  | { readonly kind: "action-proposal"; readonly value: ActionProposalView }
  | { readonly kind: "action-receipt"; readonly value: ActionReceiptView }
  | { readonly kind: "timeline"; readonly value: TimelineEventView }
  | { readonly kind: "events"; readonly value: readonly PresentationEventView[] }
  | { readonly kind: "samples"; readonly value: readonly ReleaseSampleView[] }
  | { readonly kind: "prediction-path"; readonly value: PredictionPathView }
  | { readonly kind: "transport"; readonly value: TransportStatusView }
  | { readonly kind: "global-definition"; readonly value: GlobalDisplayDefinitionV1 }
  | { readonly kind: "global-samples"; readonly value: readonly GlobalDisplaySampleV1[] }
  | { readonly kind: "global-path"; readonly value: GlobalDisplayPathChunkV1 }
  | { readonly kind: "global-transition"; readonly value: GlobalDisplayTransitionV1 }
  | { readonly kind: "global-replay-index"; readonly value: GlobalReplayIndexV1 }
  | { readonly kind: "evidence"; readonly value: EvidenceMetadata }
  | { readonly kind: "evidence-chunk"; readonly value: EvidenceChunk }
  | { readonly kind: "error"; readonly value: PresentationErrorView }
  | { readonly kind: "unhandled"; readonly messageKind: Kps1MessageKind; readonly bytes: Uint8Array };

class PayloadReader {
  private readonly view: DataView;
  private at = 12;

  constructor(private readonly payload: Uint8Array, magic: string) {
    if (payload.byteLength < 12) throw new Error("truncated presentation payload");
    this.view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    const found = new TextDecoder().decode(payload.subarray(0, 4));
    if (found !== magic) throw new Error("presentation payload magic mismatch");
    if (this.view.getUint16(4, true) !== 1 || this.view.getUint16(6, true) !== 12) {
      throw new Error("unsupported presentation payload version");
    }
    if (this.view.getUint32(8, true) !== payload.byteLength) {
      throw new Error("presentation payload length mismatch");
    }
  }

  u8(): number { return this.take(1)[0] ?? 0; }
  bool(): boolean {
    const value = this.u8();
    if (value > 1) throw new Error("invalid presentation boolean");
    return value === 1;
  }
  u16(): number { const at = this.claim(2); return this.view.getUint16(at, true); }
  u32(): number { const at = this.claim(4); return this.view.getUint32(at, true); }
  i32(): number { const at = this.claim(4); return this.view.getInt32(at, true); }
  u64(): bigint { const at = this.claim(8); return this.view.getBigUint64(at, true); }
  i32x3(): [number, number, number] { return [this.i32(), this.i32(), this.i32()]; }
  bytes(count: number): Uint8Array { return this.take(count); }
  reserved(count: number): void {
    if (this.take(count).some((value) => value !== 0)) throw new Error("nonzero reserved payload byte");
  }
  text(maximum = 512): string {
    const length = this.u16();
    if (length > maximum) throw new Error("presentation text exceeds bound");
    return new TextDecoder("utf-8", { fatal: true }).decode(this.take(length));
  }
  finish(): void { if (this.at !== this.payload.byteLength) throw new Error("trailing presentation bytes"); }

  private claim(count: number): number {
    const start = this.at;
    if (start + count > this.payload.byteLength) throw new Error("truncated presentation payload");
    this.at += count;
    return start;
  }
  private take(count: number): Uint8Array {
    const start = this.claim(count);
    return this.payload.subarray(start, start + count);
  }
}

function navigation(reader: PayloadReader): NavigationView {
  return { position: reader.i32x3(), velocity: reader.i32x3(), checksum: reader.u32() };
}

function prediction(reader: PayloadReader): PredictionSummaryView {
  const value: PredictionSummaryView = {
    identity: reader.u32(), checksum: reader.u32(), sourceEstimateIdentity: reader.u32(),
    frameIdentity: reader.u32(), apogeeQ12Km: reader.i32(), perigeeQ12Km: reader.i32(),
    timeToApogeeQ16: reader.u32(), timeToImpactQ16: reader.u32(),
    impactPosition: reader.i32x3(), terminalReason: reader.u8(),
  };
  reader.reserved(3);
  if (value.terminalReason > 5) throw new Error("invalid prediction terminal reason");
  return value;
}

function decodeSnapshot(bytes: Uint8Array, expectedRole?: NumericRole): OperationalSnapshot {
  const reader = new PayloadReader(bytes, "POS1");
  const value: OperationalSnapshot = {
    presentationModelIdentity: reader.u32(), definitionIdentity: reader.u32(),
    publicationSequence: reader.u64(), validityMask: reader.u64(),
    role: reader.u8() as NumericRole, lifecycle: reader.u8() as NumericLifecycle,
    pace: reader.u8() as NumericPace, gnssState: reader.u8(), safe: reader.bool(),
    truthPresent: reader.bool(), releaseEpoch: 0, releasePeriodMicros: 0, frameIdentity: 0,
    missionTimeQ16: 0, onboard: { position: [0, 0, 0], velocity: [0, 0, 0], checksum: 0 },
    ground: { position: [0, 0, 0], velocity: [0, 0, 0], checksum: 0 },
    prediction: { identity: 0, checksum: 0, sourceEstimateIdentity: 0, frameIdentity: 0,
      apogeeQ12Km: 0, perigeeQ12Km: 0, timeToApogeeQ16: 0, timeToImpactQ16: 0,
      impactPosition: [0, 0, 0], terminalReason: 0 },
    flightChecksum: 0, commandChecksum: 0, procedureChain: 0, journalChain: 0,
    actionChain: 0, stagedLoadIdentity: 0, actionCount: 0, rejectedLoads: 0,
  };
  reader.reserved(2);
  Object.assign(value, {
    releaseEpoch: reader.u32(), releasePeriodMicros: reader.u32(), frameIdentity: reader.u32(),
    missionTimeQ16: reader.u32(), onboard: navigation(reader), ground: navigation(reader),
    prediction: prediction(reader), flightChecksum: reader.u32(), commandChecksum: reader.u32(),
    procedureChain: reader.u32(), journalChain: reader.u32(), actionChain: reader.u32(),
    stagedLoadIdentity: reader.u32(), actionCount: reader.u32(), rejectedLoads: reader.u16(),
  });
  reader.reserved(2);
  if (value.truthPresent) {
    if (value.role !== 5) throw new Error("private truth crossed an unauthorized role boundary");
    reader.i32x3(); reader.i32x3();
    reader.i32(); reader.i32(); reader.i32(); reader.i32();
    reader.u32(); reader.u32();
  }
  reader.finish();
  if (expectedRole !== undefined && value.role !== expectedRole) throw new Error("immutable role mismatch");
  const allowedValidity = ((1n << 9n) - 1n) | (1n << 63n);
  if (value.role < 1 || value.role > 6 || value.lifecycle < 1 || value.lifecycle > 7 || value.pace < 1 || value.pace > 4 ||
      value.gnssState > 2 || value.publicationSequence === 0n || (value.validityMask & ~allowedValidity) !== 0n) {
    throw new Error("invalid snapshot enum, mask, or sequence");
  }
  if (value.truthPresent !== ((value.validityMask & (1n << 63n)) !== 0n)) throw new Error("snapshot truth validity mismatch");
  if ((value.validityMask & (1n << 3n)) !== 0n && (value.prediction.identity === 0 || value.prediction.terminalReason === 0)) {
    throw new Error("invalid prediction identity");
  }
  return value;
}

function decodeProcedure(bytes: Uint8Array): ProcedureView {
  const reader = new PayloadReader(bytes, "PPR1");
  const identity = reader.u32(); const activeStep = reader.u16(); const stepCount = reader.u16();
  const state = reader.u8(); const hintsAvailable = reader.bool(); reader.reserved(2);
  const enteredEpoch = reader.u32(); const deadlineEpoch = reader.u32();
  const count = reader.u16(); reader.reserved(2);
  if (identity === 0 || stepCount === 0 || activeStep === 0 || activeStep > stepCount || count > 16 || state < 1 || state > 7) throw new Error("invalid procedure identity, enum, or bounds");
  const title = reader.text(); const instruction = reader.text(); const predicates: ProcedurePredicate[] = [];
  for (let index = 0; index < count; index += 1) {
    const predicateIdentity = reader.u32(); const satisfied = reader.bool(); reader.reserved(3);
    if (predicateIdentity === 0) throw new Error("invalid procedure predicate identity");
    predicates.push({ identity: predicateIdentity, satisfied });
  }
  reader.finish();
  return { identity, activeStep, stepCount, state, hintsAvailable, enteredEpoch, deadlineEpoch, title, instruction, predicates };
}

function decodeDisposition(bytes: Uint8Array): DispositionView {
  const reader = new PayloadReader(bytes, "PDS1");
  const value = { overall: reader.u8(), objective: reader.u8(), vehicle: reader.u8(), procedure: reader.u8(),
    operator: reader.u8(), avionics: reader.u8(), evidence: reader.u8(), reasonIdentity: 0 };
  reader.reserved(1); value.reasonIdentity = reader.u32(); reader.finish();
  if (value.overall < 1 || value.overall > 5 || value.objective < 1 || value.objective > 5 ||
      value.vehicle < 1 || value.vehicle > 6 || value.procedure < 1 || value.procedure > 6 ||
      value.operator < 1 || value.operator > 5 || value.avionics < 1 || value.avionics > 4 ||
      value.evidence < 1 || value.evidence > 5) throw new Error("invalid disposition enum");
  return value;
}

function decodeProposal(bytes: Uint8Array): ActionProposalView {
  const reader = new PayloadReader(bytes, "PAP1");
  const value = { proposalIdentity: reader.u32(), loadIdentity: reader.u32(), loadType: reader.u8(),
    permittedOperations: reader.u8(), stageEpoch: 0, earliestCommitEpoch: 0, activationEpoch: 0,
    expiresEpoch: 0, payloadChecksum: 0, completedEventMask: 0, label: "" };
  reader.reserved(2);
  Object.assign(value, { stageEpoch: reader.u32(), earliestCommitEpoch: reader.u32(), activationEpoch: reader.u32(),
    expiresEpoch: reader.u32(), payloadChecksum: reader.u32(), completedEventMask: reader.u32(), label: reader.text() });
  reader.finish();
  if (value.proposalIdentity === 0 || value.loadType < 1 || value.loadType > 5 || value.permittedOperations === 0) throw new Error("invalid action proposal");
  return value;
}

function decodeReceipt(bytes: Uint8Array): ActionReceiptView {
  const reader = new PayloadReader(bytes, "PAR1");
  const value: ActionReceiptView = { publicationSequence: reader.u64(), proposalIdentity: reader.u32(),
    loadIdentity: reader.u32(), controlIdentity: reader.u32(), receiptEpoch: reader.u32(),
    effectiveEpoch: reader.u32(), state: reader.u8(), reason: reader.u8(), accepted: reader.bool(),
    operation: reader.u8() as ActionOperation, receiptChecksum: reader.u32() };
  reader.finish();
  if (value.publicationSequence === 0n || value.proposalIdentity === 0 || value.operation < 1 || value.operation > 4 || value.state > 6 || value.reason > 13) throw new Error("invalid action receipt");
  return value;
}

function decodeTimeline(bytes: Uint8Array): TimelineEventView {
  const reader = new PayloadReader(bytes, "PTE1");
  const sequence = reader.u64(); const releaseEpoch = reader.u32(); const sourceIdentity = reader.u32();
  const severity = reader.u8(); reader.reserved(3); const eventIdentity = reader.u32();
  const detailIdentity = reader.u32(); const label = reader.text(); reader.finish();
  if (sequence === 0n || eventIdentity === 0 || severity < 1 || severity > 4) throw new Error("invalid timeline event");
  return { sequence, releaseEpoch, sourceIdentity, severity, eventIdentity, detailIdentity, label };
}

function sample(reader: PayloadReader): ReleaseSampleView {
  return { sequence: reader.u64(), validityMask: reader.u64(), releaseEpoch: reader.u32(),
    missionTimeQ16: reader.u32(), frameIdentity: reader.u32(), onboardPosition: reader.i32x3(),
    onboardVelocity: reader.i32x3(), groundPosition: reader.i32x3(), groundVelocity: reader.i32x3(),
    predictedImpact: reader.i32x3(), predictedApogeeQ12Km: reader.i32(), altitudeQ12Km: reader.i32(),
    speedQ24KmS: reader.i32(), downrangeQ12Km: reader.i32(), crossrangeQ12Km: reader.i32() };
}

function decodeSamples(bytes: Uint8Array): readonly ReleaseSampleView[] {
  const reader = new PayloadReader(bytes, "PRS1"); const count = reader.u16(); reader.reserved(2);
  if (count > 2048) throw new Error("sample batch exceeds bound");
  const values: ReleaseSampleView[] = [];
  let previous = 0n;
  for (let index = 0; index < count; index += 1) { const value = sample(reader);
    if (value.sequence === 0n || (previous !== 0n && value.sequence <= previous)) throw new Error("invalid sample sequence");
    previous = value.sequence; values.push(value); }
  reader.finish(); return values;
}

function decodeEvents(bytes: Uint8Array): readonly PresentationEventView[] {
  const reader = new PayloadReader(bytes, "PEV1"); const count = reader.u16(); reader.reserved(2);
  if (count > 2048) throw new Error("event batch exceeds bound");
  const values: PresentationEventView[] = []; let previous = 0n;
  for (let index = 0; index < count; index += 1) {
    const sequence = reader.u64(); const releaseEpoch = reader.u32(); const kind = reader.u16();
    reader.reserved(2); const detailIdentity = reader.u32();
    if (sequence === 0n || kind === 0 || (previous !== 0n && sequence <= previous)) throw new Error("invalid event sequence");
    previous = sequence; values.push({ sequence, releaseEpoch, kind, detailIdentity });
  }
  reader.finish(); return values;
}

function decodePath(bytes: Uint8Array): PredictionPathView {
  const reader = new PayloadReader(bytes, "PPP1");
  const pathIdentity = reader.u32(); const productIdentity = reader.u32(); const modelIdentity = reader.u32();
  const sourceEstimateIdentity = reader.u32(); const sourceEstimateChecksum = reader.u32();
  const sourceEpoch = reader.u32(); const generationEpoch = reader.u32(); const frameIdentity = reader.u32();
  const terminalReason = reader.u8(); reader.reserved(3); const cadenceReleases = reader.u32();
  const pathChecksum = reader.u32(); const count = reader.u16(); reader.reserved(2);
  if (count > 4096) throw new Error("prediction path exceeds bound");
  const points: PredictionPathPoint[] = [];
  for (let index = 0; index < count; index += 1) points.push({ releaseEpoch: reader.u32(), frameIdentity: reader.u32(),
    position: reader.i32x3(), altitudeQ12Km: reader.i32(), downrangeQ12Km: reader.i32(), crossrangeQ12Km: reader.i32() });
  reader.finish();
  if (pathIdentity === 0 || modelIdentity === 0 || productIdentity < 1 || productIdentity > 4 || terminalReason < 1 || terminalReason > 5) throw new Error("invalid prediction path identity");
  for (let index = 1; index < points.length; index += 1) if (points[index]!.releaseEpoch <= points[index - 1]!.releaseEpoch) throw new Error("invalid prediction path sequence");
  return { pathIdentity, productIdentity, modelIdentity, sourceEstimateIdentity, sourceEstimateChecksum,
    sourceEpoch, generationEpoch, frameIdentity, terminalReason, cadenceReleases, pathChecksum, points };
}

function decodeTransport(bytes: Uint8Array): TransportStatusView {
  const reader = new PayloadReader(bytes, "PTS1");
  const staleness = reader.u8(); const workerState = reader.u8(); const finalizationState = reader.u8(); reader.reserved(1);
  const value = { staleness, workerState, finalizationState, commandCapacity: reader.u32(), commandsPending: reader.u32(),
    eventCapacity: reader.u32(), eventsPending: reader.u32(), timelineCapacity: reader.u32(), timelinePending: reader.u32(),
    sampleCapacity: reader.u32(), samplesPending: reader.u32(), eventOverflow: reader.bool(), timelineOverflow: reader.bool(),
    sampleOverflow: reader.bool(), lastCommandResult: 0 };
  reader.reserved(1); value.lastCommandResult = reader.i32(); reader.finish();
  if (staleness < 1 || staleness > 5 || workerState > 3 || finalizationState > 2 ||
      value.commandsPending > value.commandCapacity || value.eventsPending > value.eventCapacity ||
      value.timelinePending > value.timelineCapacity || value.samplesPending > value.sampleCapacity) throw new Error("invalid transport status");
  return value;
}

function decodeEvidence(bytes: Uint8Array): EvidenceMetadata {
  if (bytes.byteLength !== 48) throw new Error("evidence metadata must be 48 bytes");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const magic = new TextDecoder().decode(bytes.subarray(0, 4));
  if (magic !== "PEM1" || view.getUint16(4, true) !== 1 || view.getUint16(6, true) !== 48) throw new Error("invalid evidence metadata header");
  if ((bytes[32] ?? 0) > 1) throw new Error("invalid evidence completion flag");
  for (const value of bytes.subarray(34)) if (value !== 0) throw new Error("nonzero evidence reserved byte");
  const result = { evidenceIdentity: view.getUint32(8, true), evidenceCrc32: view.getUint32(12, true),
    totalLength: view.getBigUint64(16, true), chunkLength: view.getUint32(24, true), chunkCount: view.getUint32(28, true),
    complete: bytes[32] === 1, contentKind: bytes[33] ?? 0 };
  if (result.evidenceIdentity === 0 || result.contentKind === 0 || result.totalLength === 0n || result.totalLength > 16n * 1024n * 1024n ||
      result.chunkLength === 0 || result.chunkLength > (64 * 1024 - 36) || result.chunkCount === 0 ||
      BigInt(result.chunkCount) !== (result.totalLength + BigInt(result.chunkLength) - 1n) / BigInt(result.chunkLength)) throw new Error("invalid evidence metadata");
  return result;
}

function decodeEvidenceChunk(bytes: Uint8Array): EvidenceChunk {
  const reader = new PayloadReader(bytes, "PEC1");
  const evidenceIdentity = reader.u32();
  const chunkIndex = reader.u32();
  const chunkCount = reader.u32();
  const logicalOffset = reader.u64();
  const dataLength = reader.u32();
  if (dataLength === 0 || dataLength > 64 * 1024 - 36) throw new Error("invalid evidence chunk length");
  const chunkBytes = reader.bytes(dataLength).slice();
  reader.finish();
  if (evidenceIdentity === 0 || chunkCount === 0 || chunkIndex >= chunkCount) throw new Error("invalid evidence chunk identity");
  return { evidenceIdentity, chunkIndex, chunkCount, logicalOffset, bytes: chunkBytes };
}

function decodeError(bytes: Uint8Array): PresentationErrorView {
  const reader = new PayloadReader(bytes, "PER1"); const code = reader.u16(); const fatal = reader.bool();
  reader.reserved(1); const detailIdentity = reader.u32(); const message = reader.text(); reader.finish();
  return { code, fatal, detailIdentity, message };
}

export function decodePresentationPayload(frame: Kps1Frame, expectedRole?: NumericRole): DecodedPresentationPayload {
  switch (frame.kind) {
    case Kps1MessageKind.Snapshot: return { kind: "snapshot", value: decodeSnapshot(frame.payload, expectedRole) };
    case Kps1MessageKind.Procedure: return { kind: "procedure", value: decodeProcedure(frame.payload) };
    case Kps1MessageKind.Disposition: return { kind: "disposition", value: decodeDisposition(frame.payload) };
    case Kps1MessageKind.ActionProposal: return { kind: "action-proposal", value: decodeProposal(frame.payload) };
    case Kps1MessageKind.ActionReceipt: return { kind: "action-receipt", value: decodeReceipt(frame.payload) };
    case Kps1MessageKind.TimelineEvent: return { kind: "timeline", value: decodeTimeline(frame.payload) };
    case Kps1MessageKind.EventBatch: return { kind: "events", value: decodeEvents(frame.payload) };
    case Kps1MessageKind.ReleaseSampleBatch: return { kind: "samples", value: decodeSamples(frame.payload) };
    case Kps1MessageKind.PredictionPath: return { kind: "prediction-path", value: decodePath(frame.payload) };
    case Kps1MessageKind.TransportStatus: return { kind: "transport", value: decodeTransport(frame.payload) };
    case Kps1MessageKind.GlobalDisplayDefinition: return { kind: "global-definition", value: decodeGlobalDisplayDefinitionPayload(frame.payload, expectedRole ?? 1) };
    case Kps1MessageKind.GlobalDisplaySampleBatch: return { kind: "global-samples", value: decodeGlobalDisplaySamplesPayload(frame.payload, expectedRole ?? 1) };
    case Kps1MessageKind.GlobalDisplayPathChunk: return { kind: "global-path", value: decodeGlobalDisplayPathPayload(frame.payload, expectedRole ?? 1) };
    case Kps1MessageKind.GlobalDisplayTransition: return { kind: "global-transition", value: decodeGlobalDisplayTransitionPayload(frame.payload) };
    case Kps1MessageKind.GlobalReplayIndex: return { kind: "global-replay-index", value: decodeGlobalReplayIndexPayload(frame.payload) };
    case Kps1MessageKind.EvidenceMetadata: return { kind: "evidence", value: decodeEvidence(frame.payload) };
    case Kps1MessageKind.EvidenceChunk: return { kind: "evidence-chunk", value: decodeEvidenceChunk(frame.payload) };
    case Kps1MessageKind.Error: return { kind: "error", value: decodeError(frame.payload) };
    default: return { kind: "unhandled", messageKind: frame.kind, bytes: frame.payload };
  }
}

export interface ActionIntentFields {
  readonly proposalIdentity: number;
  readonly expectedLoadIdentity: number;
  readonly operation: ActionOperation;
  readonly requestedActivationEpoch: number;
  readonly clientActionSequence: bigint;
}

export function encodeActionIntentPayload(value: ActionIntentFields): Uint8Array {
  if (value.proposalIdentity === 0 || value.clientActionSequence === 0n || value.operation < 1 || value.operation > 4) {
    throw new Error("invalid presentation action intent");
  }
  if (value.operation !== 1 && value.expectedLoadIdentity === 0) throw new Error("stateful action needs a load identity");
  const bytes = new Uint8Array(32); const view = new DataView(bytes.buffer);
  bytes.set(new TextEncoder().encode("PAI1"), 0); view.setUint16(4, 1, true); view.setUint16(6, 32, true);
  view.setUint32(8, value.proposalIdentity, true); view.setUint32(12, value.expectedLoadIdentity, true);
  bytes[16] = value.operation; view.setUint32(20, value.requestedActivationEpoch, true);
  view.setBigUint64(24, value.clientActionSequence, true); return bytes;
}


export interface PresentationCursorsView {
  readonly snapshots: bigint;
  readonly events: bigint;
  readonly timeline: bigint;
  readonly actionReceipts: bigint;
  readonly releaseSamples: bigint;
}

export interface HandshakePayload {
  readonly role: NumericRole;
  readonly clientInstance: bigint;
  readonly capabilityMask: bigint;
  readonly cursors: PresentationCursorsView;
}

const PRESENTATION_CURSOR_BEGIN = 1n;

export function initialPresentationCursors(): PresentationCursorsView {
  return { snapshots: PRESENTATION_CURSOR_BEGIN, events: PRESENTATION_CURSOR_BEGIN,
    timeline: PRESENTATION_CURSOR_BEGIN, actionReceipts: PRESENTATION_CURSOR_BEGIN,
    releaseSamples: PRESENTATION_CURSOR_BEGIN };
}

function assertCursor(value: bigint): void {
  if (value <= 0n || value > ((1n << 64n) - 1n)) throw new Error("presentation cursor must be a nonzero u64");
}

function putPayloadHeader(bytes: Uint8Array, magic: string): DataView {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  bytes.set(new TextEncoder().encode(magic), 0);
  view.setUint16(4, 1, true);
  view.setUint16(6, 12, true);
  view.setUint32(8, bytes.byteLength, true);
  return view;
}

export function encodeHandshakePayload(value: HandshakePayload): Uint8Array {
  if (value.role < 1 || value.role > 6 || value.clientInstance === 0n) throw new Error("invalid presentation handshake");
  for (const cursor of Object.values(value.cursors)) assertCursor(cursor);
  const bytes = new Uint8Array(72); const view = putPayloadHeader(bytes, "PHS1");
  bytes[12] = value.role; view.setBigUint64(16, value.clientInstance, true);
  view.setBigUint64(24, value.capabilityMask, true);
  view.setBigUint64(32, value.cursors.snapshots, true); view.setBigUint64(40, value.cursors.events, true);
  view.setBigUint64(48, value.cursors.timeline, true); view.setBigUint64(56, value.cursors.actionReceipts, true);
  view.setBigUint64(64, value.cursors.releaseSamples, true); return bytes;
}

export function decodeHandshakePayload(bytes: Uint8Array): HandshakePayload {
  const reader = new PayloadReader(bytes, "PHS1"); const role = reader.u8() as NumericRole; reader.reserved(3);
  const clientInstance = reader.u64(); const capabilityMask = reader.u64();
  const cursors = { snapshots: reader.u64(), events: reader.u64(), timeline: reader.u64(),
    actionReceipts: reader.u64(), releaseSamples: reader.u64() }; reader.finish();
  if (role < 1 || role > 6 || clientInstance === 0n) throw new Error("invalid presentation handshake");
  for (const cursor of Object.values(cursors)) assertCursor(cursor);
  return { role, clientInstance, capabilityMask, cursors };
}

export function encodeCursorsPayload(value: PresentationCursorsView): Uint8Array {
  for (const cursor of Object.values(value)) assertCursor(cursor);
  const bytes = new Uint8Array(48); const view = new DataView(bytes.buffer);
  bytes.set(new TextEncoder().encode("KCR1"), 0); view.setUint16(4, 1, true); view.setUint16(6, 48, true);
  view.setBigUint64(8, value.snapshots, true); view.setBigUint64(16, value.events, true);
  view.setBigUint64(24, value.timeline, true); view.setBigUint64(32, value.actionReceipts, true);
  view.setBigUint64(40, value.releaseSamples, true); return bytes;
}


export function encodeLifecycleControlPayload(lifecycle: "running" | "paused", boundedReleases = 0): Uint8Array {
  if (!Number.isSafeInteger(boundedReleases) || boundedReleases < 0 || boundedReleases > 0xffff_ffff) throw new Error("invalid bounded release count");
  const bytes = new Uint8Array(20); const view = putPayloadHeader(bytes, "PLC1");
  bytes[12] = lifecycle === "running" ? 3 : 4; view.setUint32(16, boundedReleases, true); return bytes;
}

export function encodePaceControlPayload(pace: "fast" | "realtime", boundedReleases = 0): Uint8Array {
  if (!Number.isSafeInteger(boundedReleases) || boundedReleases < 0 || boundedReleases > 0xffff_ffff) throw new Error("invalid bounded release count");
  const bytes = new Uint8Array(20); const view = putPayloadHeader(bytes, "PPC1");
  bytes[12] = pace === "fast" ? 1 : 2; view.setUint32(16, boundedReleases, true); return bytes;
}
