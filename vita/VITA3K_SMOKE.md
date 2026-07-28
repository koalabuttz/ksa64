# Vita3K smoke and physical-device acceptance

The [fixtures/vita3k-input.json](fixtures/vita3k-input.json) sequence is operational rather than visual-only. It exercises page navigation, high-level actions, cancellation, connection loss, reconnect, and resynchronization. It does **not** replace physical Vita acceptance.

## Current verified boundary

The release VPK cross-compiles and packages successfully. No Vita3K executable is installed in this environment and no physical Vita was connected, so neither gate is claimed.

## Vita3K smoke procedure

1. Build with `./vita/build-vpk.ps1 -Profile release`.
2. Install the generated `ksa64-vita.vpk` in a clean Vita3K profile.
3. Launch the default offline, role-filtered GNSS-loss replay.
4. Execute the input fixture at the native 960x544 resolution.
5. Capture every page, log output, startup time, frame pacing, and emulator-reported memory.
6. Verify that no private truth or direct actuator action exists.
7. Force a retention gap and verify that the client visibly requires resynchronization.
8. Close the title and verify a clean return to the Vita shell.

## Physical Vita acceptance

Pair with the native broker over an explicitly selected LAN interface. Create the opt-in `ux0:data/KSA64/vita-lan.conf` file in `mode=pair`, using the broker session nonce. Compare and locally confirm the six-digit Noise XX code on both devices, then restart in `mode=reconnect` to test Noise IK:

- observer and guided-operator role binding;
- Review -> Stage -> Commit and Cancel through the proposal identity;
- encrypted KPS1 integrity, Vita secure-RNG availability, persisted host-key identity, and immutable role assignment;
- disconnect while remote authority continues;
- reconnect with retained cursors;
- retention gap -> explicit resynchronization;
- suspend/resume and network reacquisition;
- offline replay;
- 30 fps frame pacing and a working set below 64 MiB.

A Vita3K pass is only repeatable emulator evidence. The physical device remains authoritative for controls, pairing, networking, suspend/resume, memory, and timing.
