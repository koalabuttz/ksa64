import { decodePresentationPayload, type ActionProposalView, type ActionReceiptView,
  type DispositionView, type EvidenceMetadata, type NumericRole, type OperationalSnapshot,
  type PredictionPathView, type PresentationErrorView, type PresentationEventView,
  type ProcedureView, type ReleaseSampleView, type TimelineEventView, type TransportStatusView,
} from "../protocol/presentation";
import type { GlobalDisplayDefinitionV1, GlobalDisplayPathChunkV1, GlobalDisplaySampleV1,
  GlobalDisplayTransitionV1, GlobalReplayIndexV1 } from "../protocol/globalDisplay";
import type { PresentationTransportEvent, PresentationTransportState } from "../transport";
import { appendEvidenceChunk, beginEvidenceAssembly, type EvidenceAssembly } from "./evidenceAssembly";

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
  readonly globalPaths: ReadonlyMap<string, GlobalDisplayPathChunkV1>;
  readonly globalTransitions: readonly GlobalDisplayTransitionV1[];
  readonly globalReplayIndex?: GlobalReplayIndexV1;
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
    globalSamples: [], globalPaths: new Map(), globalTransitions: [], demo };
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
      case "global-path": {
        const globalPaths = new Map(state.globalPaths);
        globalPaths.set(`${decoded.value.pathIdentity}:${decoded.value.chunkIndex}`, decoded.value);
        return { ...state, globalPaths, decodeError: undefined };
      }
      case "global-transition": return { ...state,
        globalTransitions: appendUnique(state.globalTransitions, [decoded.value],
          (value) => (BigInt(value.releaseEpoch) << 32n) | BigInt(value.transitionIdentity), 64),
        decodeError: undefined };
      case "global-replay-index": return { ...state, globalReplayIndex: decoded.value, decodeError: undefined };
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
