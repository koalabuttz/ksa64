# Vita engineering build evidence — 2026-07-28 UTC

This record is noncanonical packaging evidence. It does not satisfy Vita3K or physical-device acceptance.

- Git parent at build: `35894d3f4b0a46cf8de78f68af9ad026989d78ad` (working tree contained the Phase 12B.5 Vita implementation).
- Command: `./vita/build-vpk.ps1 -Profile release`.
- Target: `armv7-sony-vita-newlibeabihf`.
- VPK bytes: `1,150,950`.
- VPK SHA-256: `e9093c1c791480fec0f26ce5cada39122cf62822549957c0506308235f69cf90`.
- Archive members: `sce_sys/param.sfo`, `eboot.bin`.
- ELF: 32-bit little-endian ARM, EABI5 hard-float.
- ELF text/data/bss: `1,433,940 / 10,996 / 293,080` bytes.
- Host unit tests: 7 passed, including a complete shared Noise XX pairing/confirmation fixture plus a compile-tested opt-in VitaSDK socket runtime.
- Clippy: all targets/all features, warnings denied, passed.

The VPK was not launched in Vita3K or on a physical Vita in this environment. Runtime layout, input, physical network pairing, reconnect, suspend/resume, working set, and 30 fps remain pending. The VPK now contains the actual opt-in VitaSDK socket path, secure RNG, XX/IK protocol implementation, persistent peer identity, and bounded encrypted packet queues; only physical/emulator execution of that path remains pending.
