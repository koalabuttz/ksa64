import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const webDirectory = path.resolve(scriptDirectory, "..");
const workspace = path.resolve(webDirectory, "..");
const source = path.join(workspace, "target", "wasm32-unknown-unknown", "release", "ksa64_session_wasm.wasm");
const destinationDirectory = path.join(webDirectory, "public", "wasm");
const destination = path.join(destinationDirectory, "ksa64-session.wasm");

const cargo = process.platform === "win32" ? "cargo.exe" : "cargo";
const result = spawnSync(cargo, ["build", "--locked", "-p", "ksa64-session-wasm", "--target", "wasm32-unknown-unknown", "--release"], {
  cwd: workspace,
  stdio: "inherit",
});
if (result.status !== 0) process.exit(result.status ?? 1);
await mkdir(destinationDirectory, { recursive: true });
await copyFile(source, destination);
const bytes = await (await import("node:fs/promises")).readFile(destination);
const sha256 = createHash("sha256").update(bytes).digest("hex");
await writeFile(path.join(destinationDirectory, "ksa64-session.wasm.sha256"), sha256 + "\n");
console.log(JSON.stringify({ source, destination, bytes: bytes.length, sha256 }));
