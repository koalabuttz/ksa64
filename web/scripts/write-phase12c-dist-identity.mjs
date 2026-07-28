import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const identityFile = "phase12c-dist-identity.json";
const excluded = new Set([identityFile]);

async function collect(directory, output = []) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name < right.name ? -1 : left.name > right.name ? 1 : 0);
  for (const entry of entries) {
    const absolute = join(directory, entry.name);
    if (entry.isDirectory()) await collect(absolute, output);
    else if (entry.isFile()) output.push(absolute);
  }
  return output;
}

async function record() {
  const files = (await collect(dist))
    .map((absolute) => ({ absolute, path: relative(dist, absolute).split(sep).join("/") }))
    .filter((entry) => !excluded.has(entry.path))
    .sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
  const aggregate = createHash("sha256");
  let bytes = 0;
  for (const entry of files) {
    const content = await readFile(entry.absolute);
    bytes += content.byteLength;
    aggregate.update(entry.path + "\0" + content.byteLength + "\0");
    aggregate.update(content);
  }
  return {
    schema: "ksa64.phase12c.web-distribution-identity.v1",
    measurement: "production web/dist payload excluding its identity record",
    path: "web/dist",
    excluded: [...excluded],
    bytes,
    file_count: files.length,
    tree_sha256: aggregate.digest("hex"),
  };
}

const identity = await record();
await writeFile(join(dist, identityFile), JSON.stringify(identity, null, 2) + "\n", "utf8");
const verified = await record();
if (JSON.stringify(identity) !== JSON.stringify(verified)) {
  throw new Error("Phase 12C production distribution identity is not self-consistent");
}
console.log(JSON.stringify(identity));
