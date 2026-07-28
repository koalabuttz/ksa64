# Frozen Phase 12B bridge header audit

Status: historical metadata discrepancy isolated; frozen executable and manifest bytes preserved.

## Finding

The accepted Win64 bridge manifest
`ksa64_viewer_bridge-423c116cf586-120b0001.manifest.json` records header
SHA-256 `8227d7d7de442049eb71d23178a9d9703bc228668e958edfc4d7100d694a682e`.
The header tracked at the manifest's declared source commit
`423c116cf58632f344d4a48774a97a4487c34113` is Git blob
`a18a749037e07d24382a1b1aab1d9c548a161ba1`, 13,738 bytes, with SHA-256
`ad0b69c66b2232b97cc1675795c1be054abf246b65ea1bb0c92b463407d20db1`.
The accepted Unreal header mirror has those same bytes.

| Reachable revision | Git blob | Bytes | SHA-256 |
|---|---|---:|---|
| `495ba01` | `7ce95486` | 5,559 | `ae863a6c...` |
| `e5300b6` | `4cce6c56` | 12,963 | `4ba38d11...` |
| `d3f53df` through accepted `423c116` | `a18a7490` | 13,738 | `ad0b69c6...` |
| Phase 12B.5 `1b8122d` onward | `c5d994ba` | 14,817 | `b0986aaa...` |

The recorded digest does not identify any tracked revision of
`viewer-bridge/ksa64_viewer_bridge.h`. It also does not match the CRLF,
UTF-8 BOM, missing-final-newline, UTF-16LE, or UTF-16BE forms of the accepted
header. A bounded scan of plausible unreachable header-sized Git blobs found no
match. The original transient bytes represented by `8227d7...` are therefore
not recoverable from repository history.

This is a noncanonical validation-metadata defect. It does not change the
accepted ABI or mission evidence.

## Independent compatibility evidence

The frozen DLL remains exactly:

- file: `ksa64_viewer_bridge-423c116cf586-120b0001.dll`;
- size: 944,640 bytes;
- SHA-256: `da6657a46759a028cb8901ce813af093d4d8901c76cb383f0d74601d64f26565`;
- ABI major: 1;
- build identity: `0x120B0001`.

The independent ABI harness loads that DLL through the accepted tracked header
and passes symbol resolution, structure layout, misuse, ownership, lifecycle,
event-order, and frozen compact-session checks. The independent full-mission
harness also passes at release 21,591 with four accepted actions and produces
the exact 2,911,464-byte KSB11 archive with SHA-256
`7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`.

## Compatibility treatment

The verifier leaves the frozen manifest, DLL, accepted source header, and all
canonical evidence unchanged. It admits the tracked header only when every
member of this exact compatibility tuple matches:

- exact manifest filename and SHA-256 `b618e31c08b185e40db83955dc47cb8440e488779dfab1f7899307abf9852365`;
- manifest schema `ksa64.viewer-bridge-manifest.v1`;
- full source commit `423c116cf58632f344d4a48774a97a4487c34113`;
- accepted DLL digest `da6657a...`;
- frozen manifest header digest `8227d7...`;
- actual tracked header digest `ad0b69c6...`;
- ABI major 1 and build identity `0x120B0001`;
- accepted 13-entry catalog digest `b7456cfd...`.

Any other header mismatch still fails closed. The portable Phase 12B.5 header
and manifest-v2 lane are separate and receive no exception.
