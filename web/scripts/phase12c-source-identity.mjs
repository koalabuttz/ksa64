import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

const INCLUDED_ROOT_FILES = new Set([
  "index.html",
  "package-lock.json",
  "package.json",
  "tsconfig.app.json",
  "tsconfig.json",
  "tsconfig.node.json",
  "vite.config.ts",
]);

const INCLUDED_EXTENSIONS = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".mjs",
  ".svg",
  ".ts",
  ".tsx",
  ".wasm",
  ".webmanifest",
]);

const EXCLUDED_NAMES = new Set([
  "dist",
  "node_modules",
  "phase12c-build-identity.json",
  "precache-manifest.js",
]);

function extension(path) {
  const index = path.lastIndexOf(".");
  return index < 0 ? "" : path.slice(index);
}

function collect(root, current = root, output = []) {
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    if (EXCLUDED_NAMES.has(entry.name)) continue;
    const absolute = resolve(current, entry.name);
    if (entry.isDirectory()) {
      collect(root, absolute, output);
      continue;
    }
    const relativePath = relative(root, absolute).split(sep).join("/");
    const topLevel = !relativePath.includes("/");
    const acceptedDirectory = relativePath.startsWith("src/") ||
      relativePath.startsWith("scripts/") ||
      relativePath.startsWith("public/");
    if ((topLevel && INCLUDED_ROOT_FILES.has(relativePath)) ||
        (acceptedDirectory && INCLUDED_EXTENSIONS.has(extension(relativePath)))) {
      output.push({ absolute, relativePath });
    }
  }
  return output;
}

function git(repoRoot, args, fallback) {
  try {
    return execFileSync("git", ["-C", repoRoot, ...args], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return fallback;
  }
}

export function computePhase12cWebSourceIdentity(repositoryRoot) {
  const root = resolve(repositoryRoot);
  const webRoot = resolve(root, "web");
  if (!statSync(webRoot).isDirectory()) throw new Error(`web source directory is missing: ${webRoot}`);
  const files = collect(webRoot).sort((left, right) =>
    left.relativePath.localeCompare(right.relativePath, "en"));
  const aggregate = createHash("sha256");
  const fileRecords = files.map(({ absolute, relativePath }) => {
    const bytes = readFileSync(absolute);
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    aggregate.update(Buffer.from(relativePath, "utf8"));
    aggregate.update(Buffer.from([0]));
    aggregate.update(Buffer.from(sha256, "ascii"));
    aggregate.update(Buffer.from([0]));
    return { path: `web/${relativePath}`, bytes: bytes.byteLength, sha256 };
  });
  const commit = git(root, ["rev-parse", "HEAD"], "unavailable");
  const status = git(root, ["status", "--porcelain=v1"], "unavailable");
  return {
    schema: "ksa64.phase12c.web-source-identity.v1",
    commit,
    dirty: status === "unavailable" || status.length > 0,
    tree_sha256: aggregate.digest("hex"),
    file_count: fileRecords.length,
    files: fileRecords,
  };
}

export function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) =>
      `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}
