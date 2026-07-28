import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const context = { self: {} };
vm.runInNewContext(await readFile(join(dist, "precache-manifest.js"), "utf8"), context, { timeout: 1000 });
const routes = context.self.__KSA64_PRECACHE__;
const version = context.self.__KSA64_PRECACHE_VERSION__;
if (!Array.isArray(routes) || typeof version !== "string" || !/^[0-9a-f]{20}$/u.test(version)) throw new Error("invalid precache manifest");
if (new Set(routes).size !== routes.length || !routes.includes("/") || !routes.includes("/index.html") || !routes.includes("/sw.js") || !routes.includes("/wasm/ksa64-session.wasm")) throw new Error("precache manifest is incomplete or duplicated");
if (routes.includes("/runtime-config.js") || routes.includes("/precache-manifest.js")) throw new Error("ephemeral broker configuration entered the cache manifest");
async function collect(directory, output = []) { for (const entry of await readdir(directory, { withFileTypes: true })) { const absolute=join(directory,entry.name); if(entry.isDirectory()) await collect(absolute,output); else if(entry.isFile()) output.push(relative(dist,absolute).split(sep).join("/")); } return output; }
const expected=(await collect(dist)).filter((path)=>path!=="runtime-config.js"&&path!=="precache-manifest.js");
for(const path of expected) if(!routes.includes("/"+path)) throw new Error("missing precache asset: "+path);
const sw=await readFile(join(dist,"sw.js"),"utf8");
if(!sw.includes('importScripts("/precache-manifest.js")')||!sw.includes('url.pathname === "/runtime-config.js"')) throw new Error("service worker does not enforce generated precache and credential bypass");
console.log(JSON.stringify({ version, assets: routes.length, verifiedFiles: expected.length }));
