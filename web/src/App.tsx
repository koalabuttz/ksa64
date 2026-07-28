import { useMemo, useState } from "react";
import { GlobalMissionViewer } from "./components/GlobalMissionViewer";
import { TrajectoryPlot } from "./components/TrajectoryPlot";
import { groundTrajectory as demoGround, onboardTrajectory as demoOnboard,
  procedures as demoProcedures, timeline as demoTimeline, type ProcedureStep, type TimelineEvent } from "./model/demo";
import { buildExactGlobalDisplay, buildLegacySchematicDisplay, type GlobalDisplayLayoutV1 } from "./model/globalDisplay";
import { plannedTrajectory } from "./model/missionReference";
import type { TrajectoryPoint } from "./model/trajectory";
import { usePresentationSession } from "./presentation/usePresentationSession";
import type { GlobalReplayIndexV1 } from "./protocol/globalDisplay";
import type { ActionOperation, ActionReceiptView, DispositionView, PredictionPathView, ProcedureView,
  ReleaseSampleView, TimelineEventView } from "./protocol/presentation";
import { createDefaultPresentationTransport, type PresentationActionKind,
  type PresentationPaceMode, type PresentationRole, type PresentationTransport } from "./transport";

const GNSS_LOSS_RELEASES = 21_591;
const NOMINAL_GLOBAL_RELEASES = 22_015;
const Q12 = 4096;
const Q16 = 65_536;
const Q24 = 16_777_216;
const SNAPSHOT_VALID_NAVIGATION = 1n << 1n;
const SNAPSHOT_VALID_GROUND_ESTIMATE = 1n << 2n;
const SNAPSHOT_VALID_PREDICTION = 1n << 3n;
const OPERATION_CODE: Record<PresentationActionKind, ActionOperation> = { review: 1, stage: 2, commit: 3, cancel: 4 };
const PERMISSION_BIT: Record<PresentationActionKind, number> = { review: 1, stage: 2, commit: 4, cancel: 8 };

export type PresentationExperience = "gnss-loss" | "nominal-global";

export interface AppProps {
  readonly transport?: PresentationTransport;
  readonly role?: PresentationRole;
  readonly experience?: PresentationExperience;
  readonly autoConnect?: boolean;
}

function magnitude(values: readonly [number, number, number]): number {
  return Math.hypot(values[0], values[1], values[2]);
}

function residual(left: readonly [number, number, number], right: readonly [number, number, number], scale: number): number {
  return magnitude([left[0] - right[0], left[1] - right[1], left[2] - right[2]]) / scale;
}

function q12(value: number): number { return value / Q12; }
function q16(value: number): number { return value / Q16; }

function met(value?: number): string {
  const seconds = q16(value ?? 0); const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${(seconds - minutes * 60).toFixed(3).padStart(6, "0")}`;
}

function frameName(value?: number): string {
  switch (value) { case 1: return "LOCAL ENU"; case 2: return "ECEF"; case 3: return "GCRF"; default: return "AWAITING"; }
}

function lifecycleName(value?: number): string {
  switch (value) { case 1: return "Compiled"; case 2: return "Ready"; case 3: return "Running"; case 4: return "Paused";
    case 5: return "Completed"; case 6: return "Aborted"; case 7: return "Incomplete"; default: return "Connecting"; }
}

function roleName(role: PresentationRole): string {
  return role.split("-").map((part) => part[0]?.toUpperCase() + part.slice(1)).join(" ");
}

function pathPoints(path?: PredictionPathView): readonly TrajectoryPoint[] {
  return path?.points.map((point) => ({ downrange: q12(point.downrangeQ12Km), altitude: q12(point.altitudeQ12Km) })) ?? [];
}

function samplePoints(samples: readonly ReleaseSampleView[], source: "onboard" | "ground"): readonly TrajectoryPoint[] {
  const required = source === "onboard" ? SNAPSHOT_VALID_NAVIGATION : SNAPSHOT_VALID_GROUND_ESTIMATE;
  return samples.filter((sample) => (sample.validityMask & required) !== 0n).map((sample) => ({
    downrange: q12(sample.downrangeQ12Km),
    altitude: q12(source === "onboard" ? sample.onboardPosition[2] : sample.groundPosition[2]),
  }));
}

function procedureItems(value?: ProcedureView): readonly ProcedureStep[] {
  if (value === undefined) return [];
  const predicates: ProcedureStep[] = value.predicates.map((predicate, index) => ({
    id: `predicate-${predicate.identity}`, title: `Guard ${index + 1}`,
    detail: `Observable predicate ${predicate.identity.toString(16).toUpperCase()}`,
    status: predicate.satisfied ? "complete" : "pending",
  }));
  return [...predicates, { id: `step-${value.activeStep}`, title: value.instruction,
    detail: `Step ${value.activeStep} of ${value.stepCount} · deadline release ${value.deadlineEpoch.toLocaleString()}`,
    status: value.state === 3 ? "complete" : value.state === 1 ? "pending" : "active" }];
}

function timelineItems(values: readonly TimelineEventView[]): readonly TimelineEvent[] {
  return values.slice(-8).map((value) => ({ time: `R+${value.releaseEpoch.toLocaleString()}`, title: value.label,
    detail: `Event ${value.eventIdentity.toString(16).toUpperCase()} · source ${value.sourceIdentity.toString(16).toUpperCase()}`,
    severity: value.severity >= 3 ? "caution" : value.severity === 2 ? "advisory" : "nominal" }));
}

const overallLabels = ["", "Nominal success", "Degraded success", "Contingency success", "Mission failure", "Indeterminate"];
const objectiveLabels = ["", "Primary achieved", "Alternate achieved", "Contingency achieved", "Not achieved", "Indeterminate"];
const vehicleLabels = ["", "Nominal", "Degraded", "Recovered", "Safe state", "Lost", "Unknown"];
const procedureLabels = ["", "Completed", "Alternate branch", "Skipped", "Mistimed", "Overridden", "Failed"];
const operatorLabels = ["", "Timely reference", "Timely alternate", "Delayed valid", "No action", "Rejected action"];
const avionicsLabels = ["", "Nominal", "Degraded operational", "Safe recovery", "Failed"];
const evidenceLabels = ["", "Complete", "Observation incomplete", "Aborted", "Invalid", "Unavailable"];

function axisState(label: string): "success" | "warning" | "active" | "failure" {
  if (/failure|failed|lost|invalid|not achieved/iu.test(label)) return "failure";
  if (/degraded|incomplete|mistimed|skipped|delayed|rejected|no action/iu.test(label)) return "warning";
  if (/pending|progress|indeterminate|unavailable/iu.test(label)) return "active";
  return "success";
}

function outcomeAxes(value?: DispositionView) {
  if (value === undefined) return [
    { label: "Objective", value: "Pending", state: "active" as const },
    { label: "Vehicle", value: "In progress", state: "active" as const },
    { label: "Procedure", value: "In progress", state: "active" as const },
    { label: "Avionics", value: "Monitoring", state: "active" as const },
  ];
  const axes = [
    { label: "Objective", value: objectiveLabels[value.objective] ?? "Unknown" },
    { label: "Vehicle", value: vehicleLabels[value.vehicle] ?? "Unknown" },
    { label: "Procedure", value: procedureLabels[value.procedure] ?? "Unknown" },
    { label: "Operator", value: operatorLabels[value.operator] ?? "Unknown" },
    { label: "Avionics", value: avionicsLabels[value.avionics] ?? "Unknown" },
    { label: "Evidence", value: evidenceLabels[value.evidence] ?? "Unknown" },
  ];
  return axes.map((axis) => ({ ...axis, state: axisState(axis.value) }));
}

function replayDisposition(value?: GlobalReplayIndexV1): DispositionView | undefined {
  if (value === undefined || value.dispositionAxes.length !== 6) return undefined;
  return {
    overall: value.terminalDisposition,
    objective: value.dispositionAxes[0] ?? 0,
    vehicle: value.dispositionAxes[1] ?? 0,
    procedure: value.dispositionAxes[2] ?? 0,
    operator: value.dispositionAxes[3] ?? 0,
    avionics: value.dispositionAxes[4] ?? 0,
    evidence: value.dispositionAxes[5] ?? 0,
    reasonIdentity: 0,
  };
}

function downloadEvidence(bytes: Uint8Array): void {
  const blob = new Blob([bytes.slice().buffer], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "ksa64-session.ksb11";
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

function receiptFeedback(receipts: readonly ActionReceiptView[], pending?: PresentationActionKind): string {
  if (pending !== undefined) return `${pending} intent submitted; awaiting authoritative receipt.`;
  const receipt = receipts.at(-1); if (receipt === undefined) return "Actions remain proposals until Rust returns a validated receipt.";
  const operation = Object.entries(OPERATION_CODE).find(([, code]) => code === receipt.operation)?.[0] ?? "action";
  return receipt.accepted ? `${operation} accepted at release ${receipt.receiptEpoch.toLocaleString()}.`
    : `${operation} rejected (reason ${receipt.reason}).`;
}

export function App({ transport: suppliedTransport, role: suppliedRole, experience: suppliedExperience, autoConnect = true }: AppProps) {
  const experience = suppliedExperience ?? window.__KSA64_PRESENTATION__?.experience ?? "gnss-loss";
  const role = suppliedRole ?? (experience === "nominal-global" ? "sim-director" : "guided-operator");
  const nominalGlobal = experience === "nominal-global";
  const totalReleases = nominalGlobal ? NOMINAL_GLOBAL_RELEASES : GNSS_LOSS_RELEASES;
  const transport = useMemo(() => suppliedTransport ?? createDefaultPresentationTransport(), [suppliedTransport]);
  const { state, pendingAction, submit, setPace, reconnect } = usePresentationSession({ transport, role, autoConnect });
  const [pace, setPaceSelection] = useState<PresentationPaceMode>("realtime");
  const [contrast, setContrast] = useState(false);
  const [demo, setDemo] = useState(false);
  const [demoActionIndex, setDemoActionIndex] = useState(0);
  const [actionError, setActionError] = useState<string>();
  const [viewerLayout, setViewerLayout] = useState<GlobalDisplayLayoutV1>("hybrid");
  const [operationsDeskOpen, setOperationsDeskOpen] = useState(true);
  const phase12cEvidenceMode = new URLSearchParams(window.location.search).get("phase12c-evidence") === "1";

  const snapshot = demo ? undefined : state.snapshot;
  const liveProcedure = demo ? undefined : state.procedure;
  const acceptedDisposition = state.disposition ?? replayDisposition(state.globalReplayIndex);
  const onboardPath = demo ? demoOnboard : (pathPoints(state.paths.get(2)).length > 0 ? pathPoints(state.paths.get(2)) : samplePoints(state.samples, "onboard"));
  const groundPath = demo ? demoGround : (pathPoints(state.paths.get(3)).length > 0 ? pathPoints(state.paths.get(3)) : samplePoints(state.samples, "ground"));
  const procedure = demo ? demoProcedures : procedureItems(liveProcedure);
  const timeline = demo ? demoTimeline : timelineItems(state.timeline);
  const axes = demo ? [
    { label: "Objective", value: "Contingency achieved", state: "success" as const },
    { label: "Vehicle", value: "Recovered", state: "success" as const },
    { label: "Procedure", value: "In progress", state: "active" as const },
    { label: "Avionics", value: "Degraded operational", state: "warning" as const },
  ] : outcomeAxes(acceptedDisposition);
  const navigationValid = snapshot !== undefined && (snapshot.validityMask & SNAPSHOT_VALID_NAVIGATION) !== 0n;
  const groundEstimateValid = snapshot !== undefined && (snapshot.validityMask & SNAPSHOT_VALID_GROUND_ESTIMATE) !== 0n;
  const predictionValid = snapshot !== undefined && (snapshot.validityMask & SNAPSHOT_VALID_PREDICTION) !== 0n;
  const estimatesReady = navigationValid && groundEstimateValid;
  const positionResidual = estimatesReady ? residual(snapshot.onboard.position, snapshot.ground.position, Q12) : undefined;
  const velocityResidual = estimatesReady ? residual(snapshot.onboard.velocity, snapshot.ground.velocity, Q24) * 1000 : undefined;
  const exactDisplaySample = state.globalSamples.at(-1);
  const displayedRelease = snapshot?.releaseEpoch ?? exactDisplaySample?.releaseEpoch ?? 0;
  const displayedMissionTime = snapshot?.missionTimeQ16 ?? exactDisplaySample?.missionTimeQ16;
  const displayedFrame = snapshot?.frameIdentity ?? exactDisplaySample?.activeFrame;
  const progress = Math.min(100, (displayedRelease / totalReleases) * 100);
  const proposal = state.proposal;
  const acceptedOperations = new Set(state.receipts.filter((receipt) => receipt.accepted && receipt.proposalIdentity === proposal?.proposalIdentity)
    .map((receipt) => receipt.operation));
  const demoFlow = ["review", "stage", "commit"] as const;

  const isEnabled = (kind: PresentationActionKind): boolean => {
    if (demo) return kind === demoFlow[demoActionIndex] || (kind === "cancel" && demoActionIndex > 0 && demoActionIndex < 3);
    if (proposal === undefined || pendingAction !== undefined) return false;
    if ((proposal.permittedOperations & PERMISSION_BIT[kind]) === 0) return false;
    if (kind === "stage" && !acceptedOperations.has(OPERATION_CODE.review)) return false;
    if (kind === "commit" && !acceptedOperations.has(OPERATION_CODE.stage)) return false;
    return !acceptedOperations.has(OPERATION_CODE[kind]);
  };

  const performAction = async (kind: PresentationActionKind) => {
    setActionError(undefined);
    if (demo) { if (kind === "cancel") setDemoActionIndex(0); else if (isEnabled(kind)) setDemoActionIndex((value) => Math.min(3, value + 1)); return; }
    try { await submit(kind); } catch (error) { setActionError(error instanceof Error ? error.message : "action failed"); }
  };

  const connectionLabel = demo ? "Demonstration fixture" : state.connection === "connected" ?
    (transport.kind === "local-worker" ? "Local Rust/WASM authority" : transport.kind === "remote-websocket" ? "Native authority broker" : "Role-filtered replay") :
    `${transport.kind === "local-worker" ? "Local authority" : "Presentation"} · ${state.connection}`;
  const noTruth = snapshot?.truthPresent !== true;
  const evidenceComplete = state.evidence?.complete === true && state.sealedEvidence !== undefined &&
    BigInt(state.sealedEvidence.byteLength) === state.evidence.totalLength;
  const evidenceReceived = state.evidenceAssembly?.receivedLength ?? 0;
  const dispositionLabel = overallLabels[acceptedDisposition?.overall ?? 0];
  const overall = demo ? "Contingency success" : dispositionLabel || lifecycleName(snapshot?.lifecycle);
  const phase12cActionState = phase12cEvidenceMode ? JSON.stringify({
    release_epoch: snapshot?.releaseEpoch ?? 0,
    lifecycle: snapshot?.lifecycle ?? 0,
    action_count: snapshot?.actionCount ?? 0,
    proposal,
    receipts: state.receipts.map(({ publicationSequence, ...receipt }) => ({
      ...receipt, publication_sequence: publicationSequence.toString(),
    })),
  }) : undefined;
  const phase12cTimelineState = phase12cEvidenceMode ? JSON.stringify(state.timeline.map(({ sequence, ...event }) => ({
    ...event, sequence: sequence.toString(),
  }))) : undefined;
  const phase12cCompletedReplay = phase12cEvidenceMode && snapshot?.lifecycle === 5;
  const globalDisplay = useMemo(() => state.globalDefinition === undefined
    ? buildLegacySchematicDisplay({ snapshot, samples: state.samples, paths: state.paths, timeline: state.timeline,
      truthAllowed: role === "sim-director" && snapshot?.truthPresent === true, demonstration: demo })
    : buildExactGlobalDisplay({ definition: state.globalDefinition, samples: state.globalSamples,
      paths: state.globalPaths, transitions: state.globalTransitions, replay: state.globalReplayIndex }),
  [demo, role, snapshot, state.globalDefinition, state.globalPaths, state.globalReplayIndex, state.globalSamples,
    state.globalTransitions, state.paths, state.samples, state.timeline]);

  return (
    <div className="app-shell" data-contrast={contrast ? "high" : "standard"}>
      <a className="skip-link" href="#main-content">Skip to mission data</a>
      <header className="topbar">
        <div className="identity"><span className="mission-mark" aria-hidden="true">K</span><div>
          <p className="eyebrow">KSA64 · {roleName(role)}</p><h1>Mission Control</h1></div></div>
        <div className="topbar-status" aria-label="Connection status" data-state={demo ? "demo" : state.connection}>
          <span className="status-light" aria-hidden="true" /><span><strong>{connectionLabel}</strong>
            <small>{demo ? "Static test data · no authority" : noTruth ? "Role-filtered · no private truth" : "SIM Director truth enabled"}</small></span>
        </div>
        <label className="topbar-pace">Pace<select aria-label="Session pace" value={pace} disabled={demo || transport.setPace === undefined}
          onChange={(event) => { const next = event.target.value as PresentationPaceMode; setPaceSelection(next); void setPace(next).catch((error: unknown) => setActionError(error instanceof Error ? error.message : "pace rejected")); }}>
          <option value="realtime">Real time</option><option value="fast">Fast</option></select></label>
        <button className="quiet-button" type="button" aria-pressed={contrast} onClick={() => setContrast((value) => !value)}>
          {contrast ? "Standard contrast" : "High contrast"}</button>
      </header>

      {(["failed", "stale", "resync-required"].includes(state.connection) || state.incompleteReason !== undefined) && !demo && (
        <aside className="connection-banner" role="alert"><strong>{state.connection === "stale" ? "Presentation disconnected; authority continues." : "Live presentation unavailable."}</strong>
          <span>{state.incompleteReason ?? state.decodeError ?? state.connectionDetail ?? "The authority connection failed."}</span>
          <button type="button" onClick={() => void reconnect()}>Reconnect live presentation</button>
          {(state.connection === "failed" || state.incompleteReason !== undefined) && <button type="button" onClick={() => setDemo(true)}>Open labeled demonstration fixture</button>}</aside>
      )}
      {demo && <aside className="demo-banner" role="status">Demonstration mode uses static, non-authoritative test data.
        <button type="button" onClick={() => setDemo(false)}>Return to live connection</button></aside>}

      <main id="main-content" className="dashboard" data-viewer-layout={viewerLayout}
        data-operations-desk={operationsDeskOpen ? "open" : "closed"}
        data-phase12c-action-state={phase12cActionState}
        data-phase12c-timeline-state={phase12cTimelineState}>
        <section className="mission-strip" aria-labelledby="mission-title"><div>
          <p className="eyebrow">{nominalGlobal ? "KSA-G10R · Nominal global replay" : "KSA-G10R · GNSS-loss operations"}</p><h2 id="mission-title">{overall}</h2></div>
          <dl className="mission-metrics">
            <div><dt>MET</dt><dd>{demo ? "06:22.406" : met(displayedMissionTime)}</dd></div>
            <div><dt>Frame</dt><dd>{demo ? "GCRF" : frameName(displayedFrame)}</dd></div>
            <div><dt>Package</dt><dd>{nominalGlobal ? "Global Flight Computer" : "Reference Ops V1"}</dd></div>
            <div><dt>Release</dt><dd>{demo ? "12,237" : displayedRelease.toLocaleString()} / {totalReleases.toLocaleString()}</dd></div>
          </dl><div className="phase-progress"><span>Mission progress</span><progress value={demo ? 56.7 : progress} max="100">{progress.toFixed(1)}%</progress>
            <strong>{demo ? "56.7" : progress.toFixed(1)}%</strong></div></section>

        <GlobalMissionViewer model={globalDisplay} replay={nominalGlobal || transport.kind === "replay" || phase12cCompletedReplay}
          layout={viewerLayout} deskOpen={operationsDeskOpen}
          onLayoutChange={(value) => {
            setViewerLayout(value);
            if (value !== "cinematic") setOperationsDeskOpen(true);
          }}
          onDeskOpenChange={setOperationsDeskOpen} />

        <section className="panel trajectory-panel operations-panel"><TrajectoryPlot planned={plannedTrajectory}
          onboard={onboardPath} ground={groundPath} apogeeKm={demo ? 249.8 : snapshot === undefined ? undefined : q12(snapshot.prediction.apogeeQ12Km)} /></section>

        <section className="panel procedure-panel operations-panel" aria-labelledby="procedure-title"><div className="panel-heading"><div>
          <p className="eyebrow">{demo ? "ASCENT / LOSS OF GPS AIDING" : liveProcedure?.title ?? "Operational procedures"}</p>
          <h2 id="procedure-title">{liveProcedure === undefined && !demo ? "Awaiting procedure" : "Active procedure"}</h2></div>
          <span className="count-pill">{demo ? "2 / 4" : liveProcedure === undefined ? "—" : `${liveProcedure.activeStep} / ${liveProcedure.stepCount}`}</span></div>
          <ol className="procedure-list">{procedure.length === 0 ? <li data-state="pending"><span className="step-number">—</span><span>
            <strong>No active procedure publication</strong><small>Waiting for the role-filtered session stream.</small></span></li> : procedure.map((step, index) => (
            <li key={step.id} data-state={step.status}><span className="step-number">{String(index + 1).padStart(2, "0")}</span><span>
              <strong>{step.title}</strong><small>{step.detail}</small></span></li>))}</ol>
          <div className="command-card" aria-labelledby="command-title"><div><p className="eyebrow">Atomic ground action</p>
            <h3 id="command-title">{demo ? "Proposal GSU-04" : proposal?.label ?? "No current proposal"}</h3>
            <p>{demo ? "Residual 0.41 km · bounded demonstration" : proposal === undefined ? "Rust has not published an actionable proposal." :
              `Load ${proposal.loadIdentity.toString(16).toUpperCase()} · activates release ${proposal.activationEpoch.toLocaleString()}`}</p></div>
            <div className="command-flow" aria-label="Atomic action sequence">{(["review", "stage", "commit"] as const).map((kind) => (
              <button key={kind} type="button" disabled={!isEnabled(kind)} data-complete={demo ? demoActionIndex > demoFlow.indexOf(kind) : acceptedOperations.has(OPERATION_CODE[kind])}
                onClick={() => void performAction(kind)}>{kind}</button>))}
              <button className="cancel-button" type="button" disabled={!isEnabled("cancel")} onClick={() => void performAction("cancel")}>cancel</button></div>
            <p className="action-feedback" role="status">{actionError ?? (demo ? demoActionIndex === 0 ? "Proposal awaits review." : demoActionIndex === 1 ?
              "Review recorded. Load may now be staged." : demoActionIndex === 2 ? "Load staged. Commit remains separate." : "Commit accepted in the demonstration." :
              receiptFeedback(state.receipts, pendingAction))}</p></div>
        </section>

        <section className="panel nav-panel operations-panel" aria-labelledby="nav-title"><div className="panel-heading"><div><p className="eyebrow">Independent estimates</p>
          <h2 id="nav-title">Navigation residuals</h2></div><span className={snapshot?.gnssState === 2 || demo ? "warning-badge" : "count-pill"}>
            {demo || snapshot?.gnssState === 2 ? "GNSS invalid" : snapshot === undefined ? "Awaiting" : "GNSS healthy"}</span></div>
          <dl className="residual-grid"><div><dt>Position</dt><dd>{demo ? "0.41 km" : positionResidual === undefined ? "Pending" : `${positionResidual.toFixed(2)} km`}</dd><span>onboard ↔ ground estimate</span></div>
            <div><dt>Velocity</dt><dd>{demo ? "1.82 m/s" : velocityResidual === undefined ? "Pending" : `${velocityResidual.toFixed(2)} m/s`}</dd><span>independent residual</span></div>
            <div><dt>Prediction</dt><dd>{!predictionValid ? "Pending" : `${q16(snapshot.prediction.timeToApogeeQ16).toFixed(1)} s`}</dd><span>time to predicted apogee</span></div>
            <div><dt>Transport</dt><dd>{state.transport === undefined ? state.connection : ["", "current", "delayed", "stale", "disconnected", "resync"][state.transport.staleness]}</dd>
              <span>{state.transport?.samplesPending ?? 0} samples pending</span></div></dl>
          <div className="mini-bars" aria-label="Navigation residual gate utilization"><div><span>Position gate</span>
            <meter value={Math.min(positionResidual ?? 0, 0.75)} min="0" max="0.75">position gate</meter></div><div><span>Velocity gate</span>
            <meter value={Math.min(velocityResidual ?? 0, 3)} min="0" max="3">velocity gate</meter></div></div></section>

        <section className="panel timeline-panel operations-panel" aria-labelledby="timeline-title"><div className="panel-heading"><div><p className="eyebrow">Operational record</p>
          <h2 id="timeline-title">Event timeline</h2></div><span className="count-pill">{timeline.length} events</span></div>
          <ol className="timeline">{timeline.length === 0 ? <li data-severity="nominal"><time>—</time><span><strong>Awaiting events</strong>
            <small>Timeline records are ordered and never coalesced.</small></span></li> : timeline.map((event, index) => (
            <li key={`${event.time}-${event.title}-${index}`} data-severity={event.severity}><time>{event.time}</time><span><strong>{event.title}</strong>
              <small>{event.detail}</small></span></li>))}</ol></section>

        <section className="panel disposition-panel operations-panel" aria-labelledby="disposition-title"><div className="panel-heading"><div><p className="eyebrow">Not a single pass/fail bit</p>
          <h2 id="disposition-title">Mission disposition</h2></div><span className="count-pill">{overall}</span></div>
          <dl className="outcome-grid">{axes.map((axis) => <div key={axis.label} data-state={axis.state}><dt>{axis.label}</dt><dd>{axis.value}</dd></div>)}</dl>
          <div className="evidence-card" data-complete={evidenceComplete}><span className="integrity-mark" aria-hidden="true">{evidenceComplete ? "✓" : "…"}</span><span>
            <strong>{demo ? "Demonstration evidence label" : evidenceComplete ? "Evidence chain sealed" : state.evidence?.complete === true ? "Evidence transfer verifying" : snapshot?.lifecycle === 7 ? "Evidence incomplete" : "Evidence accumulating"}</strong>
            <small>{demo ? "Static fixture · not a completed archive" : `${snapshot?.releaseEpoch ?? 0} ordered releases · ${snapshot?.actionCount ?? 0} accepted actions · ${evidenceReceived}/${(state.evidence?.totalLength ?? 0n).toString()} bytes verified`}</small>
          </span>{evidenceComplete && state.sealedEvidence !== undefined ? <button type="button" onClick={() => downloadEvidence(state.sealedEvidence!)}>Save KSB11</button> : null}</div></section>

      </main><footer><span>KSA64 role-filtered engineering presentation</span><span>{demo ? "DEMONSTRATION FIXTURE · no live authority" :
        transport.kind === "replay" ? "Read-only role-filtered replay" : "Rust owns world, flight, operations, and evidence"}</span></footer>
    </div>
  );
}
