# Phase 7 completion record

Status: accepted on 2026-07-24.

Phase 7 is complete. It introduces an additive multi-profile evaluation facade,
a separately scaled hobby-rocket numeric contract, offline compiled vehicle and
mission packs, a deterministic vertical ascent/recovery mission, strict
telemetry and campaign formats, independent analysis, and stock-C64 execution.

## Gate record

| Gate | Commit | Accepted result |
|---|---|---|
| Roadmap | `2c39727` | Phase 7–9 responsibilities separated. |
| Legacy facade | `8764de0` | KSA-2A/KSA-5A adapters preserve frozen executors. |
| Numeric/formats | `d897104` | Hobby SI fixed-point contract and KVP7–KRA7 framing frozen. |
| Pack compiler | `b8371cf` | Offline JSON/RASP sources compile to bounded packs. |
| Vertical mission | `9baf6bf` | Rail, sampled thrust, coast, dual deploy, and landing accepted. |
| Evidence | `f708dfc` | KST7/KSR7 and independent float64 mission evidence accepted. |
| Campaign/storage | `0e5bde9` | Candidate lists, keyed uncertainty, KPH7, and KRA7 accepted. |
| Stock C64 | `52e506b` | Exact trace, replay, complete mission, and PAL timing accepted. |

## Frozen mission evidence

- KST7: 42,144 bytes, 438 frames, SHA-256
  `b12ec2a06dfa071e5f30678769ea66321606aaa4deb824082ac67682a6db98bd`.
- KSR7: 192 bytes, SHA-256
  `a0042a7e1ac2445b6a088e259721652b12b9bb378e0b4d5d9166b186e69e6498`.
- KPH7: 2,052 bytes, 124 points, SHA-256
  `29664a4bae703f0ef7e6440645ce59dff7b3c13f56b97d40688f177af6a3bec2`.
- Exact mission: 2,702 steps, all event bits `0x01ff`, zero numeric faults,
  state checksum `0xa61c5720`.
- Exact apogee: 978.066040 m; float64 reference: 978.075735 m.
- Exact impact velocity: -6.156540 m/s; float64 reference: -6.156093 m/s.

## Frozen campaign evidence

The 1,024-run reference campaign uses seed `0x4b534137`.

- KSC7 SHA-256:
  `c60b9b76e813dec23f31cf6c3f6a608477f4da92a3554841768f694303b29e6d`.
- KRA7: 204,864 bytes, SHA-256
  `0075b65ca99242e97d38acd4817048adef1b986e4ac395c4855fec84bc348295`.
- Ordered KSR7 CRC-32: `0xa939041c`.
- One-worker and four-worker artifacts are byte-identical.
- All 1,024 cases reach recovered ground contact.
- Apogee range: 860.819580–1,097.143921 m.
- Impact-velocity range: -6.564833 to -5.797173 m/s.

`reference/analyze_campaign.py` independently validates KSC7, KRA7, every
embedded KSR7, keyed draws, variation identities, ordering, reserved fields,
and CRCs without using the Rust codecs.

## Stock-C64 evidence

| Program | Bytes | End address | Result |
|---|---:|---:|---|
| Full mission | 21,884 | `$5D7B` | Exact landing and checksum |
| KPH7 replay | 10,076 | `$2F5B` | 1,000-byte screen accepted |
| 129-state trace | 21,818 | `$5D39` | Native/MOS exact |

The full mission used 1,047,635,269 net PAL cycles, 387,725.86 cycles per
simulation step, and projects to 1,063.32 seconds (17.72 minutes) on a stock PAL
C64. This is below the accepted 30-minute threshold, so the complete run—not
only a projection—forms part of completion evidence.

During target acceptance, the trace found a target-only error at step 24: the
250 m environment-table stride had been cast to 16-bit `usize` before division.
The final implementation divides signed 32-bit raw altitude by the raw stride
before converting the bounded quotient to an index. The repaired 129-state
trace and full mission are exact.

## Compatibility and limitations

All accepted Phase 0–6 tests pass. Phase 2 and Phase 5 missions continue to use
their original executors and artifacts. KSA-6R remains the Phase 6 realtime
flight/link profile over the KSA-5A world.

The hobby reference uses published component data plus declared assumptions; it
is not correlated to a flown Firestorm/I211W and is not certification,
regulatory, safety, or probability evidence. Spatial stability, wind,
weathercocking, recovery drift, derived CG/CP/inertia, and external correlation
are Phase 8 work. Search algorithms, Pareto analysis, robust objectives, and
finalist browsing are Phase 9 work.

The bounded audit command is:

```powershell
powershell -File phase7/complete.ps1
```

It verifies frozen full-flight evidence and binary hashes but reruns only the
finite target trace and replay. A fresh full C64 mission remains an explicit,
non-routine action.
