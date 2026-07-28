const SCHEMA = "ksa64.phase12c.browser-producer.v1" as const;
const ROLE_SCHEMA = "ksa64.phase12c.browser-role-producer.v1" as const;
const REQUIRED_RELEASES = [29, 1920, 3579, 8124, 12_669, 15_255, 15_257, 20_929, 22_014] as const;

type RendererPreference = "auto" | "webgl2" | "2d";
type RendererBackend = "webgpu" | "webgl2" | "2d";

interface BuildIdentity {
  readonly schema: string;
  readonly commit: string;
  readonly dirty: boolean;
  readonly tree_sha256: string;
  readonly file_count: number;
}

interface SemanticRecord {
  readonly schema: string;
  readonly releaseEpoch: number;
  readonly frame: string;
  readonly segment: string;
  readonly truthLabelVisible: boolean;
  readonly sources: readonly { readonly source: string }[];
}

interface BackendEvidence {
  readonly requested: RendererPreference;
  readonly actual: RendererBackend;
  readonly detail: string;
  readonly frames_observed: number;
  readonly elapsed_milliseconds: number;
  readonly frames_per_second: number;
  readonly semantic_sha256: string;
  readonly authority_sha256: string;
  readonly canvas: { readonly width: number; readonly height: number; readonly css_width: number; readonly css_height: number };
}

interface Phase12cBrowserEvidenceApi {
  readonly schema: "ksa64.phase12c.browser-harness-api.v1";
  waitUntilReady(timeoutMilliseconds?: number): Promise<void>;
  captureRolePolicy(): Promise<unknown>;
  runGuided(): Promise<unknown>;
  runNominal(options?: { readonly fpsMilliseconds?: number }): Promise<unknown>;
  snapshot(label?: string): Promise<unknown>;
}

declare global {
  interface Window {
    __KSA64_PHASE12C_EVIDENCE__?: Phase12cBrowserEvidenceApi;
  }
}

function wait(delayMilliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, delayMilliseconds));
}

async function waitFor<T>(
  read: () => T | undefined,
  timeoutMilliseconds: number,
  description: string,
): Promise<T> {
  const deadline = performance.now() + timeoutMilliseconds;
  while (performance.now() < deadline) {
    const value = read();
    if (value !== undefined) return value;
    await wait(25);
  }
  throw new Error(`timed out waiting for ${description}`);
}

function viewer(): HTMLElement | undefined {
  const value = document.querySelector<HTMLElement>(".global-viewer-panel");
  return value ?? undefined;
}

function semantic(): SemanticRecord | undefined {
  const raw = viewer()?.dataset.semanticScene;
  if (raw === undefined) return undefined;
  const value = JSON.parse(raw) as SemanticRecord;
  return value.schema === "ksa64.global-scene-semantic.v1" ? value : undefined;
}

function activeBackend(): RendererBackend | undefined {
  const value = document.querySelector<HTMLElement>(".global-viewer-status [data-state]")?.dataset.state;
  return value === "webgpu" || value === "webgl2" || value === "2d" ? value : undefined;
}

function selectWithLabel(label: string): HTMLSelectElement {
  const values = [...document.querySelectorAll<HTMLSelectElement>("select")];
  const value = values.find((candidate) => candidate.getAttribute("aria-label") === label);
  if (value === undefined) throw new Error(`select is missing: ${label}`);
  return value;
}

function setSelect(label: string, value: string): void {
  const select = selectWithLabel(label);
  const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")?.set;
  if (setter === undefined) throw new Error("browser select value setter is unavailable");
  setter.call(select, value);
  select.dispatchEvent(new Event("change", { bubbles: true }));
}

function releaseInput(): HTMLInputElement {
  const value = document.querySelector<HTMLInputElement>(".release-scrubber input[type=range]");
  if (value === null) throw new Error("exact release scrubber is missing");
  return value;
}

function setRelease(releaseEpoch: number): void {
  const input = releaseInput();
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  if (setter === undefined) throw new Error("browser input value setter is unavailable");
  setter.call(input, String(releaseEpoch));
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

function truthInput(): HTMLInputElement | undefined {
  return [...document.querySelectorAll<HTMLInputElement>(".global-source-legend input[type=checkbox]")]
    .find((value) => value.closest("label")?.dataset.source === "truth");
}

function setChecked(input: HTMLInputElement, checked: boolean): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "checked")?.set;
  if (setter === undefined) throw new Error("browser checkbox setter is unavailable");
  setter.call(input, checked);
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>;
    return `{${Object.keys(record).sort().map((key) =>
      `${JSON.stringify(key)}:${canonical(record[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

async function sha256(value: unknown): Promise<string> {
  const bytes = new TextEncoder().encode(canonical(value));
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, "0")).join("");
}

function authorityState() {
  const axes = [...document.querySelectorAll<HTMLElement>(".outcome-grid > div")].map((axis) => ({
    label: axis.querySelector("dt")?.textContent?.trim() ?? "",
    value: axis.querySelector("dd")?.textContent?.trim() ?? "",
    state: axis.dataset.state ?? "",
  }));
  return {
    mission: document.querySelector(".mission-strip")?.textContent?.replace(/\s+/gu, " ").trim() ?? null,
    evidence: document.querySelector(".evidence-card")?.textContent?.replace(/\s+/gu, " ").trim() ?? null,
    disposition: document.querySelector(".disposition-panel")?.textContent?.replace(/\s+/gu, " ").trim() ?? null,
    connection: document.querySelector(".topbar-status")?.textContent?.replace(/\s+/gu, " ").trim() ?? null,
    overall: document.querySelector(".mission-strip h2")?.textContent?.trim() ?? null,
    axes,
  };
}

async function buildIdentity(): Promise<BuildIdentity> {
  const response = await fetch("/phase12c-build-identity.json", { cache: "no-store" });
  if (!response.ok) throw new Error("Phase 12C web build identity is unavailable");
  const value = await response.json() as BuildIdentity;
  if (value.schema !== "ksa64.phase12c.web-source-identity.v1" ||
      !/^[0-9a-f]{64}$/u.test(value.tree_sha256) || value.file_count < 1) {
    throw new Error("Phase 12C web build identity is invalid");
  }
  return value;
}

async function waitForSemanticRelease(releaseEpoch: number): Promise<SemanticRecord> {
  return waitFor(() => {
    const value = semantic();
    return value?.releaseEpoch === releaseEpoch ? value : undefined;
  }, 10_000, `semantic release ${releaseEpoch}`);
}

async function selectBackend(preference: RendererPreference): Promise<RendererBackend> {
  const select = selectWithLabel("Global renderer backend");
  const priorPreference = select.value;
  const before = activeBackend();
  setSelect("Global renderer backend", preference);
  if (preference === "auto" && priorPreference === "auto" && before !== undefined) {
    return before;
  }
  // Let React commit the preference and dispose the previous renderer before
  // accepting the newly announced backend.
  await wait(100);
  return waitFor(() => {
    const value = activeBackend();
    if (value === undefined) return undefined;
    if (preference === "2d") return value === "2d" ? value : undefined;
    if (preference === "webgl2") return value === "webgl2" || value === "2d" ? value : undefined;
    return value;
  }, 20_000, `${preference} renderer`);
}

async function measureFrames(durationMilliseconds: number): Promise<{
  readonly frames_observed: number;
  readonly elapsed_milliseconds: number;
  readonly frames_per_second: number;
}> {
  const started = performance.now();
  let finished = started;
  let frames = 0;
  await new Promise<void>((resolve) => {
    const count = (now: number): void => {
      finished = now;
      frames += 1;
      if (now - started >= durationMilliseconds) resolve();
      else requestAnimationFrame(count);
    };
    requestAnimationFrame(count);
  });
  const elapsed = Math.max(1, finished - started);
  return {
    frames_observed: frames,
    elapsed_milliseconds: Number(elapsed.toFixed(3)),
    frames_per_second: Number((((frames - 1) * 1000) / elapsed).toFixed(3)),
  };
}

function canvasRecord(): BackendEvidence["canvas"] {
  const canvas = document.querySelector<HTMLCanvasElement>(".global-render-stage canvas");
  if (canvas === null) throw new Error("global renderer canvas is missing");
  const bounds = canvas.getBoundingClientRect();
  return {
    width: canvas.width,
    height: canvas.height,
    css_width: Number(bounds.width.toFixed(3)),
    css_height: Number(bounds.height.toFixed(3)),
  };
}

async function captureBackend(
  preference: RendererPreference,
  durationMilliseconds: number,
  releaseEpoch: number,
): Promise<BackendEvidence> {
  setRelease(releaseEpoch);
  await waitForSemanticRelease(releaseEpoch);
  const actual = await selectBackend(preference);
  await wait(100);
  const current = semantic();
  if (current === undefined || current.releaseEpoch !== releaseEpoch) {
    throw new Error("renderer selection changed the selected semantic release");
  }
  const frames = await measureFrames(durationMilliseconds);
  return {
    requested: preference,
    actual,
    detail: preference === "auto" && actual !== "webgpu"
      ? `WebGPU was not selected; automatic fallback rendered through ${actual}`
      : `${preference} selected ${actual}`,
    ...frames,
    semantic_sha256: await sha256(current),
    authority_sha256: await sha256(authorityState()),
    canvas: canvasRecord(),
  };
}

async function rolePolicy() {
  const current = await waitFor(semantic, 10_000, "role-filtered semantic scene");
  const identity = document.querySelector(".topbar .eyebrow")?.textContent ?? "";
  const input = truthInput();
  const simDirector = /SIM Director/iu.test(identity);
  const guidedOperator = /Guided Operator/iu.test(identity);
  const initialTruth = current.sources.some((value) => value.source === "truth");
  let optInLabeled = false;
  if (input !== undefined) {
    setChecked(input, true);
    const visible = await waitFor(() => {
      const value = semantic();
      return value?.truthLabelVisible === true ? value : undefined;
    }, 5_000, "truth opt-in label");
    optInLabeled = visible.sources.some((value) => value.source === "truth") &&
      document.querySelector(".sim-truth-watermark")?.textContent?.trim() === "SIM TRUTH";
    setChecked(input, false);
    await waitFor(() => semantic()?.truthLabelVisible === false ? true : undefined, 5_000, "truth hidden state");
  }
  return {
    sim_director: simDirector,
    guided_operator: guidedOperator,
    truth_control_present: input !== undefined,
    truth_control_absent: input === undefined,
    truth_default_hidden: !initialTruth && current.truthLabelVisible === false,
    truth_source_absent: input === undefined && !initialTruth,
    truth_opt_in_labeled: optInLabeled,
  };
}

async function runNominal(options: { readonly fpsMilliseconds?: number } = {}) {
  await waitFor(() => {
    const element = viewer();
    const input = document.querySelector<HTMLInputElement>(".release-scrubber input[type=range]");
    return element?.dataset.quality === "global-display-v1" &&
      Number(input?.max) >= 22_014 && semantic()?.releaseEpoch === 22_014 ? true : undefined;
  }, 120_000, "complete nominal GlobalDisplayV1 replay");
  setSelect("Global display motion", "exact");
  const duration = Math.max(500, Math.min(options.fpsMilliseconds ?? 1_500, 10_000));
  const build = await buildIdentity();
  const role = await rolePolicy();
  const commonRelease = 8_124;
  const auto = await captureBackend("auto", duration, commonRelease);
  const webgl2 = await captureBackend("webgl2", duration, commonRelease);
  const twoD = await captureBackend("2d", duration, commonRelease);

  setRelease(commonRelease);
  await waitForSemanticRelease(commonRelease);
  const beforeBackend = await selectBackend("webgl2");
  const beforeSemantic = semantic();
  if (beforeBackend !== "webgl2" || beforeSemantic === undefined) {
    throw new Error("WebGL2 is required for the context-loss evidence probe");
  }
  const beforeHash = await sha256(beforeSemantic);
  const canvas = document.querySelector<HTMLCanvasElement>(".global-render-stage canvas");
  if (canvas === null) throw new Error("WebGL2 canvas is missing");
  canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
  const afterBackend = await waitFor(() => activeBackend() === "2d" ? "2d" as const : undefined,
    5_000, "context-loss 2-D fallback");
  const afterSemantic = semantic();
  if (afterSemantic === undefined) throw new Error("semantic scene vanished during context-loss fallback");
  const afterHash = await sha256(afterSemantic);

  const milestones = [];
  for (const releaseEpoch of REQUIRED_RELEASES) {
    setRelease(releaseEpoch);
    const value = await waitForSemanticRelease(releaseEpoch);
    const milestoneAuthority = authorityState();
    milestones.push({
      release_epoch: releaseEpoch,
      semantic_sha256: await sha256(value),
      authority_sha256: await sha256(milestoneAuthority),
      semantic: value,
      authority_state: milestoneAuthority,
    });
  }
  setRelease(commonRelease);
  await waitForSemanticRelease(commonRelease);
  const finalAuthorityHash = await sha256(authorityState());
  return {
    schema: SCHEMA,
    experience: "nominal-global",
    build_identity: build,
    environment: {
      user_agent: navigator.userAgent,
      platform: navigator.platform,
      navigator_gpu_present: "gpu" in navigator,
      device_pixel_ratio: window.devicePixelRatio,
      viewport: { width: window.innerWidth, height: window.innerHeight },
    },
    backends: { auto, webgl2, two_d: twoD },
    context_loss: {
      before_backend: beforeBackend,
      after_backend: afterBackend,
      before_semantic_sha256: beforeHash,
      after_semantic_sha256: afterHash,
    },
    milestones,
    role,
    invariance: {
      release_epoch: commonRelease,
      semantic_sha256: [auto.semantic_sha256, webgl2.semantic_sha256, twoD.semantic_sha256],
      authority_sha256: [auto.authority_sha256, webgl2.authority_sha256, twoD.authority_sha256, finalAuthorityHash],
    },
  };
}

interface GuidedActionState {
  readonly release_epoch: number;
  readonly lifecycle: number;
  readonly action_count: number;
  readonly proposal?: { readonly proposalIdentity: number };
  readonly receipts: readonly {
    readonly proposalIdentity: number;
    readonly receiptEpoch: number;
    readonly accepted: boolean;
    readonly operation: number;
    readonly receiptChecksum: number;
  }[];
}

interface GuidedTimelineRecord {
  readonly releaseEpoch: number;
  readonly sourceIdentity: number;
  readonly severity: number;
  readonly eventIdentity: number;
  readonly detailIdentity: number;
  readonly label: string;
}

function evidenceDataset<T>(name: string): T | undefined {
  const raw = document.querySelector<HTMLElement>("#main-content")?.dataset[name];
  return raw === undefined ? undefined : JSON.parse(raw) as T;
}

function guidedActionState(): GuidedActionState | undefined {
  return evidenceDataset<GuidedActionState>("phase12cActionState");
}

function guidedTimeline(): readonly GuidedTimelineRecord[] {
  return evidenceDataset<readonly GuidedTimelineRecord[]>("phase12cTimelineState") ?? [];
}

function actionButton(kind: "review" | "stage" | "commit"): HTMLButtonElement | undefined {
  return [...document.querySelectorAll<HTMLButtonElement>(".command-flow button")]
    .find((button) => button.textContent?.trim().toLowerCase() === kind);
}

async function submitGuidedAction(
  kind: "review" | "stage" | "commit",
  operation: number,
  releaseEpoch: number,
): Promise<void> {
  const before = guidedActionState()?.receipts.length ?? 0;
  const button = await waitFor(() => {
    const value = actionButton(kind);
    return value !== undefined && !value.disabled ? value : undefined;
  }, 15_000, "enabled " + kind + " action at release " + releaseEpoch);
  button.click();
  await waitFor(() => {
    const state = guidedActionState();
    const accepted = state?.receipts.some((receipt, index) =>
      index >= before && receipt.accepted && receipt.operation === operation &&
      receipt.receiptEpoch === releaseEpoch);
    return accepted ? true : undefined;
  }, 15_000, "accepted " + kind + " receipt at release " + releaseEpoch);
}

async function resumeFastThroughStagedGate(targetRelease: number): Promise<void> {
  while ((guidedActionState()?.release_epoch ?? 0) < targetRelease) {
    const before = guidedActionState()?.release_epoch ?? 0;
    setSelect("Session pace", "fast");
    const after = await waitFor(() => {
      const release = guidedActionState()?.release_epoch ?? 0;
      return release > before ? release : undefined;
    }, 10_000, "guided progress toward release " + targetRelease);
    if (after > targetRelease) throw new Error("guided authority skipped action release " + targetRelease);
  }
}

async function performAcceptedGuidedTranscript(): Promise<void> {
  setSelect("Session pace", "fast");
  await waitFor(() => {
    const state = guidedActionState();
    return state?.release_epoch === 6_080 && state.proposal !== undefined ? true : undefined;
  }, 120_000, "first guided action proposal");
  await submitGuidedAction("review", 1, 6_080);
  await submitGuidedAction("stage", 2, 6_080);
  await resumeFastThroughStagedGate(6_240);
  await submitGuidedAction("commit", 3, 6_240);

  await waitFor(() => {
    const state = guidedActionState();
    return state?.release_epoch === 6_560 && state.proposal !== undefined ? true : undefined;
  }, 120_000, "second guided action proposal");
  await submitGuidedAction("review", 1, 6_560);
  await submitGuidedAction("stage", 2, 6_560);
  await resumeFastThroughStagedGate(6_720);
  await submitGuidedAction("commit", 3, 6_720);
  setSelect("Session pace", "fast");

  await waitFor(() => {
    const state = guidedActionState();
    return state?.lifecycle === 5 && state.release_epoch === 21_591 && state.action_count === 4
      ? true : undefined;
  }, 180_000, "complete four-action guided mission");
  await waitFor(() => !releaseInput().disabled ? true : undefined, 10_000, "completed-session replay controls");
}

async function guidedMilestone(
  releaseEpoch: number,
  kind: "fault" | "action",
  metadata: unknown,
) {
  setRelease(releaseEpoch);
  const value = await waitForSemanticRelease(releaseEpoch);
  const authority = authorityState();
  return {
    release_epoch: releaseEpoch,
    kind,
    frame: value.frame,
    segment: value.segment,
    sources: value.sources.map((source) => source.source),
    semantic_sha256: await sha256(value),
    authority_sha256: await sha256(authority),
    semantic: value,
    authority_state: authority,
    metadata,
  };
}

async function runGuided() {
  await waitFor(() => viewer()?.dataset.quality === "global-display-v1" && semantic() !== undefined
    ? true : undefined, 120_000, "guided GlobalDisplayV1 scene");
  const build = await buildIdentity();
  await performAcceptedGuidedTranscript();
  setSelect("Global display motion", "exact");

  const state = guidedActionState();
  if (state === undefined) throw new Error("guided action evidence state is unavailable");
  const acceptedActions = state.receipts.filter((receipt) =>
    receipt.accepted && (receipt.operation === 2 || receipt.operation === 3));
  const expectedActions = [
    { release: 6_080, operation: 2 },
    { release: 6_240, operation: 3 },
    { release: 6_560, operation: 2 },
    { release: 6_720, operation: 3 },
  ];
  for (const expected of expectedActions) {
    if (!acceptedActions.some((receipt) => receipt.receiptEpoch === expected.release &&
        receipt.operation === expected.operation)) {
      throw new Error("accepted guided action missing at release " + expected.release);
    }
  }
  const timeline = guidedTimeline();
  const outage = timeline.find((event) => event.releaseEpoch === 5_760 &&
    event.label === "GNSS observations missing");
  const qualified = timeline.find((event) => event.releaseEpoch === 5_824 &&
    event.label === "GNSS loss qualified after three missing fixes");
  if (outage === undefined || qualified === undefined) {
    throw new Error("persistent GNSS fault timeline evidence is incomplete");
  }

  const milestones = [
    await guidedMilestone(5_760, "fault", outage),
    await guidedMilestone(5_824, "fault", qualified),
  ];
  for (const expected of expectedActions) {
    const receipt = acceptedActions.find((value) => value.receiptEpoch === expected.release &&
      value.operation === expected.operation);
    milestones.push(await guidedMilestone(expected.release, "action", receipt));
  }
  const role = await rolePolicy();
  return {
    schema: ROLE_SCHEMA,
    experience: "gnss-loss",
    build_identity: build,
    role,
    accepted_action_count: state.action_count,
    accepted_action_receipts: acceptedActions,
    fault_policy: {
      persistent_gnss_outage: true,
      outage_release: 5_760,
      qualified_release: 5_824,
      reacquisition_event: null,
      note: "The accepted scenario intentionally keeps GNSS unavailable; no reacquisition is fabricated.",
    },
    operational_milestones: milestones,
    terminal_authority_state: authorityState(),
  };
}

async function captureRolePolicy() {
  await waitFor(() => viewer()?.dataset.quality === "global-display-v1" && semantic() !== undefined
    ? true : undefined, 120_000, "role-filtered GlobalDisplayV1 scene");
  return {
    schema: ROLE_SCHEMA,
    experience: document.querySelector(".mission-strip .eyebrow")?.textContent?.includes("GNSS-loss")
      ? "gnss-loss" : "nominal-global",
    build_identity: await buildIdentity(),
    role: await rolePolicy(),
    semantic: semantic(),
  };
}

export function installPhase12cBrowserEvidenceHarness(): void {
  if (window.__KSA64_PHASE12C_EVIDENCE__ !== undefined) return;
  window.__KSA64_PHASE12C_EVIDENCE__ = {
    schema: "ksa64.phase12c.browser-harness-api.v1",
    waitUntilReady: async (timeoutMilliseconds = 120_000) => {
      await waitFor(() => viewer()?.dataset.quality === "global-display-v1" && semantic() !== undefined
        ? true : undefined, timeoutMilliseconds, "GlobalDisplayV1 browser evidence scene");
    },
    captureRolePolicy,
    runGuided,
    runNominal,
    snapshot: async (label = "manual") => ({
      label,
      backend: activeBackend(),
      semantic: semantic(),
      semantic_sha256: semantic() === undefined ? undefined : await sha256(semantic()),
      authority_sha256: await sha256(authorityState()),
      canvas: canvasRecord(),
    }),
  };
}
