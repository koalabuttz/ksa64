import { createHash } from "node:crypto";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const excluded = new Set(["runtime-config.js", "precache-manifest.js"]);

async function collect(directory, output = []) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const absolute = join(directory, entry.name);
    if (entry.isDirectory()) await collect(absolute, output);
    else if (entry.isFile()) output.push(absolute);
  }
  return output;
}

const files = (await collect(dist))
  .map((absolute) => relative(dist, absolute).split(sep).join("/"))
  .filter((path) => !excluded.has(path))
  .sort();
const routes = ["/", ...files.map((path) => "/" + path)];
const fingerprint = createHash("sha256");
for (const path of files) {
  const absolute = join(dist, path);
  const metadata = await stat(absolute);
  fingerprint.update(path + "\0" + metadata.size + "\0");
  fingerprint.update(await readFile(absolute));
}
const version = fingerprint.digest("hex").slice(0, 20);
const source = [
  "// Generated after the production build; do not edit by hand.",
  "self.__KSA64_PRECACHE_VERSION__ = " + JSON.stringify(version) + ";",
  "self.__KSA64_PRECACHE__ = " + JSON.stringify(routes) + ";",
  "",
].join("\n");
await writeFile(join(dist, "precache-manifest.js"), source, "utf8");
console.log(JSON.stringify({ version, assets: routes.length, excluded: [...excluded] }));
