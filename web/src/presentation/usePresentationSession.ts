import { useCallback, useEffect, useReducer, useState } from "react";
import type { NumericRole } from "../protocol/presentation";
import type { PresentationActionKind, PresentationPaceMode, PresentationRole, PresentationTransport } from "../transport";
import { initialLivePresentationState, reduceTransportEvent } from "./liveState";

function numericRole(role: PresentationRole): NumericRole {
  switch (role) {
    case "observer": return 1; case "guided-operator": return 2; case "flight-controller": return 3;
    case "flight-software-engineer": return 4; case "sim-director": return 5;
  }
}

export interface UsePresentationSessionOptions {
  readonly transport: PresentationTransport;
  readonly role: PresentationRole;
  readonly autoConnect?: boolean;
}

export function usePresentationSession({ transport, role, autoConnect = true }: UsePresentationSessionOptions) {
  const [state, dispatch] = useReducer((current: ReturnType<typeof initialLivePresentationState>, event: Parameters<typeof reduceTransportEvent>[1]) =>
    reduceTransportEvent(current, event, numericRole(role)), undefined, () => initialLivePresentationState());
  const [pendingAction, setPendingAction] = useState<PresentationActionKind>();

  useEffect(() => {
    const unsubscribe = transport.subscribe((event) => {
      dispatch(event);
      if (event.type === "frame" || event.type === "incomplete" || (event.type === "state" && event.state === "failed")) setPendingAction(undefined);
    });
    if (autoConnect) void transport.connect({ role }).catch(() => { /* transport publishes the typed failure */ });
    return () => { unsubscribe(); void transport.disconnect(); };
  }, [autoConnect, role, transport]);

  const submit = useCallback(async (kind: PresentationActionKind) => {
    const proposal = state.proposal;
    if (proposal === undefined) throw new Error("no current action proposal");
    setPendingAction(kind);
    try {
      await transport.submit({ kind, proposalId: proposal.label,
        proposalIdentity: proposal.proposalIdentity, expectedLoadIdentity: proposal.loadIdentity,
        requestedActivationEpoch: proposal.activationEpoch });
    } catch (error) {
      setPendingAction(undefined);
      throw error;
    }
  }, [state.proposal, transport]);

  const setPace = useCallback(async (pace: PresentationPaceMode) => {
    if (transport.setPace === undefined) throw new Error("this presentation source does not support live pacing");
    await transport.setPace(pace);
  }, [transport]);

  const reconnect = useCallback(async () => {
    await transport.disconnect();
    await transport.connect({ role });
  }, [role, transport]);

  return { state, pendingAction, submit, setPace, reconnect };
}
