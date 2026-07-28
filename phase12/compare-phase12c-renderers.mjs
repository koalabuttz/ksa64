import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const REPOSITORY_ROOT = resolve(dirname(SCRIPT_PATH), "..");

const CATALOG_SHA256 = "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13";
const REQUIRED = [
  { label: "launch", release: 29, frame: 2, frameName: "ecef", segment: 2, segmentName: "ecef-ascent" },
  { label: "burnout", release: 1920, frame: 2, frameName: "ecef", segment: 2, segmentName: "ecef-ascent" },
  { label: "coast", release: 3579, frame: 3, frameName: "gcrf", segment: 3, segmentName: "eci-coast" },
  { label: "apogee", release: 8124, frame: 3, frameName: "gcrf", segment: 3, segmentName: "eci-coast" },
  { label: "entry", release: 12669, frame: 2, frameName: "ecef", segment: 4, segmentName: "ecef-entry" },
  { label: "recovery", release: 15255, frame: 1, frameName: "local-enu", segment: 5, segmentName: "local-recovery" },
  { label: "drogue", release: 15257, frame: 1, frameName: "local-enu", segment: 5, segmentName: "local-recovery" },
  { label: "main", release: 20929, frame: 1, frameName: "local-enu", segment: 5, segmentName: "local-recovery" },
  { label: "landing", release: 22014, frame: 1, frameName: "local-enu", segment: 5, segmentName: "local-recovery" },
];
const TRANSITIONS = new Set([29, 3579, 12669, 15255]);
const OPERATIONAL_RELEASES = [5760, 5824, 6080, 6240, 6560, 6720];

function fail(message) { throw new Error(message); }
function required(condition, message) { if (!condition) fail(message); }
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function canonical(value) {
  if (Array.isArray(value)) return `[` + value.map(canonical).join(",") + `]`;
  if (value !== null && typeof value === "object") {
    return `{` + Object.keys(value).sort().map((key) => JSON.stringify(key) + ":" + canonical(value[key])).join(",") + `}`;
  }
  return JSON.stringify(value);
}
function parseArgs(argv) {
  const args = new Map();
  for (let i = 0; i < argv.length; i += 2) {
    required(argv[i]?.startsWith("--") && argv[i + 1] !== undefined, "arguments must be --name value pairs");
    args.set(argv[i], argv[i + 1]);
  }
  for (const key of ["--native", "--unreal", "--browser", "--output", "--source-commit"]) {
    required(args.has(key), "missing " + key);
  }
  return args;
}
function readJson(path, schema) {
  const absolute = resolve(path);
  const bytes = readFileSync(absolute);
  let value;
  try { value = JSON.parse(bytes.toString("utf8")); } catch (error) { fail(absolute + " is not valid JSON: " + error.message); }
  required(value.schema === schema, absolute + " schema mismatch: " + String(value.schema));
  return { absolute, bytes, value, sha256: sha256(bytes) };
}
function artifact(path) {
  const absolute = resolve(path);
  const bytes = readFileSync(absolute);
  required(bytes.byteLength > 0, "artifact is empty: " + absolute);
  return { path: absolute.replaceAll("\\", "/"), bytes: bytes.byteLength, sha256: sha256(bytes) };
}
function directoryArtifact(path, excluded = new Set()) {
  const root = resolve(path); const files = [];
  const collect = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name, "en"))) {
      const absolute = resolve(current, entry.name);
      if (entry.isDirectory()) collect(absolute); else if (entry.isFile()) files.push(absolute);
    }
  };
  collect(root); files.sort((left, right) => {
    const leftPath = relative(root, left).split(sep).join("/"); const rightPath = relative(root, right).split(sep).join("/");
    return leftPath < rightPath ? -1 : leftPath > rightPath ? 1 : 0;
  });
  const aggregate = createHash("sha256"); let bytes = 0; let fileCount = 0;
  for (const absolute of files) {
    const portable = relative(root, absolute).split(sep).join("/"); if (excluded.has(portable)) continue;
    const content = readFileSync(absolute); bytes += content.byteLength; fileCount += 1;
    aggregate.update(portable + "\0" + content.byteLength + "\0"); aggregate.update(content);
  }
  return { path: root.replaceAll("\\", "/"), bytes, file_count: fileCount, tree_sha256: aggregate.digest("hex") };
}
function resolveArtifact(owner, value) { return isAbsolute(value) ? resolve(value) : resolve(dirname(owner), value); }
function resolveWithinRoot(root, value, label) {
  required(typeof value === "string" && value.length > 0 && !isAbsolute(value) && !value.includes("\\"), label + " path is not a portable relative path");
  const parts = value.split("/");
  required(parts.every((part) => part.length > 0 && part !== "." && part !== ".."), label + " path contains a forbidden segment");
  const absolute = resolve(root, ...parts);
  const contained = relative(root, absolute);
  required(contained.length > 0 && !isAbsolute(contained) && !contained.split(sep).includes(".."), label + " path escapes its artifact root");
  return absolute;
}
function verifyPackageInventory(owner, record, sourceCommit, artifactRoot = null) {
  required(record?.measurement === "immutable packaged application payload excluding Saved" &&
    Number.isSafeInteger(record.bytes) && record.bytes > 0 &&
    Number.isSafeInteger(record.file_count) && record.file_count > 1 &&
    /^[0-9a-f]{64}$/.test(record.tree_sha256) && typeof record.inventory_file === "string" &&
    /^[0-9a-f]{64}$/.test(record.inventory_sha256), "Unreal packaged-directory inventory record is malformed");
  const inventoryPath = artifactRoot === null
    ? resolveArtifact(owner, record.inventory_file)
    : resolveWithinRoot(artifactRoot, record.inventory_file, "Unreal packaged-directory inventory");
  const inventoryArtifact = artifact(inventoryPath);
  required(inventoryArtifact.sha256 === record.inventory_sha256, "Unreal packaged-directory inventory hash mismatch");
  const inventory = readJson(inventoryPath, "ksa64.phase12c.packaged-directory-inventory.v1").value;
  required(inventory.source_commit === sourceCommit && inventory.measurement === record.measurement &&
    inventory.root === "Windows/Ksa64MissionFoundry" && !isAbsolute(inventory.root) &&
    !inventory.root.split("/").includes("..") && inventory.root.includes("\\") === false &&
    canonical(inventory.excluded) === canonical(["Saved"]) &&
    inventory.bytes === record.bytes && inventory.file_count === record.file_count &&
    inventory.tree_sha256 === record.tree_sha256 && Array.isArray(inventory.files) && inventory.files.length === record.file_count,
  "Unreal packaged-directory inventory metadata mismatch");
  const root = resolve(dirname(inventoryPath), ...inventory.root.split("/"));
  const aggregate = createHash("sha256"); const seen = new Set(); let bytes = 0; let previous = "";
  for (const entry of inventory.files) {
    required(typeof entry.path === "string" && entry.path.length > 0 && !isAbsolute(entry.path) &&
      !entry.path.split("/").includes("..") && entry.path.includes("\\") === false &&
      !entry.path.startsWith("Saved/") && entry.path !== "Saved" &&
      Number.isSafeInteger(entry.bytes) && entry.bytes >= 0 && /^[0-9a-f]{64}$/.test(entry.sha256) &&
      !seen.has(entry.path) && (previous === "" || previous < entry.path),
    "Unreal packaged-directory inventory file record is malformed or noncanonical");
    seen.add(entry.path); previous = entry.path;
    const absolute = resolve(root, ...entry.path.split("/")); const content = readFileSync(absolute);
    required(content.byteLength === entry.bytes && sha256(content) === entry.sha256,
      "Unreal packaged file mismatch: " + entry.path);
    bytes += entry.bytes; aggregate.update(entry.path + "\0" + entry.bytes + "\0" + entry.sha256 + "\n");
  }
  required(bytes === record.bytes && aggregate.digest("hex") === record.tree_sha256,
    "Unreal packaged-directory tree accounting mismatch");
  return { ...record, inventory: inventoryArtifact, root: inventory.root, excluded: inventory.excluded };
}
function verifyArtifactRecord(owner, record, label, artifactRoot = null) {
  required(record && typeof record.path === "string" && Number.isSafeInteger(record.bytes) && /^[0-9a-f]{64}$/.test(record.sha256), label + " record is malformed");
  const actual = artifact(artifactRoot === null
    ? resolveArtifact(owner, record.path)
    : resolveWithinRoot(artifactRoot, record.path, label));
  required(actual.bytes === record.bytes, label + " byte length mismatch");
  required(actual.sha256 === record.sha256, label + " SHA-256 mismatch");
  return actual;
}
function exactMilestones(values, releaseName = "release_epoch") {
  required(Array.isArray(values) && values.length === REQUIRED.length, "exactly nine nominal milestones are required");
  required(values.every((value, index) => value[releaseName] === REQUIRED[index].release), "nominal milestone ordering/release set mismatch");
}
function sourceMask(sources) {
  const bit = { planned: 1, onboard: 2, ground: 4, truth: 8 };
  return sources.reduce((mask, source) => mask | (bit[source.source] ?? 0), 0);
}
function validateNative(input, sourceCommit) {
  const value = input.value;
  required(value.pass === true, "native harness did not pass");
  required(typeof value.bridge?.source_commit === "string" && value.bridge.source_commit.length >= 12 && sourceCommit.startsWith(value.bridge.source_commit), "native bridge source commit mismatch");
  required(value.bridge?.catalog_identity === CATALOG_SHA256, "native catalog identity mismatch");
  required(value.bridge?.sha256 && value.bridge?.manifest_sha256, "native bridge hashes missing");
  const dll = artifact(resolveArtifact(input.absolute, value.bridge.path));
  const manifest = artifact(resolveArtifact(input.absolute, value.bridge.manifest_path));
  required(dll.sha256 === value.bridge.sha256, "native bridge DLL hash mismatch");
  required(manifest.sha256 === value.bridge.manifest_sha256, "native bridge manifest hash mismatch");
  const bridgeManifest = readJson(manifest.path, "ksa64.viewer-bridge-artifact.v2");
  required(typeof bridgeManifest.value.source_commit === "string" && bridgeManifest.value.source_commit === value.bridge.source_commit && sourceCommit.startsWith(bridgeManifest.value.source_commit), "bridge artifact manifest source commit mismatch");
  required(bridgeManifest.value.sha256 === dll.sha256, "bridge artifact manifest DLL hash mismatch");
  const replay = value.nominal_replay;
  required(replay?.samples === 22015 && replay.first_release === 0 && replay.last_release === 22014 && replay.transitions === 4, "native replay extent mismatch");
  required(replay.director_source_mask === 11 && replay.guided_source_mask === 3, "native role source masks mismatch");
  required(replay.terminal_disposition === 1, "native terminal disposition is not nominal success");
  required(/^[0-9a-f]{64}$/.test(replay.semantic_sha256), "native semantic hash missing");
  const storage = value.display_storage;
  required(storage?.measurement === "serialized GlobalDisplayV1 payload bytes", "native display-storage measurement is missing");
  const storageParts = [storage.definition_bytes, storage.sample_bytes, storage.transition_bytes, storage.replay_index_bytes, storage.path_bytes];
  required(storageParts.every((bytes) => Number.isSafeInteger(bytes) && bytes > 0) &&
    storage.nominal_replay_bytes === storageParts.reduce((sum, bytes) => sum + bytes, 0),
  "native nominal replay display-storage accounting is inconsistent");
  const exactStorage = storage.exact_path_storage;
  required(Number.isSafeInteger(exactStorage?.chunk_count) && exactStorage.chunk_count > 0 &&
    Number.isSafeInteger(exactStorage.point_count) && exactStorage.point_count > 0 &&
    Number.isSafeInteger(exactStorage.serialized_bytes) && exactStorage.serialized_bytes > 0,
  "native exact path-storage accounting is absent or invalid");
  const exactPath = storage.exact_active_window_path;
  required(exactPath?.chunk_count === 1 &&
    Number.isSafeInteger(exactPath.point_count) && exactPath.point_count > 0 && exactPath.point_count <= 4096 &&
    Number.isSafeInteger(exactPath.serialized_bytes) && exactPath.serialized_bytes > 0 &&
    exactPath.serialized_bytes <= exactStorage.serialized_bytes,
  "native exact active-window path accounting is absent or invalid");
  required(Number.isSafeInteger(replay.successful_path_requests) && replay.successful_path_requests >= 9 &&
    Number.isSafeInteger(replay.successful_path_chunk_fetches) && replay.successful_path_chunk_fetches >= replay.successful_path_requests,
  "native path request/chunk accounting is absent or invalid");
  exactMilestones(value.milestones);
  value.milestones.forEach((entry, index) => {
    const expected = REQUIRED[index];
    required(entry.mission_time_q16 === expected.release * 2048, expected.label + ": native mission time mismatch");
    required(entry.frame_identity === expected.frame && entry.segment_identity === expected.segment, expected.label + ": native frame/segment mismatch");
    required((entry.source_mask & 2) !== 0 && (entry.source_mask & ~11) === 0, expected.label + ": native source availability mismatch");
    required(Number.isSafeInteger(entry.event_mask) && Number.isSafeInteger(entry.discontinuity_mask) && entry.continuity_identity > 0, expected.label + ": native event/continuity evidence missing");
    if (TRANSITIONS.has(expected.release)) required(entry.discontinuity_mask !== 0, expected.label + ": native transition is not an exact discontinuity");
  });
  const timing = value.timing;
  required(timing.availability_samples > 0 && timing.range_samples === 86 && timing.path_samples > 0, "native timing sample counts are not measured");
  required(timing.availability_p99_ns >= 0 && timing.range_p99_ns >= 0 && timing.availability_p99_ns < 1_000_000 && timing.range_p99_ns < 1_000_000, "native bridge polling exceeds 1 ms p99");
  return { dll, manifest, semanticSha256: replay.semantic_sha256, milestones: value.milestones, timing, storage };
}
function validateUnreal(input, sourceCommit) {
  const value = input.value;
  required(value.pass === true && value.failure_reason === "", "Unreal producer did not pass cleanly");
  required(value.source_commit === sourceCommit, "Unreal source commit mismatch");
  required(value.scenario === "ksa-g10r.global/nominal" && value.role === "sim-director-read-only", "Unreal scenario/role mismatch");
  required(value.guided_scenario === "ksa-g10r.operations/gnss-loss" && value.guided_role === "guided-operator", "Unreal guided scenario/role mismatch");
  required(value.accepted_exact === true && value.nominal_truth_permitted === true && value.nominal_truth_visible === false && value.guided_truth_permitted === false && value.guided_truth_visible === false, "Unreal exact/truth policy mismatch");
  required(value.nominal_terminal_release_epoch === 22014 && value.nominal_terminal_disposition === 1, "Unreal nominal terminal state mismatch");
  required(value.guided_terminal_release_epoch === 21591 && value.guided_terminal_disposition === 2, "Unreal guided terminal state mismatch");
  const manifestDirectory = dirname(input.absolute);
  const archiveRoot = resolve(manifestDirectory, "..", "..", "..", "..", "..");
  required(relative(archiveRoot, manifestDirectory).split(sep).join("/") === "Windows/Ksa64MissionFoundry/Saved/KSA64/GlobalViewerEvidence",
    "Unreal evidence manifest is not in the required packaged archive location");
  const packagedArtifact = verifyArtifactRecord(input.absolute, value.package, "Unreal legacy packaged-executable record", archiveRoot);
  const executableArtifact = verifyArtifactRecord(input.absolute, value.executable, "Unreal packaged executable", archiveRoot);
  required(executableArtifact.path === packagedArtifact.path && executableArtifact.bytes === packagedArtifact.bytes && executableArtifact.sha256 === packagedArtifact.sha256,
    "Unreal legacy package record does not match the explicit executable record");
  const packagedDirectory = verifyPackageInventory(input.absolute, value.packaged_directory, sourceCommit, archiveRoot);
  required(packagedDirectory.bytes > executableArtifact.bytes,
    "Unreal complete packaged-directory measurement is absent or invalid");
  const renderer = value.renderer;
  required(renderer?.d3d12 === true && /D3D12/i.test(renderer.rhi_name) && renderer.width === 1920 && renderer.height === 1080, "Unreal did not render through D3D12 at 1920x1080");
  required(renderer.packaged_runtime === true && renderer.editor_required === false && renderer.mcp_required === false && renderer.python_required === false, "Unreal evidence is not packaged-runtime independent");
  required(Number(renderer.frames_per_second) >= 60, "Unreal actual measured frame rate is below 60 fps or absent");
  exactMilestones(value.captures);
  const captures = value.captures.map((capture, index) => {
    const expected = REQUIRED[index];
    required(capture.release_epoch === expected.release && capture.frame_identity === expected.frame && capture.segment_identity === expected.segment, expected.label + ": Unreal capture frame/segment mismatch");
    required(capture.source_mask === 11 && capture.transition_markers >= 4, expected.label + ": Unreal source/transition availability mismatch");
    required(capture.planned_path_points > 0 && capture.onboard_path_points > 0 && capture.observed_path_points > 0, expected.label + ": Unreal path evidence missing");
    required(capture.width === 1920 && capture.height === 1080 && capture.sampled_pixels > 0 && capture.distinct_color_buckets >= 8 && capture.luminance_range >= 24 && capture.non_dark_samples > 0, expected.label + ": Unreal screenshot measurement failed");
    const semantic = readJson(resolveArtifact(input.absolute, capture.semantic_file), "ksa64.unreal-global-viewer-semantic.v1");
    const screenshot = artifact(resolveArtifact(input.absolute, capture.screenshot_file));
    const state = semantic.value;
    required(state.release_epoch === expected.release && state.replay_selected_release === expected.release, expected.label + ": Unreal selected release mismatch");
    required(state.mission_time_q16 === expected.release * 2048 && state.frame_identity === expected.frame && state.segment_identity === expected.segment, expected.label + ": Unreal semantic frame/time mismatch");
    required(state.source_mask === 11 && state.acceptance_eligible === true && state.scene_ready === true && state.exact_snap === true, expected.label + ": Unreal semantic acceptance state mismatch");
    required(state.truth_permitted === true && state.truth_visible === false, expected.label + ": Unreal truth visibility mismatch");
    required(state.overall_disposition === 1 && state.evidence_disposition === 1, expected.label + ": Unreal disposition mismatch");
    required(state.planned_path_points > 0 && state.onboard_path_points > 0 && state.observed_path_points > 0 && state.transition_markers >= 4, expected.label + ": Unreal semantic path evidence missing");
    return { expected, capture, semantic, screenshot };
  });
  const rendererOrigin = value.renderer_origin;
  required(Number.isSafeInteger(rendererOrigin?.change_count) && rendererOrigin.change_count > 0 &&
    rendererOrigin.continuity_checks === 1 && rendererOrigin.semantic_unchanged === true &&
    Number.isSafeInteger(rendererOrigin.rendered_sample_count) && rendererOrigin.rendered_sample_count >= 8 &&
    Number(rendererOrigin.max_reconstructed_delta_cm) >= 0 && Number(rendererOrigin.max_reconstructed_delta_cm) <= 100 &&
    rendererOrigin.rendered_continuity === true && rendererOrigin.semantic_continuity === true,
    "Unreal renderer-origin semantic/rendered continuity evidence is absent or invalid");
  const performance = value.performance;
  required(performance?.pass === true && performance.measured_frames >= 600 && performance.percentile_method === "nearest-rank", "Unreal measured performance record missing");
  required(performance.p99_ns >= 0 && performance.p99_ns < 1_000_000 && performance.max_ns >= 0 && performance.max_ns < 2_000_000, "Unreal display publication exceeds limit");
  return { renderer, captures, performance, packagedArtifact, executableArtifact, packagedDirectory, rendererOrigin };
}
function validateBrowser(input, sourceCommit) {
  const value = input.value;
  required(value.pass === true, "browser producer did not pass");
  required(value.source?.commit === sourceCommit && value.source.dirty === false && /^[0-9a-f]{64}$/.test(value.source.tree_sha256) && value.source.file_count > 0, "browser source identity mismatch or dirty");
  const productionDistRecord = value.production_dist;
  required(productionDistRecord?.schema === "ksa64.phase12c.web-distribution-identity.v1" &&
    productionDistRecord.measurement === "production web/dist payload excluding its identity record" &&
    canonical(productionDistRecord.excluded) === canonical(["phase12c-dist-identity.json"]) &&
    productionDistRecord?.path === "web/dist" && Number.isSafeInteger(productionDistRecord.bytes) && productionDistRecord.bytes > 0 &&
    Number.isSafeInteger(productionDistRecord.file_count) && productionDistRecord.file_count > 1 && /^[0-9a-f]{64}$/.test(productionDistRecord.tree_sha256),
    "browser production distribution record is missing");
  const productionDist = directoryArtifact(resolve(REPOSITORY_ROOT, productionDistRecord.path), new Set(productionDistRecord.excluded));
  required(productionDist.bytes === productionDistRecord.bytes && productionDist.file_count === productionDistRecord.file_count &&
    productionDist.tree_sha256 === productionDistRecord.tree_sha256, "browser production distribution accounting mismatch");
  const authorityWasm = value.source?.authority_wasm;
  required(authorityWasm?.path === "web/public/wasm/ksa64-session.wasm", "browser authority WASM identity missing");
  const wasmArtifact = artifact(resolve(REPOSITORY_ROOT, authorityWasm.path));
  required(wasmArtifact.bytes === authorityWasm.bytes && wasmArtifact.sha256 === authorityWasm.sha256, "browser authority WASM hash mismatch");
  const nominalRaw = verifyArtifactRecord(input.absolute, value.producer?.nominal_raw, "browser nominal raw");
  const guidedRaw = verifyArtifactRecord(input.absolute, value.producer?.guided_raw, "browser guided raw");
  const screenshots = value.producer?.screenshots;
  required(Array.isArray(screenshots) && screenshots.length >= 3, "browser completion evidence requires at least three rendered screenshots");
  const screenshotArtifacts = screenshots.map((entry, index) => verifyArtifactRecord(input.absolute, entry, "browser screenshot " + index));
  const nominal = readJson(nominalRaw.path, "ksa64.phase12c.browser-producer.v1");
  const guided = readJson(guidedRaw.path, "ksa64.phase12c.browser-role-producer.v1");
  required(nominal.value.experience === "nominal-global" && guided.value.experience === "gnss-loss", "browser producer experiences mismatch");
  for (const producer of [nominal.value, guided.value]) {
    required(producer.build_identity?.commit === sourceCommit && producer.build_identity?.dirty === false && producer.build_identity?.tree_sha256 === value.source.tree_sha256, "browser raw producer build identity mismatch");
    required(canonical(producer.distribution_identity) === canonical(productionDistRecord),
      "browser raw producer was not served from the captured production distribution");
  }
  required(nominal.value.environment?.production_build === true && guided.value.environment?.production_build === true,
    "browser evidence was not captured from the production Vite build");
  for (const key of ["webgpu", "webgl2", "two_d"]) {
    required(value.backends?.[key]?.status === "rendered", "browser " + key + " lane was not actually rendered");
    required(Number(value.backends[key].frames_per_second) > 0, "browser " + key + " has no measured frame rate");
  }
  required(Number(value.backends.webgpu.frames_per_second) >= 30 && Number(value.backends.webgl2.frames_per_second) >= 30, "Babylon WebGPU/WebGL2 fell below 30 fps");
  required(value.context_loss?.before_backend === "webgl2" && value.context_loss?.after_backend === "2d" && value.context_loss.before_semantic_sha256 === value.context_loss.after_semantic_sha256, "browser context-loss semantic fallback failed");
  required(canonical(value.renderer_origin) === canonical(nominal.value.renderer_origin),
    "browser summary renderer-origin evidence differs from its bound raw producer");
  const rendererOrigin = nominal.value.renderer_origin;
  required(rendererOrigin?.change_count === 1 && rendererOrigin.semantic_unchanged === true &&
    rendererOrigin.rendered_continuity === true && rendererOrigin.semantic_continuity === true &&
    Number.isSafeInteger(rendererOrigin.rendered_sample_count) && rendererOrigin.rendered_sample_count >= 8 &&
    Number(rendererOrigin.max_reconstructed_delta_km) >= 0 && Number(rendererOrigin.max_reconstructed_delta_km) <= 0.001 &&
    /^[0-9a-f]{64}$/.test(rendererOrigin.absolute_semantic_sha256) &&
    /^[0-9a-f]{64}$/.test(rendererOrigin.rendered_absolute_sha256) &&
    canonical(rendererOrigin.before_origin_km) !== canonical(rendererOrigin.after_origin_km),
    "browser renderer-origin semantic/rendered continuity evidence is absent or invalid");
  required(value.role_filtering?.sim_director?.truth_default_hidden === true && value.role_filtering?.sim_director?.truth_opt_in_labeled === true, "browser SIM Director truth policy failed");
  required(value.role_filtering?.guided_operator?.truth_control_absent === true && value.role_filtering?.guided_operator?.truth_source_absent === true, "browser Guided Operator received truth controls/data");
  exactMilestones(value.semantic_milestones);
  exactMilestones(nominal.value.milestones);
  const milestones = nominal.value.milestones.map((record, index) => {
    const expected = REQUIRED[index];
    const semantic = record.semantic;
    required(record.release_epoch === expected.release && semantic?.schema === "ksa64.global-scene-semantic.v1", expected.label + ": browser semantic missing");
    required(semantic.releaseEpoch === expected.release && semantic.missionTimeQ16 === expected.release * 2048, expected.label + ": browser selected release/time mismatch");
    required(semantic.frame === expected.frameName && semantic.segment === expected.segmentName, expected.label + ": browser frame/segment mismatch");
    required(semantic.quality === "global-display-v1" && semantic.exactSnapRequired === true && semantic.truthLabelVisible === false, expected.label + ": browser exact/truth state mismatch");
    const visibleMask = sourceMask(semantic.sources ?? []);
    required((visibleMask & 2) !== 0 && (visibleMask & 8) === 0, expected.label + ": browser visible source policy mismatch");
    const pathSources = new Set((semantic.paths ?? []).map((path) => path.source));
    required(pathSources.has("planned") && pathSources.has("onboard"), expected.label + ": browser planned/onboard paths missing");
    for (const path of semantic.paths) required(path.pointCount > 0 && Number.isSafeInteger(path.pointChecksum) && path.pointChecksum > 0, expected.label + ": browser path checksum missing");
    const authority = record.authority_state;
    required(authority?.overall === "Nominal success", expected.label + ": browser overall disposition mismatch");
    required(Array.isArray(authority.axes) && authority.axes.length === 6 && authority.axes.every((axis) => axis.label && axis.value && axis.state === "success"), expected.label + ": browser disposition axes mismatch");
    required(/^[0-9a-f]{64}$/.test(record.semantic_sha256) && /^[0-9a-f]{64}$/.test(record.authority_sha256), expected.label + ": browser semantic/authority hash missing");
    required(record.semantic_sha256 === sha256(Buffer.from(canonical(semantic))), expected.label + ": browser embedded semantic hash mismatch");
    required(record.authority_sha256 === sha256(Buffer.from(canonical(authority))), expected.label + ": browser embedded authority hash mismatch");
    return { expected, record, visibleMask };
  });
  return { authorityWasm: wasmArtifact, productionDist, nominalRaw, guidedRaw, guidedValue: guided.value, screenshots: screenshotArtifacts, milestones, backends: value.backends, contextLoss: value.context_loss, rendererOrigin };
}
function optionalOperationalParity(unreal, browser) {
  const unrealEvents = unreal.value.operational_milestones;
  const browserEvents = browser.value.operational_milestones;
  if (unrealEvents === undefined && browserEvents === undefined) return { status: "not-present-in-nominal-evidence", count: 0, milestones: [] };
  required(Array.isArray(unrealEvents) && Array.isArray(browserEvents), "operational action/fault evidence must be present in both renderers");
  required(unrealEvents.length === browserEvents.length && unrealEvents.length === OPERATIONAL_RELEASES.length, "operational action/fault count mismatch");
  required(unrealEvents.every((entry, index) => entry.release_epoch === OPERATIONAL_RELEASES[index]) && browserEvents.every((entry, index) => entry.release_epoch === OPERATIONAL_RELEASES[index]), "operational milestone release set mismatch");
  const labels = { "Nominal success": 1, "Degraded success": 2, "Contingency success": 3, "Mission failure": 4, Indeterminate: 5 };
  const milestones = [];
  for (let index = 0; index < unrealEvents.length; index += 1) {
    const unrealEntry = unrealEvents[index]; const browserEntry = browserEvents[index];
    const unrealKind = String(unrealEntry.kind ?? ""); const browserKind = String(browserEntry.kind ?? "");
    const unrealClass = /fault/iu.test(unrealKind) ? "fault" : /stage|commit|action|branch|update/iu.test(unrealKind) ? "action" : "unknown";
    const browserClass = /fault/iu.test(browserKind) ? "fault" : /action|stage|commit/iu.test(browserKind) ? "action" : "unknown";
    required(unrealEntry.release_epoch === browserEntry.release_epoch && unrealClass === browserClass && unrealClass !== "unknown", "operational action/fault milestone mismatch");
    const unrealSemantic = unrealEntry.semantic ?? (unrealEntry.semantic_file ? readJson(resolveArtifact(unreal.absolute, unrealEntry.semantic_file), "ksa64.unreal-global-viewer-semantic.v1").value : unrealEntry);
    const browserSemantic = browserEntry.semantic;
    required(unrealSemantic.release_epoch === unrealEntry.release_epoch && unrealSemantic.replay_selected_release === unrealEntry.release_epoch, "Unreal operational selected release mismatch");
    required(unrealSemantic.frame_identity === 3 && unrealSemantic.segment_identity === 3 && unrealSemantic.source_mask === 3 && unrealSemantic.truth_visible === false, "Unreal guided operational frame/source/truth mismatch");
    required(browserSemantic?.schema === "ksa64.global-scene-semantic.v1" && browserSemantic.releaseEpoch === browserEntry.release_epoch && browserSemantic.frame === "gcrf" && browserSemantic.segment === "eci-coast" && browserSemantic.truthLabelVisible === false, "browser guided operational frame/source/truth mismatch");
    const visibleMask = sourceMask(browserSemantic.sources ?? []);
    required((visibleMask & 2) !== 0 && (visibleMask & 8) === 0, "browser guided operational source mismatch");
    const browserDisposition = labels[browserEntry.authority_state?.overall];
    required(browserDisposition !== undefined && unrealSemantic.overall_disposition === browserDisposition, "guided operational disposition mismatch");
    milestones.push({ release_epoch: unrealEntry.release_epoch, kind: unrealClass, unreal_label: unrealKind, browser_label: browserKind, frame_identity: 3, frame: "gcrf", segment_identity: 3, segment: "eci-coast", source_availability_mask: 3, visible_source_mask: visibleMask, overall_disposition: browserDisposition, truth_visible: false, unreal_semantic_sha256: sha256(Buffer.from(canonical(unrealSemantic))), browser_semantic_sha256: sha256(Buffer.from(canonical(browserSemantic))) });
  }
  return { status: "compared", count: unrealEvents.length, milestones };
}
function main() {
  const args = parseArgs(process.argv.slice(2));
  const sourceCommit = args.get("--source-commit").toLowerCase();
  required(/^[0-9a-f]{40}$/.test(sourceCommit), "source commit must be a full 40-digit SHA-1");
  const nativeInput = readJson(args.get("--native"), "ksa64.phase12c.global-display-harness.v1");
  const unrealInput = readJson(args.get("--unreal"), "ksa64.phase12c.unreal-global-evidence.v1");
  const browserInput = readJson(args.get("--browser"), "ksa64.phase12c.browser-evidence.v1");
  const native = validateNative(nativeInput, sourceCommit);
  const unreal = validateUnreal(unrealInput, sourceCommit);
  const browser = validateBrowser(browserInput, sourceCommit);
  const operational = optionalOperationalParity(unrealInput, { value: browser.guidedValue });
  const milestones = REQUIRED.map((expected, index) => {
    const n = native.milestones[index]; const u = unreal.captures[index]; const b = browser.milestones[index];
    required(n.release_epoch === u.capture.release_epoch && n.release_epoch === b.record.release_epoch, expected.label + ": selected release parity failed");
    required(n.frame_identity === u.capture.frame_identity && n.frame_identity === expected.frame, expected.label + ": frame parity failed");
    required(n.segment_identity === u.capture.segment_identity && n.segment_identity === expected.segment, expected.label + ": segment parity failed");
    return {
      label: expected.label, release_epoch: expected.release, mission_time_q16: expected.release * 2048,
      frame_identity: expected.frame, frame: expected.frameName, segment_identity: expected.segment, segment: expected.segmentName,
      native: { source_mask: n.source_mask, event_mask: n.event_mask, discontinuity_mask: n.discontinuity_mask, continuity_identity: n.continuity_identity },
      unreal: { source_mask: u.capture.source_mask, semantic_sha256: u.semantic.sha256, screenshot_sha256: u.screenshot.sha256,
        planned_path_points: u.capture.planned_path_points, onboard_path_points: u.capture.onboard_path_points, observed_path_points: u.capture.observed_path_points },
      browser: { visible_source_mask: b.visibleMask, semantic_sha256: b.record.semantic_sha256, authority_sha256: b.record.authority_sha256,
        path_count: b.record.semantic.paths.length, path_point_count: b.record.semantic.paths.reduce((sum, path) => sum + path.pointCount, 0),
        path_checksums: b.record.semantic.paths.map((path) => path.pointChecksum) },
      terminal_disposition: 1, truth_visible: false,
    };
  });
  const output = {
    schema: "ksa64.phase12c.cross-renderer-evidence.v2", pass: true,
    producer: { kind: "strict-source-bound-parity-comparator", script: artifact(SCRIPT_PATH), source_commit: sourceCommit },
    inputs: {
      native: { ...artifact(nativeInput.absolute), bridge_dll: native.dll, bridge_manifest: native.manifest },
      unreal: { ...artifact(unrealInput.absolute), executable: unreal.executableArtifact, packaged_directory: unreal.packagedDirectory, package: unreal.packagedArtifact },
      browser: { ...artifact(browserInput.absolute), authority_wasm: browser.authorityWasm, production_dist: browser.productionDist, nominal_raw: browser.nominalRaw, guided_raw: browser.guidedRaw, screenshots: browser.screenshots },
    },
    catalog_sha256: CATALOG_SHA256,
    nominal: { releases: 22015, first_release: 0, last_release: 22014, transition_count: 4, terminal_disposition: 1,
      source_availability: { sim_director: 11, guided_operator: 3 }, milestones },
    operational_milestones: operational,
    storage: {
      nominal_replay_display: native.storage,
      unreal: { executable_bytes: unreal.executableArtifact.bytes, packaged_directory_bytes: unreal.packagedDirectory.bytes, packaged_directory_files: unreal.packagedDirectory.file_count },
      web: { production_dist_bytes: browser.productionDist.bytes, production_dist_files: browser.productionDist.file_count, production_dist_sha256: browser.productionDist.tree_sha256 },
    },
    renderer_origins: {
      unreal: unreal.rendererOrigin,
      browser: browser.rendererOrigin,
      semantic_continuity: unreal.rendererOrigin.semantic_continuity === true && browser.rendererOrigin.semantic_continuity === true,
    },
    performance: {
      unreal: { resolution: "1920x1080", frames_per_second: Number(unreal.renderer.frames_per_second), display_publication_p99_ns: unreal.performance.p99_ns, display_publication_max_ns: unreal.performance.max_ns },
      bridge: { availability_p99_ns: native.timing.availability_p99_ns, range_p99_ns: native.timing.range_p99_ns },
      babylon: { webgpu_frames_per_second: Number(browser.backends.webgpu.frames_per_second), webgl2_frames_per_second: Number(browser.backends.webgl2.frames_per_second), two_d_frames_per_second: Number(browser.backends.two_d.frames_per_second), context_loss_fallback: true },
    },
  };
  writeFileSync(resolve(args.get("--output")), canonical(output) + "\n", "utf8");
  console.log("Phase 12C strict cross-renderer parity passed for " + REQUIRED.length + " exact milestones");
}
main();
