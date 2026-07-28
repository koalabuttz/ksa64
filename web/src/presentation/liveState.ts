import { decodePresentationPayload, type ActionProposalView, type ActionReceiptView,
  type DispositionView, type EvidenceMetadata, type NumericRole, type OperationalSnapshot,
  type PredictionPathView, type PresentationErrorView, type PresentationEventView,
  type ProcedureView, type ReleaseSampleView, type TimelineEventView, type TransportStatusView,
} from "../protocol/presentation";
import type { GlobalDisplayCursorStateV1, GlobalDisplayDefinitionV1, GlobalDisplayPathChunkV1,
  GlobalDisplaySampleV1, GlobalDisplayTransitionV1, GlobalReplayIndexV1 } from "../protocol/globalDisplay";
import type { PresentationTransportEvent, PresentationTransportState } from "../transport";
import { appendEvidenceChunk, beginEvidenceAssembly, type EvidenceAssembly } from "./evidenceAssembly";

const MAX_PENDING_GLOBAL_PATH_STREAMS = 64;
const MAX_PENDING_GLOBAL_PATH_CHUNKS = 64;
const MAX_PENDING_GLOBAL_PATH_POINTS = 65_536;
const MAX_ACTIVE_GLOBAL_PATH_STREAMS = 64;
const MAX_ACTIVE_GLOBAL_PATH_POINTS = 65_536;

interface GlobalPathAssembly {
  readonly generationKey: string;
  readonly chunks: ReadonlyMap<number, GlobalDisplayPathChunkV1>;
}

function globalPathStreamKey(value: GlobalDisplayPathChunkV1): string {
  return [value.pathIdentity, value.source, value.displayFrame, value.lod].join(":");
}

function globalPathGenerationKey(value: GlobalDisplayPathChunkV1): string {
  return [
    globalPathStreamKey(value),
    value.flags,
    value.modelIdentity,
    value.estimateIdentity,
    value.sourceChecksum,
    value.continuityIdentity,
    value.chunkCount,
  ].join(":");
}

function globalPathPointCount(values: Iterable<GlobalDisplayPathChunkV1>): number {
  let count = 0;
  for (const value of values) count += value.points.length;
  return count;
}

function pendingGlobalPathPointCount(
  pending: ReadonlyMap<string, GlobalPathAssembly>,
  excludingStream?: string,
): number {
  let count = 0;
  for (const [streamKey, assembly] of pending) {
    if (streamKey !== excludingStream) count += globalPathPointCount(assembly.chunks.values());
  }
  return count;
}

function completeGlobalPath(
  state: LivePresentationState,
  streamKey: string,
  assembly: GlobalPathAssembly,
): LivePresentationState {
  const first = assembly.chunks.get(0);
  if (first === undefined || assembly.chunks.size !== first.chunkCount) {
    throw new Error("global display path assembly completed without every chunk");
  }
  let priorRelease = -1;
  let priorTime = -1;
  for (let index = 0; index < first.chunkCount; index += 1) {
    const chunk = assembly.chunks.get(index);
    if (chunk === undefined) {
      throw new Error("global display path assembly has a missing chunk");
    }
    for (const point of chunk.points) {
      if (point.releaseEpoch <= priorRelease || point.missionTimeQ16 <= priorTime) {
        throw new Error("global display path point order changed between chunks");
      }
      priorRelease = point.releaseEpoch;
      priorTime = point.missionTimeQ16;
    }
  }

  // Never mutate the public map until the entire generation has passed its
  // boundary checks. React therefore sees either the last complete path or
  // the next complete path, never a transient subset.
  const globalPaths = new Map(state.globalPaths);
  for (const [key, value] of globalPaths) {
    if (globalPathStreamKey(value) === streamKey) globalPaths.delete(key);
  }
  for (let index = 0; index < first.chunkCount; index += 1) {
    const value = assembly.chunks.get(index);
    if (value === undefined) throw new Error("global display path assembly has a missing chunk");
    globalPaths.set(`${streamKey}:${index}`, value);
  }

  const activeStreams = new Set([...globalPaths.values()].map(globalPathStreamKey));
  if (activeStreams.size > MAX_ACTIVE_GLOBAL_PATH_STREAMS) {
    throw new Error("too many active global display paths");
  }
  if (globalPathPointCount(globalPaths.values()) > MAX_ACTIVE_GLOBAL_PATH_POINTS) {
    throw new Error("active global display paths exceed the retained point limit");
  }

  const pendingGlobalPaths = new Map(state.pendingGlobalPaths);
  pendingGlobalPaths.delete(streamKey);
  return { ...state, globalPaths, pendingGlobalPaths, decodeError: undefined };
}

function acceptGlobalPathChunk(
  state: LivePresentationState,
  value: GlobalDisplayPathChunkV1,
): LivePresentationState {
  const streamKey = globalPathStreamKey(value);
  const generationKey = globalPathGenerationKey(value);
  if (value.chunkCount > MAX_PENDING_GLOBAL_PATH_CHUNKS) {
    throw new Error("global display path exceeds the bounded pending chunk limit");
  }
  const pendingGlobalPaths = new Map(state.pendingGlobalPaths);
  let assembly = pendingGlobalPaths.get(streamKey);
  if (assembly === undefined && pendingGlobalPaths.size >= MAX_PENDING_GLOBAL_PATH_STREAMS) {
    throw new Error("too many pending global display paths");
  }

  if (assembly !== undefined && assembly.generationKey !== generationKey) {
    // A replacement generation starts at its first chunk. Keep the last
    // complete generation visible while rebuilding the replacement. A changed
    // later chunk cannot belong to either valid generation and fails closed.
    if (value.chunkIndex !== 0) {
      throw new Error("global display path metadata changed within a pending generation");
    }
    assembly = undefined;
  }

  const chunks = new Map(assembly?.chunks);
  if (chunks.has(value.chunkIndex)) {
    throw new Error("duplicate global display path chunk in one generation");
  }
  const retainedOtherPoints = pendingGlobalPathPointCount(
    pendingGlobalPaths,
    assembly === undefined ? streamKey : undefined,
  );
  if (retainedOtherPoints + value.points.length > MAX_PENDING_GLOBAL_PATH_POINTS) {
    throw new Error("pending global display paths exceed the retained point limit");
  }
  chunks.set(value.chunkIndex, value);
  const nextAssembly: GlobalPathAssembly = { generationKey, chunks };

  if (chunks.size === value.chunkCount) {
    return completeGlobalPath(
      { ...state, pendingGlobalPaths },
      streamKey,
      nextAssembly,
    );
  }

  pendingGlobalPaths.set(streamKey, nextAssembly);
  return { ...state, pendingGlobalPaths, decodeError: undefined };
}

export interface LivePresentationState {
  readonly connection: PresentationTransportState;
  readonly connectionDetail?: string;
  readonly incompleteReason?: string;
  readonly decodeError?: string;
  readonly snapshot?: OperationalSnapshot;
  readonly procedure?: ProcedureView;
  readonly disposition?: DispositionView;
  readonly proposal?: ActionProposalView;
  readonly receipts: readonly ActionReceiptView[];
  readonly timeline: readonly TimelineEventView[];
  readonly events: readonly PresentationEventView[];
  readonly samples: readonly ReleaseSampleView[];
  readonly paths: ReadonlyMap<number, PredictionPathView>;
  readonly transport?: TransportStatusView;
  readonly globalDefinition?: GlobalDisplayDefinitionV1;
  readonly globalSamples: readonly GlobalDisplaySampleV1[];
  /** Complete path generations only; partial multi-chunk paths stay internal. */
  readonly globalPaths: ReadonlyMap<string, GlobalDisplayPathChunkV1>;
  /** Pending chunks are retained until a whole path generation is complete. */
  readonly pendingGlobalPaths: ReadonlyMap<string, GlobalPathAssembly>;
  readonly globalTransitions: readonly GlobalDisplayTransitionV1[];
  readonly globalReplayIndex?: GlobalReplayIndexV1;
  readonly globalCursor?: GlobalDisplayCursorStateV1;
  readonly evidence?: EvidenceMetadata;
  readonly evidenceAssembly?: EvidenceAssembly;
  readonly sealedEvidence?: Uint8Array;
  readonly evidenceAssemblyError?: string;
  readonly presentationError?: PresentationErrorView;
  readonly pendingAction?: string;
  readonly demo: boolean;
}

export function initialLivePresentationState(demo = false): LivePresentationState {
  return { connection: "idle", receipts: [], timeline: [], events: [], samples: [], paths: new Map(),
    globalSamples: [], globalPaths: new Map(), pendingGlobalPaths: new Map(), globalTransitions: [], demo };
}

function appendUnique<T>(current: readonly T[], incoming: readonly T[], key: (value: T) => bigint, limit: number): readonly T[] {
  if (incoming.length === 0) return current;
  const seen = new Set(current.map((value) => key(value).toString()));
  const merged = [...current];
  for (const value of incoming) if (!seen.has(key(value).toString())) { merged.push(value); seen.add(key(value).toString()); }
  merged.sort((left, right) => key(left) < key(right) ? -1 : key(left) > key(right) ? 1 : 0);
  return merged.slice(-limit);
}

export function reduceTransportEvent(
  state: LivePresentationState,
  event: PresentationTransportEvent,
  expectedRole: NumericRole,
): LivePresentationState {
  if (event.type === "state") return { ...state, connection: event.state, connectionDetail: event.detail,
    pendingAction: event.state === "failed" ? undefined : state.pendingAction };
  if (event.type === "incomplete") return { ...state, incompleteReason: event.reason, connection: "failed", pendingAction: undefined };
  try {
    const decoded = decodePresentationPayload(event.frame, expectedRole);
    switch (decoded.kind) {
      case "snapshot":
        if (state.snapshot !== undefined && decoded.value.publicationSequence < state.snapshot.publicationSequence) return state;
        return {
          ...state,
          snapshot: decoded.value,
          proposal: (decoded.value.validityMask & (1n << 5n)) === 0n ? undefined : state.proposal,
          decodeError: undefined,
        };
      case "procedure": return { ...state, procedure: decoded.value, decodeError: undefined };
      case "disposition": return { ...state, disposition: decoded.value, decodeError: undefined };
      case "action-proposal": return { ...state, proposal: decoded.value, decodeError: undefined };
      case "action-receipt": return { ...state,
        receipts: appendUnique(state.receipts, [decoded.value], (value) => value.publicationSequence, 128),
        pendingAction: undefined, decodeError: undefined };
      case "timeline": return { ...state,
        timeline: appendUnique(state.timeline, [decoded.value], (value) => value.sequence, 2048), decodeError: undefined };
      case "events": return { ...state,
        events: appendUnique(state.events, decoded.value, (value) => value.sequence, 4096), decodeError: undefined };
      case "samples": return { ...state,
        samples: appendUnique(state.samples, decoded.value, (value) => value.sequence, 4096), decodeError: undefined };
      case "prediction-path": {
        const paths = new Map(state.paths); paths.set(decoded.value.productIdentity, decoded.value);
        return { ...state, paths, decodeError: undefined };
      }
      case "transport": return { ...state, transport: decoded.value, decodeError: undefined };
      case "global-definition": return { ...state, globalDefinition: decoded.value, decodeError: undefined };
      case "global-samples": return { ...state,
        globalSamples: appendUnique(state.globalSamples, decoded.value, (value) => value.sequence, 32_768),
        decodeError: undefined };
      case "global-path": return acceptGlobalPathChunk(state, decoded.value);
      case "global-transition": return { ...state,
        globalTransitions: appendUnique(state.globalTransitions, [decoded.value],
          (value) => (BigInt(value.releaseEpoch) << 32n) | BigInt(value.transitionIdentity), 64),
        decodeError: undefined };
      case "global-replay-index": return { ...state, globalReplayIndex: decoded.value, decodeError: undefined };
      case "global-cursor": return { ...state, globalCursor: decoded.value, decodeError: undefined };
      case "evidence": {
        const evidenceAssembly = beginEvidenceAssembly(state.evidenceAssembly, decoded.value);
        return { ...state, evidence: decoded.value, evidenceAssembly,
          sealedEvidence: evidenceAssembly.sealed, evidenceAssemblyError: undefined, decodeError: undefined };
      }
      case "evidence-chunk": {
        const evidenceAssembly = appendEvidenceChunk(state.evidenceAssembly, decoded.value);
        return { ...state, evidenceAssembly, sealedEvidence: evidenceAssembly.sealed,
          evidenceAssemblyError: undefined, decodeError: undefined };
      }
      case "error": return { ...state, presentationError: decoded.value,
        connection: decoded.value.fatal ? "failed" : decoded.value.code === 1 ? "resync-required" : state.connection,
        pendingAction: undefined, decodeError: undefined };
      case "unhandled": return state;
    }
  } catch (error) {
    const detail = error instanceof Error ? error.message : "presentation payload rejected";
    return { ...state, decodeError: detail,
      connection: "failed", pendingAction: undefined };
  }
}

export function markPendingAction(state: LivePresentationState, kind: string): LivePresentationState {
  return { ...state, pendingAction: kind, connectionDetail: undefined };
}

export function markActionFailure(state: LivePresentationState, detail: string): LivePresentationState {
  return { ...state, pendingAction: undefined, connectionDetail: detail };
}
