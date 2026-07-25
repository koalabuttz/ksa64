# Phase 8.5 Stock-C64 Fit Decision

The required self-contained combined world-plus-avionics image reached the plan's explicit stock-fit decision boundary. It does **not** fit in a stock C64 without removing features, introducing a phase overlay, or adding expansion memory.

## Measured result

- Pinned toolchain: `ghcr.io/koalabuttz/rust-mos:ac2fb2277-4537158-4aaa40e16`.
- Smallest configured profile: `opt-level = "z"`, whole-program LTO, one code-generation unit.
- Ordinary C64 linker region: 51,199 bytes (`$0801-$CFFF`).
- Required resident image: 71,500 bytes.
- Ordinary-region deficit: 20,301 bytes.
- The stricter original flat-image gate below `$C000` is short by 24,397 bytes.
- RAM hidden beneath I/O and KERNAL adds at most 12,288 bytes, leaving a best-case deficit of 8,013 bytes.
- Even the impossible assumption that all 65,536 physical bytes were available leaves a 5,964-byte deficit before zero page, the hardware stack, screen memory, I/O, or the mailbox/result area are reserved.

The custom banking step was therefore ruled out before writing unsafe trampolines: banking changes which RAM is visible, but cannot make a 71,500-byte resident payload fit in 65,536 physical bytes.

## Measured optimization evidence

The standalone avionics kernel was optimized where timing evidence identified the real hotspot. It now costs 21,184 cycles for an aided release and 10,843 cycles for a fast release, passing the 24,631-cycle PAL 80% budget with 3,447 cycles of aided-path headroom.

For size, forcing the two largest boundaries out of line made the deficit **worse**, increasing the then-current deficit from 20,096 to 22,520 bytes. That experiment was reverted. The remaining largest linked contributions are:

| Contribution | Bytes |
|---|---:|
| Combined main / inlined executor | 28,272 |
| Force evaluation | 6,004 |
| Attitude step | 2,758 |
| Mass-property derivation | 1,852 |
| Vehicle validation | 1,450 |
| World-result construction | 1,287 |
| Scaled division | 1,006 |
| Deterministic gust target | 1,004 |

Further compression is no longer a small packaging task. It means hand-specializing or rewriting major physics kernels, or changing the execution architecture.

## Supported stock-C64 capability

This does **not** prevent stock-C64 avionics operation:

- The generic monitor/gimbal flight-computer endpoint is 15,412 bytes and fits at `$0801-$4432`.
- Monitor-only and gimbal finite host/VICE exchanges pass with identical KLF6/KLR8 ordering and close VICE immediately afterward.
- The standalone kernel meets its real-time PAL release budget.
- The frozen Phase 8 physical world remains independently runnable on stock hardware.
- Host/host and host-world plus VICE/C64-flight placements remain supported, including live F1-F7 Mission Control.

## Options requiring user direction

The plan forbids selecting these automatically:

1. **Phase overlays from disk:** keep world state and avionics resident, load powered/coast/recovery kernels at phase boundaries. This preserves a stock machine but makes the combined target disk-assisted rather than a single self-contained image.
2. **A separate stock-specialized executor:** aggressively fuse and hand-rewrite the local world and avionics loop, likely including assembly. This may recover the remaining 7.8 KB, but carries meaningful verification and maintenance cost.
3. **Optional expansion memory:** use an REU or cartridge for cold code/tables. This is easiest technically but cannot replace the promised stock baseline without explicit approval.
4. **Feature partitioning:** remove functionality from the combined build while retaining it in host/VICE placements. The Phase 8.5 plan explicitly forbids doing this silently.

Accordingly, implementation stops at the required decision boundary. No REU requirement, feature removal, or split executable has been imposed.
