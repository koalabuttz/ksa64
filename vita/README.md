# KSA64 Vita feasibility client

This folder contains the Phase 12B.5 PlayStation Vita client foundation. It is a **presentation-only** KPS1 consumer: it never owns KSA64 world state, flight software, action validation, or canonical evidence encoding.

## Status

The host fixture builds without VitaSDK and verifies KPS1 parsing, bounded state retention, action intent construction, resynchronization signaling, and the 64 MiB client working-set budget. It is not an emulator or physical-device acceptance result.

The VPK path is deliberately pinned and documented, but needs a local VitaSDK, `cargo-vita`, SDL2, Vita3K, and physical Vita before it can qualify the Phase 12B.5 target claims.

## Architecture

```text
paired LAN / replay KPS1
          |
          v
ksa64-vita-client (static presentation crate)
          |
          +-- fixed-capacity Vita view model
          +-- role/connection/staleness/resync indicators
          +-- Review -> Stage -> Commit / Cancel intents
          +-- compact 960x544 page model
          |
          v
SDL2/Vita frontend (later platform shell)
```

The user interface has six pages: status, navigation, procedure, trajectory, timeline, and evidence. The library can run on a host through `vita-fixture`; the eventual SDL2 layer is intentionally separate so it cannot become a second simulation.

## Host fixture

```powershell
cargo +stable run --manifest-path vita/Cargo.toml -p ksa64-vita-client --bin vita-fixture
cargo +stable test --manifest-path vita/Cargo.toml -p ksa64-vita-client
```

## Pinned target lane

See [toolchain-manifest.toml](toolchain-manifest.toml), [build-vpk.ps1](build-vpk.ps1), and [VITA3K_SMOKE.md](VITA3K_SMOKE.md).

The target uses Rust's Tier 3 `armv7-sony-vita-newlibeabihf` lane with the exact nightly, VitaSDK, and `cargo-vita` versions recorded in the manifest. The final platform adapter should statically link this crate and the KPS1 contract; it must not load the desktop 64-bit bridge.

## Physical acceptance still required

- 960x544 layout and controls
- paired-LAN comparison-code confirmation
- disconnect/reconnect and resynchronization
- Vita suspend/resume
- memory and 30 fps presentation measurements
- offline replay

No physical Vita result is claimed by this repository yet.
