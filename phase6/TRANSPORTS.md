# Phase 6 transport and endpoint evidence

Status: transport contracts and finite stock endpoint packaging accepted; live multi-endpoint bridge remains in progress.

## Capability ladder

All transports implement the same nonblocking byte-stream contract. KLF6 owns session, replay, retry, identity, sequence, CRC-32, and deterministic transcript rules. KLR6 supplies compact fixed-size CRC-16 cells for KSA-6R.

| Transport | Baseline | Intended profile |
|---|---|---|
| In-process/native mock | development host | exact and realtime |
| C64 user-port serial | stock C64 plus protected interface cable | exact paced at 9,600 baud |
| SwiftLink/Turbo232 ACIA | cartridge or VICE emulation | realtime at 57,600 baud |
| Ultimate UCI TCP | optional Ultimate hardware | exact and realtime |

At 8-N-1, a worst world-to-flight KSA-6R release is 104 bytes and a worst flight-to-world release is 72 bytes. They require 108.334 ms and 75 ms at 9,600 baud, so stock user-port serial cannot carry the 32 Hz profile. At 57,600 baud they require 18.056 ms and 12.5 ms, both inside the 31.25 ms fast slot. User-port serial therefore remains a universal exact-paced path; ACIA or UCI is required for realtime linking.

VICE 3.10 exposes socket-backed RS-232 devices, user-port UP9600, and ACIA modes for Normal, SwiftLink, and Turbo232. The planned automated bridge uses `-acia1`, base `$DE00`, Turbo232 mode, and a socket-backed RS232 device. See the [VICE RS-232 settings](https://vice-emu.sourceforge.io/vice_6.html#SEC132).

The memory-mapped adapter accepts `$DE00` or `$DF00`. Its polling setup resets the 6551, selects enhanced 8-N-1 mode, selects Turbo232 code `10` for 57,600 baud, and disables data interrupts. Register meanings are recorded in the [Turbo232/SwiftLink register description](https://rr.pokefinder.org/wiki/Turbo232_Swiftlink_Registers.txt).

## Stock endpoint probe

The finite rust-mos probe includes:

- the 398-slice Q15/Q12 guidance table and signature;
- KLR6 aid, inertial, command, and status codecs;
- the resynchronizing compact-cell receiver and bounded transmitter;
- the KSA-6R scheduler, navigation, sequencing, guidance interpolation, controller, and evidence chains;
- a direct nonblocking byte-feed adapter shaped like the hardware driver.

Three pinned VICE runs agree on all result words. The 16,147-byte image loads at `$0801` and ends at `$4712`, safely below `$C000`. A generic host `MemoryTransport` prefill exposed a rust-mos-only queue specialization issue; the accepted target probe uses sequential byte delivery, matching the physical driver and broker contract. The generic transport remains covered by native conformance tests.

## Ultimate path

`UltimateUciTransport` is a bounded connect/poll/read/write/fault state machine over a device trait. Its deterministic native mock covers connection delay, backpressure, receive, fault latching, and reconnect entry. Actual Ultimate discovery, register arbitration at `$DF1B-$DF1F`, and physical Ethernet acceptance remain optional-hardware work; they cannot block stock or VICE completion.
