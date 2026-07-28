import { crc32, KPS1_EVIDENCE_OBJECT_MAX } from "../protocol/kps1";
import type { EvidenceChunk, EvidenceMetadata } from "../protocol/presentation";

export interface EvidenceAssembly {
  readonly metadata: EvidenceMetadata;
  readonly chunks: ReadonlyMap<number, EvidenceChunk>;
  readonly receivedLength: number;
  readonly sealed?: Uint8Array;
}

function sameMetadata(left: EvidenceMetadata, right: EvidenceMetadata): boolean {
  return left.evidenceIdentity === right.evidenceIdentity && left.evidenceCrc32 === right.evidenceCrc32 &&
    left.totalLength === right.totalLength && left.chunkLength === right.chunkLength &&
    left.chunkCount === right.chunkCount && left.complete === right.complete && left.contentKind === right.contentKind;
}

export function beginEvidenceAssembly(current: EvidenceAssembly | undefined, metadata: EvidenceMetadata): EvidenceAssembly {
  if (current !== undefined && sameMetadata(current.metadata, metadata)) return current;
  if (!metadata.complete) return { metadata, chunks: new Map(), receivedLength: 0 };
  if (metadata.totalLength > BigInt(KPS1_EVIDENCE_OBJECT_MAX) || metadata.totalLength > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("sealed evidence exceeds the presentation object bound");
  }
  return { metadata, chunks: new Map(), receivedLength: 0 };
}

export function appendEvidenceChunk(current: EvidenceAssembly | undefined, chunk: EvidenceChunk): EvidenceAssembly {
  if (current === undefined || !current.metadata.complete) throw new Error("evidence chunk arrived before complete metadata");
  const metadata = current.metadata;
  if (chunk.evidenceIdentity !== metadata.evidenceIdentity || chunk.chunkCount !== metadata.chunkCount) {
    throw new Error("evidence chunk identity does not match metadata");
  }
  const expectedOffset = BigInt(chunk.chunkIndex) * BigInt(metadata.chunkLength);
  const totalLength = Number(metadata.totalLength);
  const expectedLength = Math.min(metadata.chunkLength, totalLength - Number(expectedOffset));
  if (chunk.logicalOffset !== expectedOffset || expectedLength <= 0 || chunk.bytes.byteLength !== expectedLength) {
    throw new Error("evidence chunk offset or length is invalid");
  }
  const existing = current.chunks.get(chunk.chunkIndex);
  if (existing !== undefined) {
    if (existing.logicalOffset !== chunk.logicalOffset || existing.bytes.byteLength !== chunk.bytes.byteLength ||
        existing.bytes.some((value, index) => value !== chunk.bytes[index])) throw new Error("conflicting duplicate evidence chunk");
    return current;
  }
  const chunks = new Map(current.chunks);
  chunks.set(chunk.chunkIndex, chunk);
  const receivedLength = current.receivedLength + chunk.bytes.byteLength;
  if (chunks.size !== metadata.chunkCount) return { metadata, chunks, receivedLength };
  if (receivedLength !== totalLength) throw new Error("sealed evidence chunk total is incomplete");
  const sealed = new Uint8Array(totalLength);
  for (let index = 0; index < metadata.chunkCount; index += 1) {
    const part = chunks.get(index);
    if (part === undefined) throw new Error("sealed evidence has a missing chunk");
    sealed.set(part.bytes, Number(part.logicalOffset));
  }
  if (crc32([sealed]) !== metadata.evidenceCrc32) throw new Error("sealed evidence CRC mismatch");
  return { metadata, chunks, receivedLength, sealed };
}
