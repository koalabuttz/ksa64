# KSA64 Vita Mission Control

This folder contains the Phase 12B.5 PlayStation Vita presentation client. It is a **presentation-only**, role-filtered KPS1 consumer: it never owns KSA64 world state, flight software, action validation, or canonical evidence encoding.

## Implemented target

The target is now a real 960x544 SDL2 application and a packageable VPK, not a placeholder. It provides six bounded pages:

- mission status and multidimensional disposition;
- onboard and ground navigation with residuals;
- procedure state and public guards;
- a compact altitude/downrange trajectory plot;
- a severity-coded event timeline;
- evidence identities, checksums, and client resource bounds.

The bundled offline fixture is a role-filtered GNSS-loss contingency-success replay. It contains no SIM Director truth and accepts no authoritative actions.

A real, explicit opt-in LAN runtime is also compiled into the VPK. It uses VitaSDK networking, fresh `sceKernelGetRandomNumber` entropy, Noise XX pairing with a six-digit code that must be confirmed locally on both devices, a remembered host public key, and Noise IK for later reconnects. It carries only length-prefixed encrypted KPS1 packets, bounds inbound and outbound queues, treats a disconnect as stale while remote authority continues, and visibly requires resynchronization after a retention gap. The Vita role is permanently `GuidedOperator`; direct effector commands remain impossible.

Controls:

| Control | Action |
|---|---|
| D-pad left/right | Change page |
| D-pad up/down | Scroll retained timeline |
| Cross | Review, Stage, then Commit when a live proposal permits it |
| Circle | Cancel when permitted |
| Triangle | Request resynchronization when required |
| Select | Return retained timeline to newest events |
| Start | Exit |

Direct gimbal, RCS, canard, parachute, or other effector commands do not exist in this client.

## Opt-in paired LAN configuration

Offline replay is the default. To enable a private-LAN connection, create this file on the Vita writable user partition:

    ux0:data/KSA64/vita-lan.conf

    mode=pair
    host=192.168.1.42
    port=27864
    session_nonce=4b53413600000001

Only a private or link-local address, nonzero port, and nonzero hexadecimal session nonce are accepted. There is no discovery, wildcard bind, UPnP, Internet listener, or unauthenticated fallback.

Start the separate host paired-LAN launcher first:

```text
cargo run -p ksa64-session-broker --features launcher --bin ksa64-paired-lan -- \
  serve --bind 192.168.1.42:27864 --state-dir ./local-paired-lan \
  --session-nonce 4b53413600000001
```

On first pairing, the Vita displays a six-digit code after the host broker has received the XX handshake. Confirm that exact same code on the host and then press Cross on the Vita. The client saves only its device keypair and the authenticated host public key at `ux0:data/KSA64/vita-peer.vpi`; it never saves a session secret, action log, KSB11 evidence, or truth data. Replace `mode=pair` with `mode=reconnect` for later Noise IK connections. Deleting `vita-peer.vpi` intentionally forgets the peer and requires a new XX pairing.

The configured nonce is an explicit session parameter shared with the host broker. It is not a password, does not appear in canonical evidence, and should be changed for a new session.

## Host fixture

```powershell
cargo +stable run --manifest-path vita/Cargo.toml -p ksa64-vita-client --bin vita-fixture
cargo +stable test --manifest-path vita/Cargo.toml -p ksa64-vita-client
cargo +stable clippy --manifest-path vita/Cargo.toml -p ksa64-vita-client --all-targets --all-features -- -D warnings
```

The host fixture verifies strict KPS1 parsing, bounded retention, Review/Stage/Commit intent construction, explicit resynchronization, role enforcement, and the 64 MiB working-set budget.

## Building the VPK

The verified lane is WSL Ubuntu plus the user-local VitaSDK recorded in [toolchain-manifest.toml](toolchain-manifest.toml):

```powershell
./vita/build-vpk.ps1 -Profile release
```

The helper runs `cargo +nightly-2026-07-20 vita build vpk` for the `armv7-sony-vita-newlibeabihf` target with `--no-default-features --features vita-target --bin ksa64-vita --release`.

Output: `vita/target/armv7-sony-vita-newlibeabihf/release/ksa64-vita.vpk`. The VPK is ignored by Git and is an engineering artifact, not canonical mission evidence.

## Evidence boundary

A release VPK was successfully cross-compiled and structurally inspected as a 32-bit little-endian ARM EABI5 hard-float executable containing `sce_sys/param.sfo` and `eboot.bin`. That proves the Rust/SDL2/VitaSDK networking/Noise client links and packages.

It does **not** prove that the application starts or renders in Vita3K, nor does it prove physical Vita layout, controls, pairing, reconnect, suspend/resume, memory, or 30 fps behavior. Those gates remain explicitly pending; see [VITA3K_SMOKE.md](VITA3K_SMOKE.md).

## Architecture

```text
paired encrypted LAN or offline KPS1 replay
                     |
                     v
       ksa64-vita-client presentation model
                     |
        +------------+-------------+
        |                          |
 shared no_std Noise          SDL2 Vita UI
 XX/IK primitives             960x544 at 30 fps target
        |
        v
 Review / Stage / Commit / Cancel proposals only
```

The Vita statically links `ksa64-presentation` and the shared no-std Noise primitives. It does not load the desktop 64-bit bridge and does not contain the simulator.
