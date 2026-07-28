# Phase 12C.5 handoff — portable operations productization

Status: **draft; activates only after Phase 12C acceptance**.

Date: 2026-07-28

Phase 12C.5 consumes the renderer-neutral `GlobalDisplayV1` and the accepted
Phase 12B.5 presentation/session boundary. It does not reopen coordinate,
authority, role, replay, evidence, or mission-disposition decisions.

## Required entry evidence

Every item is pending until the Phase 12C completion record is accepted:

- exact 22,015-release nominal global display replay;
- exact 21,591-release guided GNSS-loss display replay;
- unchanged 13-entry catalog, Phase 10 artifacts, and KSB11;
- accepted additive KPS1 capability and optional C function table;
- cross-renderer equality at all nine nominal and six guided action/fault
  milestones, including complete source/path products, event and discontinuity
  masks, continuity identity, temporal/event-aware path checksums, raw path-state
  flags, and the normalized shared view mode, recorded by
  `ksa64.phase12c.cross-renderer-evidence.v2`;
- packaged Win64/D3D12 procedural viewer whose runtime evidence binds the
  source commit, bridge, semantic captures, screenshots, and package inventory;
- exercised Babylon WebGPU, forced-WebGL2, context-loss, and 2-D fallback;
- truth-isolation, exact-snap, seek, reconnect, and render-invariance evidence;
- recorded package, memory, path, publication, polling, and frame-rate metrics;
  and
- an accepted [Phase 12C completion record](PHASE12C_COMPLETION.md).

## Phase 12C.5 owns

- product-quality web/PWA Mission Control and global viewing;
- browser-LAN certificate and pairing user experience;
- physical Lenovo Duet qualification and measured Chromebook tiers;
- Vita3K and physical Vita completion of the carried Phase 12B.5 workstream;
  these qualifications were deliberately decoupled from Phase 12C renderer
  completion but are product gates for claims about those devices;
- refined SDL2/Vita status, replay, procedures, and high-level actions;
- Android ARM64 presentation packaging;
- iOS/iPadOS presentation packaging once the Mac/Xcode/signing lane is pinned;
- mobile lifecycle, thermal, memory, suspension, reconnect, and offline behavior;
- optional measured local authority placements on devices that can reproduce
  accepted evidence exactly; and
- accessibility, input, layout, installation, update, and device-performance
  product evidence.

## Phase 12C.5 does not own

- new or alternate physics, avionics, navigation, frame, time, event, or
  evidence authority;
- renderer-side parsing of KSB11 or canonical K records;
- direct effector commanding;
- cloud accounts, public Internet listeners, relay services, discovery, UPnP,
  or multi-controller arbitration;
- vehicle/mission authoring, which remains Phase 12D;
- production NASA imagery, final vehicle art, terrain, effects, and broad
  distribution polish, which remain Phase 12E;
- portable C64-world work, a 6502 rewrite, Ultimate acceleration, or physical
  C64 link acceptance; or
- certification, launch approval, regulatory acceptance, or safety authority.

## Authority and lifecycle rules

Every portable client consumes role-filtered Rust presentation products.
Remote authority continues through a client disconnect. Local authority that
is suspended, terminated, or loses its worker remains incomplete unless a
separately validated checkpoint contract reconstructs it exactly. A mobile or
browser shell may never infer that missed wall time was simulated.

Renderer backend, screen size, touch/controller input, accessibility mode,
quality tier, local origin, path LOD, polling cadence, and connection placement
remain outside mission and evidence identity. `RayTracingMode=Inline` is an
Unreal Launcher compile-time ABI accommodation only; runtime ray tracing is
disabled and cannot become a portable-client or hardware requirement.
