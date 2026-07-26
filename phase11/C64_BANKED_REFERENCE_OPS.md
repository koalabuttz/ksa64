# Phase 11 banked stock-C64 reference operations

Status: accepted stopgap for the externally paced host-world/C64-flight baseline.

## Why this exists

The complete portable KsaG10rReferenceOpsV1 package could not fit in one flat image ending below $C000. The independent SafeholdRecoveryV1 endpoint already fits flat, but it does not provide the reference package's mission-plan, uplink, prediction, and journal services.

The accepted stopgap keeps the portable Rust package unchanged and uses stock C64 RAM that is normally hidden by BASIC, I/O, and KERNAL. It requires no REU. A future 6502-specific rewrite and C64 Ultimate acceleration remain separate target-engineering tracks.

## Memory contract

| Range | Purpose | Accepted use |
|---|---|---:|
| $0200-$040F | Externally paced operation mailbox and 512-byte codec workspace | 528 bytes |
| $0410-$0427 | Endpoint result and fail-closed record | 24 bytes |
| $0428-$053E | Emergency C software stack | 279 bytes; 16-byte measured high-water |
| $053F-$0800 | Low helper-code bank | 706 bytes; exact fit |
| $0801-$BFCC | Main code and read-only data | 47,052 bytes; 51-byte guard |
| $C000-$E1FD | Portable package state plus compiler static stack | 8,702 bytes |
| $E1FE-$FFDA | High helper-code bank beneath KERNAL | 7,645 bytes; 37-byte guard |

Entry is $080D. The first target initializer disables maskable interrupts and CIA/VIC interrupt sources, then writes $34 to the C64 CPU port. BASIC, I/O, and KERNAL are unavailable afterward. The headless endpoint performs no ROM, display, IEC, or other I/O call while active.

The custom linker deliberately discards the default C64 charset initializer. That initializer calls KERNAL CHROUT at $FFD2; after KERNAL is hidden, the call would enter banked application code instead. The VICE gate exposed and proved this failure before the initializer was removed.

## Build and package

From the repository root:

~~~powershell
& phase11/c64-banked/build.ps1
~~~

The script:

1. builds the rust-mos endpoint through the pinned Docker toolchain and custom linker;
2. emits and validates a three-segment KSB1 bundle;
3. packages three load-addressed PRGs plus a SHA-256 manifest; and
4. generates a native 13-operation KOT1 exactness transcript.

Generated files live under target/phase11-c64-banked/. The linker map is target/phase11-reference-banked.map.

## Finite VICE acceptance

The accepted probe loads all three segments directly through the VICE binary monitor, maps and verifies hidden RAM, starts at $080D, and replays the native transcript:

~~~powershell
python phase11/reference/vice_reference_ops_banked.py --vice .toolchains/vice/3.10/GTK3VICE-3.10-win64/bin/x64sc.exe --image-dir target/phase11-c64-banked --transcript target/phase11-c64-banked/reference-ops-transcript.bin
~~~

The probe uses one hidden PAL VICE instance, leaves warp disabled, and closes the instance after success or proven failure. It covers ordinary and 8 Hz aided releases, compact prediction, staged and committed uplink, ground blackout/reacquisition, and ordered journal recovery.

Accepted evidence:

- 13 native/C64 byte-exact operations;
- navigation checksum c73060d2;
- flight checksum 6e07595c;
- command checksum 6ab926f2;
- code segments unchanged;
- both bank guards preserved;
- 16 of 279 emergency-stack bytes used;
- no REU and no realtime claim.

Host wall time is diagnostic only. The first aided release took about 2.73 seconds in the accepted run, so the host must pace the world by acknowledgment rather than by wall-clock 32 Hz.

## Scope and remaining work

This is a stock-CPU and stock-64-KiB execution result in VICE. A physical C64 still needs a validated loader/transport capable of placing the hidden banks before handing control to the endpoint. Physical user-port, ACIA, or Ultimate Ethernet acceptance remains a separate hardware boundary.

The banked stopgap does not replace:

- the future 6502-specific reference-package rewrite;
- C64 Ultimate RAM/accelerator investigation;
- physical link and physical loader acceptance;
- the portable C64 world; or
- realtime advanced/global flight work.

It does let Phase 11 continue with the accessible host-world/C64-flight architecture while preserving one portable implementation and the exact KLR10 behavior.
