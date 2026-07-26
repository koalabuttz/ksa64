#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const input = path.resolve(process.argv[2] || 'target/mos-c64-none/c64/ksa64-phase11-reference-ops-endpoint-c64');
const outputDir = path.resolve(process.argv[3] || 'target/phase11-c64-banked');
const bundle = fs.readFileSync(input);
const fail = message => { throw new Error(message); };
if (bundle.length < 20 || bundle.subarray(0, 4).toString('ascii') !== 'KSB1') fail('invalid KSB1 bundle');
if (bundle.readUInt16LE(4) !== 1) fail('unsupported KSB1 version');
const entry = bundle.readUInt16LE(6);
const expected = [
  { name: 'extra', origin: 0x053f, capacity: 0x02c2 },
  { name: 'main', origin: 0x0801, capacity: 0xb7ff },
  { name: 'high', origin: 0xe1fe, capacity: 0x1e02 },
];
let cursor = 20;
const segments = expected.map((spec, index) => {
  const origin = bundle.readUInt16LE(8 + index * 4);
  const length = bundle.readUInt16LE(10 + index * 4);
  if (origin !== spec.origin) fail(spec.name + ' origin mismatch');
  if (length === 0 || length > spec.capacity) fail(spec.name + ' length exceeds capacity');
  const bytes = bundle.subarray(cursor, cursor + length);
  if (bytes.length !== length) fail('truncated ' + spec.name + ' segment');
  cursor += length;
  return { ...spec, length, endExclusive: origin + length, bytes };
});
if (cursor !== bundle.length) fail('unexpected trailing bundle bytes: ' + (bundle.length - cursor));
if (entry < segments[1].origin || entry >= segments[1].endExclusive) fail('entry is outside main segment');
if (segments[0].endExclusive > segments[1].origin) fail('extra overlaps main');
if (segments[1].endExclusive > 0xc000) fail('main overlaps state');
if (segments[2].endExclusive > 0x10000) fail('high exceeds address space');

fs.mkdirSync(outputDir, { recursive: true });
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hex = value => '0x' + value.toString(16).padStart(4, '0');
const artifacts = [];
for (const segment of segments) {
  const prg = Buffer.alloc(2 + segment.length);
  prg.writeUInt16LE(segment.origin, 0);
  segment.bytes.copy(prg, 2);
  const filename = 'reference-ops-' + segment.name + '.prg';
  fs.writeFileSync(path.join(outputDir, filename), prg);
  artifacts.push({
    name: segment.name,
    file: filename,
    load_address: hex(segment.origin),
    load_end_exclusive: hex(segment.endExclusive),
    payload_bytes: segment.length,
    capacity_bytes: segment.capacity,
    margin_bytes: segment.capacity - segment.length,
    sha256: sha256(prg),
  });
}
const manifest = {
  schema: 'ksa64.phase11.reference-ops-banked-bundle.v1',
  source_bundle: path.relative(process.cwd(), input).replaceAll('\\', '/'),
  bundle_bytes: bundle.length,
  bundle_sha256: sha256(bundle),
  entry: hex(entry),
  mailbox: { start: '0x0200', end_exclusive: '0x0410' },
  result: { start: '0x0410', end_exclusive: '0x0428' },
  emergency_software_stack: { start: '0x0428', end_exclusive: '0x053f', bytes: 279 },
  package_state_and_static_stack: { start: '0xc000', end_exclusive: '0xe1fe', bytes: 8702 },
  banking: {
    cpu_port: '0x34',
    policy: 'interrupts disabled; BASIC, KERNAL, and I/O mapped out after entry; no ROM or I/O calls',
    reu_required: false,
  },
  artifacts,
};
fs.writeFileSync(path.join(outputDir, 'reference-ops-banked-manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(JSON.stringify(manifest, null, 2));
