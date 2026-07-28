import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalJson, computePhase12cWebSourceIdentity } from "./phase12c-source-identity.mjs";

function argumentsOf(argv) {
  const values = new Map();
  const screenshots = [];
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    const value = argv[index + 1];
    if (key === "--screenshot" && value !== undefined) {
      screenshots.push(value);
      index += 1;
    } else if (key?.startsWith("--") && value !== undefined) {
      values.set(key, value);
      index += 1;
    }
  }
  return { values, screenshots };
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function readJson(path) {
  const bytes = readFileSync(path);
  return { bytes, value: JSON.parse(bytes.toString("utf8")) };
}

function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}

function portablePath(path) {
  const local = relative(repositoryRoot, path);
  return local !== "" && !local.startsWith(`..${sep}`) && local !== ".."
    ? local.split(sep).join("/")
    : path;
}

function screenshotRecord(specification) {
  const separator = specification.indexOf("=");
  requireValue(separator > 0, "--screenshot must use label=path");
  const label = specification.slice(0, separator);
  const path = resolve(specification.slice(separator + 1));
  const bytes = readFileSync(path);
  return { label, path: portablePath(path), bytes: bytes.byteLength, sha256: sha256(bytes) };
}

const here = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(here, "../..");
const { values, screenshots } = argumentsOf(process.argv.slice(2));
const nominalPath = values.get("--nominal");
const guidedPath = values.get("--guided");
const outputPath = values.get("--output");
requireValue(nominalPath !== undefined && guidedPath !== undefined && outputPath !== undefined,
  "usage: node scripts/write-phase12c-browser-evidence.mjs --nominal RAW.json --guided RAW.json --output MANIFEST.json [--screenshot label=path]");

const nominal = readJson(resolve(nominalPath));
const guided = readJson(resolve(guidedPath));
requireValue(nominal.value.schema === "ksa64.phase12c.browser-producer.v1", "nominal producer schema mismatch");
requireValue(guided.value.schema === "ksa64.phase12c.browser-role-producer.v1", "guided producer schema mismatch");
requireValue(nominal.value.experience === "nominal-global", "nominal producer experience mismatch");
requireValue(guided.value.experience === "gnss-loss", "guided producer experience mismatch");

const built = nominal.value.build_identity;
requireValue(canonicalJson(built) === canonicalJson(guided.value.build_identity),
  "nominal and guided evidence used different web builds");
const current = computePhase12cWebSourceIdentity(repositoryRoot);
requireValue(built.schema === "ksa64.phase12c.web-source-identity.v1" &&
  built.commit === current.commit && built.file_count === current.file_count,
  "rendered build source identity does not match the current checkout");
requireValue(built.tree_sha256 === current.tree_sha256,
  `rendered build source hash ${built.tree_sha256} does not match current source ${current.tree_sha256}`);
requireValue(built.dirty === false && current.dirty === false,
  "browser completion evidence must come from a clean, commit-qualified web source tree");
const authorityWasm = built.files?.find((entry) => entry.path === "web/public/wasm/ksa64-session.wasm");
requireValue(authorityWasm !== undefined && authorityWasm.bytes > 0 && /^[0-9a-f]{64}$/.test(authorityWasm.sha256),
  "rendered build identity does not bind public/wasm/ksa64-session.wasm");

const requiredMilestones = [29, 1920, 3579, 8124, 12669, 15255, 15257, 20929, 22014];
requireValue(canonicalJson(nominal.value.milestones.map((entry) => entry.release_epoch)) ===
  canonicalJson(requiredMilestones), "nominal milestone set mismatch");
for (const milestone of nominal.value.milestones) {
  requireValue(milestone.semantic?.releaseEpoch === milestone.release_epoch,
    `semantic release mismatch at ${milestone.release_epoch}`);
  requireValue(sha256(Buffer.from(canonicalJson(milestone.semantic), "utf8")) === milestone.semantic_sha256,
    `semantic hash mismatch at ${milestone.release_epoch}`);
  requireValue(sha256(Buffer.from(canonicalJson(milestone.authority_state), "utf8")) === milestone.authority_sha256,
    `authority-state hash mismatch at ${milestone.release_epoch}`);
}
const commonMilestone = nominal.value.milestones.find((entry) =>
  entry.release_epoch === nominal.value.invariance.release_epoch);
requireValue(commonMilestone !== undefined, "invariance release is absent from milestone evidence");
requireValue(nominal.value.invariance.semantic_sha256.every((value) =>
  value === commonMilestone.semantic_sha256), "backend semantic hashes do not bind the reviewed common milestone");
requireValue(nominal.value.backends.webgl2.actual === "webgl2" &&
  nominal.value.backends.webgl2.frames_per_second >= 30 &&
  nominal.value.backends.webgl2.canvas.width > 0 && nominal.value.backends.webgl2.canvas.height > 0,
  "forced WebGL2 did not render a measured 30 fps canvas");
requireValue(nominal.value.backends.two_d.actual === "2d" &&
  nominal.value.backends.two_d.canvas.width > 0 && nominal.value.backends.two_d.canvas.height > 0,
  "forced 2-D did not render an operational canvas");
requireValue(["webgpu", "webgl2", "2d"].includes(nominal.value.backends.auto.actual),
  "automatic renderer produced an unknown backend");
requireValue(nominal.value.context_loss.before_backend === "webgl2" &&
  nominal.value.context_loss.after_backend === "2d", "WebGL context loss did not visibly fall back to 2-D");
requireValue(nominal.value.context_loss.before_semantic_sha256 ===
  nominal.value.context_loss.after_semantic_sha256, "context loss changed the semantic scene");
requireValue(nominal.value.invariance.semantic_sha256.every((value) =>
  value === nominal.value.invariance.semantic_sha256[0]), "renderer selection changed the semantic scene");
requireValue(nominal.value.invariance.authority_sha256.every((value) =>
  value === nominal.value.invariance.authority_sha256[0]), "renderer selection changed authority-facing state");
requireValue(nominal.value.role.sim_director === true &&
  nominal.value.role.truth_default_hidden === true &&
  nominal.value.role.truth_opt_in_labeled === true, "SIM Director truth policy failed");
requireValue(guided.value.role.guided_operator === true &&
  guided.value.role.truth_control_absent === true &&
  guided.value.role.truth_source_absent === true, "Guided Operator received SIM truth presentation");

const requiredOperationalMilestones = [
  { release_epoch: 5_760, kind: "fault" },
  { release_epoch: 5_824, kind: "fault" },
  { release_epoch: 6_080, kind: "action" },
  { release_epoch: 6_240, kind: "action" },
  { release_epoch: 6_560, kind: "action" },
  { release_epoch: 6_720, kind: "action" },
];
requireValue(canonicalJson(guided.value.operational_milestones?.map(({ release_epoch, kind }) =>
  ({ release_epoch, kind }))) === canonicalJson(requiredOperationalMilestones),
"guided operational milestone set mismatch");
for (const milestone of guided.value.operational_milestones) {
  requireValue(milestone.semantic?.releaseEpoch === milestone.release_epoch,
    `guided semantic release mismatch at ${milestone.release_epoch}`);
  requireValue(sha256(Buffer.from(canonicalJson(milestone.semantic), "utf8")) === milestone.semantic_sha256,
    `guided semantic hash mismatch at ${milestone.release_epoch}`);
  requireValue(sha256(Buffer.from(canonicalJson(milestone.authority_state), "utf8")) === milestone.authority_sha256,
    `guided authority-state hash mismatch at ${milestone.release_epoch}`);
}
const requiredAcceptedActions = [
  { release: 6_080, operation: 2 },
  { release: 6_240, operation: 3 },
  { release: 6_560, operation: 2 },
  { release: 6_720, operation: 3 },
];
requireValue(guided.value.accepted_action_count === 4 &&
  Array.isArray(guided.value.accepted_action_receipts) && guided.value.accepted_action_receipts.length === 4,
"guided accepted-action count mismatch");
for (const expected of requiredAcceptedActions) {
  requireValue(guided.value.accepted_action_receipts.some((receipt) =>
    receipt.accepted === true && receipt.receiptEpoch === expected.release && receipt.operation === expected.operation),
  `guided accepted action is missing at release ${expected.release}`);
}
requireValue(guided.value.fault_policy?.persistent_gnss_outage === true &&
  guided.value.fault_policy.outage_release === 5_760 &&
  guided.value.fault_policy.qualified_release === 5_824 &&
  guided.value.fault_policy.reacquisition_event === null,
"guided persistent-GNSS-loss policy mismatch");

const screenshotRecords = screenshots.map(screenshotRecord);
requireValue(screenshotRecords.length >= 3 && new Set(screenshotRecords.map((record) => record.label)).size === screenshotRecords.length,
  "browser completion evidence requires at least three uniquely labelled rendered screenshots");
const webgpu = nominal.value.backends.auto.actual === "webgpu"
  ? { status: "rendered", frames_per_second: nominal.value.backends.auto.frames_per_second }
  : {
      status: "unavailable",
      navigator_gpu_present: nominal.value.environment.navigator_gpu_present,
      selected_fallback: nominal.value.backends.auto.actual,
      reason: nominal.value.backends.auto.detail,
    };
const manifest = {
  schema: "ksa64.phase12c.browser-evidence.v1",
  producer: {
    kind: "rendered-browser-phase12c-harness",
    nominal_raw: { path: portablePath(resolve(nominalPath)), bytes: nominal.bytes.byteLength, sha256: sha256(nominal.bytes) },
    guided_raw: { path: portablePath(resolve(guidedPath)), bytes: guided.bytes.byteLength, sha256: sha256(guided.bytes) },
    screenshots: screenshotRecords,
  },
  source: {
    commit: built.commit,
    dirty: built.dirty,
    tree_sha256: built.tree_sha256,
    file_count: built.file_count,
    authority_wasm: authorityWasm,
  },
  environment: nominal.value.environment,
  backends: {
    webgpu,
    webgl2: {
      status: "rendered",
      frames_per_second: nominal.value.backends.webgl2.frames_per_second,
      semantic_sha256: nominal.value.backends.webgl2.semantic_sha256,
      canvas: nominal.value.backends.webgl2.canvas,
    },
    two_d: {
      status: "rendered",
      frames_per_second: nominal.value.backends.two_d.frames_per_second,
      semantic_sha256: nominal.value.backends.two_d.semantic_sha256,
      canvas: nominal.value.backends.two_d.canvas,
    },
  },
  context_loss: nominal.value.context_loss,
  semantic_milestones: nominal.value.milestones,
  role_filtering: { sim_director: nominal.value.role, guided_operator: guided.value.role },
  operational_milestones: guided.value.operational_milestones,
  guided_actions: {
    accepted_action_count: guided.value.accepted_action_count,
    accepted_action_receipts: guided.value.accepted_action_receipts,
  },
  guided_fault_policy: guided.value.fault_policy,
  evidence_invariance: nominal.value.invariance,
  pass: true,
};
const resolvedOutput = resolve(outputPath);
mkdirSync(dirname(resolvedOutput), { recursive: true });
writeFileSync(resolvedOutput, `${canonicalJson(manifest)}\n`, "utf8");
console.log(`wrote ${resolvedOutput}`);
