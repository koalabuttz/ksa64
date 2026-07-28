import { writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalJson, computePhase12cWebSourceIdentity } from "./phase12c-source-identity.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(here, "../..");
const output = resolve(repositoryRoot, "web/public/phase12c-build-identity.json");
const source = computePhase12cWebSourceIdentity(repositoryRoot);
writeFileSync(output, `${canonicalJson({
  ...source,
  generated_by: "web/scripts/write-phase12c-build-identity.mjs",
})}\n`, "utf8");
console.log(`wrote ${output}`);
