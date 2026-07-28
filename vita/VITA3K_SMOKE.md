# Vita3K smoke and physical-device acceptance

The `fixtures/vita3k-input.json` sequence is intentionally operational rather
than visual-only. It exercises navigation pages, an offered action, cancellation,
connection loss, reconnect, and resynchronization. It does **not** replace
physical Vita acceptance.

## Vita3K smoke

1. Build the VPK with `./vita/build-vpk.ps1`.
2. Install the generated VPK in a clean Vita3K profile.
3. Load an offline role-filtered KPS1 replay fixture.
4. Execute the input fixture at 960x544.
5. Capture screenshots, log output, and memory use.
6. Verify that no direct actuator control exists and a resync gap is visible.

## Physical Vita acceptance

Pair with the native broker over a user-selected LAN interface. Compare the
locally displayed pairing code on both devices, then exercise:

- observer and guided-operator role binding;
- Review -> Stage -> Commit action flow;
- disconnect while the remote authority continues;
- reconnect with retained cursors;
- forced retention gap -> explicit resync;
- suspend/resume;
- offline replay;
- 30 fps and working-set measurement.

A successful Vita3K run is only a smoke result. A physical device is required
for controls, pairing, networking, suspend/resume, memory, and timing claims.
