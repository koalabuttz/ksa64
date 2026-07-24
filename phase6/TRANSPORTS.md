# Phase 6 transport and endpoint evidence

Status: portable transport software accepted; live physical-link acceptance pending.

Every Phase 6 adapter implements a nonblocking byte-stream boundary. KLF6 owns identity, framing, sequence, replay, retry, CRC-32, and transcript behavior. KLR6 supplies compact fixed-size CRC-16 cells for KSA-6R. TCP reliability never replaces application framing or checksums.

## Capability ladder

| Transport | Intended profile | Current evidence |
|---|---|---|
| In-process/native mock | Exact and realtime | Complete conformance, impairment, backpressure, and full mission |
| Native TCP socket | Realtime broker | Complete 12,692-epoch split mission with exact shadow verification |
| VICE binary-monitor mailbox | Target acceptance only | One-cell smoke and complete externally paced 1x PAL flight |
| C64 user-port serial at 9,600 baud | Exact paced | Bandwidth/policy accepted; electrical interface and target driver pending |
| SwiftLink at 38,400 baud | Realtime candidate | Register driver and stock endpoint built; hardware run pending |
| Turbo232 at 57,600 baud | Realtime candidate | Register driver and stock endpoint built; VICE/physical receive run pending |
| Ultimate UCI TCP | Exact and realtime candidate | State machine and native mock accepted; target hardware adapter pending |

## Bandwidth

At 8-N-1, a worst world-to-flight KSA-6R release is 104 bytes and a worst flight-to-world release is 72 bytes.

| Baud | World release | Flight release | 31.25 ms slot |
|---:|---:|---:|---|
| 9,600 | 108.334 ms | 75.000 ms | Does not fit |
| 38,400 | 27.084 ms | 18.750 ms | Fits wire time, narrow processing margin |
| 57,600 | 18.056 ms | 12.500 ms | Fits |

Stock user-port serial therefore remains the universal exact-paced path. Realtime linking requires a faster ACIA, Ultimate UCI, or another measured transport.

## Stock endpoint images

| Program | Purpose | Bytes | Loaded end exclusive |
|---|---|---:|---:|
| `ksa64-phase6-flight-endpoint-c64` | Physical SwiftLink/Turbo232 flight endpoint | 17,554 | `$4C91` |
| `ksa64-phase6-mailbox-endpoint-c64` | VICE acceptance endpoint | 15,324 | `$43DB` |
| `ksa64-phase6-endpoint-probe-c64` | Codec/flight/package probe | 16,491 | `$486A` |
| `ksa64-phase6-realtime-timed-c64` | CIA release timing | 14,924 | `$424B` |

All load below `$C000` and require no REU. The physical endpoint emits the four-byte `KLR6_READY` preamble, then receives optional aid plus inertial cells and returns command plus periodic status cells.

## ACIA configuration

The target adapter accepts `$DE00` or `$DF00`. It can configure SwiftLink 38,400 baud or Turbo232 enhanced 57,600 baud, uses 8-N-1 polling, keeps transmitter enable set, and rejects status fault bits. The earlier command value `$03` disabled the transmitter; the accepted polling command is `$0B`.

The pinned Windows x64sc 3.10 socket backend transmitted C64-to-host bytes but did not deliver host-to-C64 bytes in the tested Normal, SwiftLink, Turbo232, raw, and IP232 configurations. A child-pipe experiment also failed to start reliably. These failures were closed promptly and are not counted as acceptance evidence. Automated C64 acceptance therefore uses a monitor mailbox while the physical ACIA image remains ready for later hardware validation.

## Mailbox acceptance adapter

The VICE-only mailbox reserves `$C800-$C8FF` for one inbound and one outbound KLR6 exchange plus sequence/acknowledgement bytes. The host binary monitor pauses the C64, writes one complete input set, resumes execution, waits for the matching acknowledgement, then reads the result. The adapter never exists in the physical endpoint and does not alter the flight-computer model.

The full relay runs exactly one VICE process. Endpoint initialization and each post-readiness epoch have a 120-second progress bound; the complete mission has no total time limit. Success, proven failure, unexpected harness error, and explicit keyboard interruption all close VICE and the native broker. Routine audit refuses to launch if another x64sc process is already present.

## Ultimate path

`UltimateUciTransport` is a bounded connect/poll/read/write/fault state machine over a device trait. The native mock covers connection delay, receive, transmit backpressure, closure, fault latching, and reconnect entry. Actual UCI discovery, register arbitration at `$DF1B-$DF1F`, and physical Ethernet acceptance remain hardware work. Ultimate support is optional and cannot become a stock-C64 requirement.

## Physical safety

No original C64 user-port pin should be connected directly to an incompatible host serial standard. The eventual user-port acceptance record must specify voltage levels, protection, directionality, handshaking, cable pinout, reset behavior, and disconnect safeing before it recommends hardware construction.
