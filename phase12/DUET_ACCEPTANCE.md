# Lenovo Duet 11 physical acceptance

Phase 12B.5 requires a physical result from the 8 GB ARM64 Lenovo Duet 11.
This record is deliberately **pending** until the commands below are run on that
device; CI and desktop emulation do not substitute for it.

## Required topologies

1. Crostini-native authority and CLI/TUI.
2. Crostini authority with the ChromeOS PWA over forwarded loopback.
3. Complete local Rust/WASM authority in a dedicated browser worker.
4. WebGPU, forced WebGL2, and presentation-only 2-D fallback.
5. Installed/offline PWA shell, service-worker update, worker termination, tab
   suspension, and recovery from an explicitly incomplete browser session.

No realtime mission requirement applies. The UI must remain responsive and the
authoritative results must remain exact.

## Preparation

- Debian Crostini on the physical 8 GB Duet.
- The repository at the commit being qualified.
- Rust 1.93 with `wasm32-unknown-unknown`, Node 24, npm, a C/C++ compiler, and
  `/usr/bin/time`.
- ChromeOS and Chrome versions recorded below.

Run from the repository root:

```sh
./phase12/duet-acceptance.sh evidence/phase12b5/duet
```

The script refuses non-ARM64 hosts. It verifies the exact native presentation
session, builds the ARM64 product and bridge, runs the exported WASM authority,
and records startup, timing/RSS, archive-write, storage, and available-worker evidence. Browser lifecycle and rendering checks remain
manual because Crostini cannot honestly synthesize ChromeOS suspension or GPU
backend behavior.

## Physical evidence record

| Field | Result |
|---|---|
| Device | Lenovo Duet 11, 8 GB |
| Status | **Pending physical run** |
| ChromeOS build | pending |
| Chrome build | pending |
| Debian/Crostini release | pending |
| Kernel / architecture | pending |
| Rust / Cargo / Node / npm | pending |
| Source commit | pending |
| Native exact KSB11 | pending |
| Local-WASM exact KSB11 | pending |
| Startup / mission elapsed | pending |
| Peak RSS | pending |
| Archive-write time | pending |
| WebGPU / WebGL2 / 2-D | pending |
| PWA offline/update | pending |
| Worker failure / suspension | pending |

## Acceptance rule

Accepted evidence must show release 21,591, four canonical actions, exactly
2,911,464 KSB11 bytes, and SHA-256
`7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4`
for both native ARM64 and local WASM. A stopped or suspended worker is incomplete
and must never be labelled completed.
