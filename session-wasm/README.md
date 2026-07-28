# ksa64-session-wasm

A deliberately small browser-worker adapter over the accepted
`ksa64-session::FullMissionPresentationSession`. It owns no simulation model,
filesystem, wall clock, or threads. The worker accepts fixed `KSW1` commands
and returns bounded `KSR1` results; publication payloads are strict
role-filtered `KPS1` records.

The ABI is intentionally not canonical evidence. Completed evidence is obtained
as an opaque KSB11 byte stream from the already-accepted session finalizer.

```text
KSW1 command -> FullMissionPresentationSession -> KPS1 publications / KSB11
```

Run the native facade tests:

```powershell
cargo +stable test --manifest-path session-wasm/Cargo.toml
```

After building `wasm32-unknown-unknown`, run the worker ABI harness:

```powershell
node session-wasm/tools/harness.mjs session-wasm/target/wasm32-unknown-unknown/release/ksa64_session_wasm.wasm
```

A panic or terminated worker can only leave an incomplete session. It never
returns fabricated completed evidence.

The PWA runs `npm run build:wasm` to build this crate with the root lockfile and copy the raw module to `web/public/wasm/`. The module is intentionally loaded without wasm-bindgen.
