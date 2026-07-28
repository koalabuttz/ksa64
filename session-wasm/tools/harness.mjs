import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';

const wasmPath = process.argv[2];
if (!wasmPath) {
  throw new Error('usage: node session-wasm/tools/harness.mjs <ksa64_session_wasm.wasm>');
}

const bytes = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes, {});
const { memory, ksa64_wasm_alloc: alloc, ksa64_wasm_dealloc: dealloc,
  ksa64_wasm_submit: submit, ksa64_wasm_open_replay: openReplayExport,
  ksa64_wasm_result_ptr: resultPtr, ksa64_wasm_result_len: resultLen } = instance.exports;

const text = new TextDecoder();
const commandLength = 32;
const expectedSha = '7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4';

function command(kind, values = []) {
  const input = new Uint8Array(commandLength);
  input.set([0x4b, 0x53, 0x57, 0x31]);
  const data = new DataView(input.buffer);
  data.setUint16(4, 1, true);
  data.setUint16(6, kind, true);
  for (let index = 0; index < 5; index += 1) data.setUint32(8 + index * 4, values[index] ?? 0, true);
  return input;
}

function workerResult(expectedStatus = 0) {
  const result = new Uint8Array(memory.buffer, resultPtr(), resultLen()).slice();
  if (text.decode(result.subarray(0, 4)) !== 'KSR1') throw new Error('missing KSR1 result');
  const header = new DataView(result.buffer, result.byteOffset, result.byteLength);
  const code = header.getInt16(6, true);
  const payloadLength = header.getUint32(8, true);
  if (code !== expectedStatus || payloadLength !== result.length - 12) {
    throw new Error('worker result status mismatch: ' + code + ' expected ' + expectedStatus);
  }
  return result.subarray(12);
}

function run(input) {
  const pointer = alloc(input.length);
  new Uint8Array(memory.buffer, pointer, input.length).set(input);
  const status = submit(pointer, input.length);
  dealloc(pointer, input.length);
  if (status !== 0) throw new Error('wasm submit failed: ' + status);
  return workerResult();
}

function openReplay(archive, role, nonce, expectedStatus = 0) {
  const pointer = alloc(archive.length);
  new Uint8Array(memory.buffer, pointer, archive.length).set(archive);
  const status = openReplayExport(
    pointer, archive.length, role,
    Number(nonce & 0xffff_ffffn), Number(nonce >> 32n),
  );
  dealloc(pointer, archive.length);
  if (status !== 0) throw new Error('wasm replay entrypoint failed: ' + status);
  return workerResult(expectedStatus);
}

// Drive the real worker command and KPS1 publication surfaces at each accepted
// operational decision epoch. Review remains presentation-only; Stage and Commit
// are the four canonical actions sealed into KSB11.
const catalog = JSON.parse(text.decode(run(command(1))));
if (catalog.experiences?.[0]?.id !== 'ksa-g10r.operations') throw new Error('catalog identity mismatch');
run(command(2, [2, 0x3456789a, 0x12345678])); // Guided Operator + deterministic nonce
run(command(3));
run(command(4, [1])); // Fast

function publicationHasPayload(bundle, magic) {
  if (text.decode(bundle.subarray(0, 4)) !== 'KPW1') throw new Error('missing KPW1 publication bundle');
  const view = new DataView(bundle.buffer, bundle.byteOffset, bundle.byteLength);
  const count = view.getUint16(6, true);
  let at = 8;
  for (let index = 0; index < count; index += 1) {
    if (at + 4 > bundle.length) throw new Error('truncated KPW1 record length');
    const length = view.getUint32(at, true); at += 4;
    if (length < 48 || at + length > bundle.length) throw new Error('invalid KPW1 record length');
    if (text.decode(bundle.subarray(at + 48, at + 52)) === magic) return true;
    at += length;
  }
  if (at !== bundle.length) throw new Error('trailing KPW1 bytes');
  return false;
}

function advance(releases) { const value = run(command(5, [releases])); if (new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true) !== releases) throw new Error('authority did not advance exact requested releases'); }
function proposal() { const bundle = run(command(6)); if (!publicationHasPayload(bundle, 'PAP1')) throw new Error('decision epoch did not publish an action proposal'); }
function action(operation, clientSequence) { const bundle = run(command(7, [operation, 0, 0, 0, clientSequence])); if (!publicationHasPayload(bundle, 'PAR1')) throw new Error('action did not return a receipt'); }

advance(6080); proposal(); action(1, 1); action(2, 2);
advance(160); proposal(); action(3, 3);
advance(320); proposal(); action(1, 4); action(2, 5);
advance(160); proposal(); action(3, 6);
advance(21591 - 6720);

const terminal = run(command(6));
if (!publicationHasPayload(terminal, 'PEM1')) throw new Error('completed session did not publish sealed-evidence metadata');
const evidence = run(command(8));
if (evidence.length !== 2911464) throw new Error('KSB11 length mismatch: ' + evidence.length);
if (text.decode(evidence.subarray(0, 4)) !== 'KSB1') throw new Error('KSB11 magic mismatch');
const summary = run(command(11));
const summaryView = new DataView(summary.buffer, summary.byteOffset, summary.byteLength);
if (summaryView.getUint32(0, true) !== 21591) throw new Error('release epoch mismatch');
if (summaryView.getUint32(4, true) !== 4) throw new Error('accepted action count mismatch');
if (summaryView.getUint32(8, true) !== 2911464) throw new Error('summary evidence length mismatch');
const sha = createHash('sha256').update(evidence).digest('hex');
if (sha !== expectedSha) throw new Error('KSB11 SHA-256 mismatch: ' + sha);

// The browser replay lane treats KSB11 as opaque: only Rust validates and
// deterministically re-executes it, then JavaScript receives role-filtered KPS1.
run(command(10)); // Destroy live authority before opening read-only replay.
const corrupt = evidence.slice();
corrupt[32] ^= 1;
openReplay(corrupt, 1, 0x123456789abcdef0n, -13);

const replayInfo = openReplay(evidence, 1, 0x123456789abcdef0n);
if (text.decode(replayInfo.subarray(0, 4)) !== 'KPRI' || replayInfo.length !== 72) {
  throw new Error('missing strict replay metadata');
}
const replayInfoView = new DataView(replayInfo.buffer, replayInfo.byteOffset, replayInfo.byteLength);
const replayFrames = replayInfoView.getBigUint64(8, true);
if (replayFrames < 21592n || replayInfo[28] !== 1 ||
    replayInfoView.getBigUint64(32, true) !== 0x123456789abcdef0n) {
  throw new Error('invalid role-bound replay metadata');
}

function publicationFrames(bundle) {
  if (text.decode(bundle.subarray(0, 4)) !== 'KPW1') throw new Error('missing replay KPW1 bundle');
  const view = new DataView(bundle.buffer, bundle.byteOffset, bundle.byteLength);
  const count = view.getUint16(6, true);
  const frames = [];
  let at = 8;
  for (let index = 0; index < count; index += 1) {
    const length = view.getUint32(at, true); at += 4;
    if (length < 48 || at + length > bundle.length) throw new Error('truncated replay frame');
    frames.push(bundle.subarray(at, at + length)); at += length;
  }
  if (at !== bundle.length) throw new Error('trailing replay bundle bytes');
  return frames;
}

let replayCursor = 0n;
let replaySnapshots = 0;
let replayFinal = false;
while (replayCursor < replayFrames) {
  const bundle = run(command(13, [
    Number(replayCursor & 0xffff_ffffn), Number(replayCursor >> 32n), 256, 1024 * 1024,
  ]));
  const frames = publicationFrames(bundle);
  if (frames.length === 0) throw new Error('premature replay EOF');
  for (const frame of frames) {
    const view = new DataView(frame.buffer, frame.byteOffset, frame.byteLength);
    if (text.decode(frame.subarray(0, 4)) !== 'KPS1' ||
        view.getBigUint64(16, true) !== 0x123456789abcdef0n ||
        view.getBigUint64(24, true) !== replayCursor + 1n) {
      throw new Error('invalid replay KPS1 identity or sequence');
    }
    replayCursor += 1n;
    const payload = frame.subarray(48);
    const payloadMagic = text.decode(payload.subarray(0, 4));
    if (payloadMagic === 'POS1') {
      replaySnapshots += 1;
      if (payload[36] !== 1 || payload[41] !== 0 || (view.getBigUint64(48 + 28, true) & (1n << 63n)) !== 0n) {
        throw new Error('observer replay exposed private truth or wrong role');
      }
    }
    replayFinal = payloadMagic === 'PEM1' && (view.getUint32(12, true) & 2) !== 0;
  }
}
if (replaySnapshots < 21592 || !replayFinal) throw new Error('replay omitted lifecycle or final evidence metadata');
const eof = publicationFrames(run(command(13, [Number(replayCursor), 0, 1, 64])));
if (eof.length !== 0) throw new Error('replay EOF was not stable');
console.log(JSON.stringify({ wasm: wasmPath, path: 'worker-kps1-actions-and-opaque-replay', releaseEpoch: 21591, acceptedActions: 4, bytes: evidence.length, sha256: sha, replayFrames: replayCursor.toString(), replaySnapshots }));
