# Phase 0 toolchains

KSA64 pins both candidate C64 compilers before the benchmark is implemented.

## Pinned versions

### rust-mos

The Rust candidate uses David's fork image, matching the image pinned by koalabuttz/roguelike:

    ghcr.io/koalabuttz/rust-mos:ac2fb2277-4537158-4aaa40e16

Resolved repository digest:

    sha256:9ddd4ce9502e54409fb56e0a80a2200f1df693a629f2911ff4f1c81303e51271

Verified contents:

- rustc 1.94.0-dev
- cargo 1.94.0-dev
- LLVM 23.0.0
- Clang 23.0.0

Docker Desktop is the only host prerequisite. The compiler does not need a host rustup installation.

Pull the pinned image:

    docker pull ghcr.io/koalabuttz/rust-mos:ac2fb2277-4537158-4aaa40e16

Run a command inside it:

    powershell -File tools/toolchains/rust-mos.ps1 rustc --version --verbose

Run from a project subdirectory:

    powershell -File tools/toolchains/rust-mos.ps1 -WorkingDirectory toolchains/smoke/rust-mos cargo build --release

The wrapper mounts the repository at /workspace, uses the pinned image, supplies the fork's toolchain path, and keeps Cargo's package cache under the ignored `.toolchains/cache/rust-mos-cargo` directory. The cache survives disposable containers but is never a source or evidence input.

### Oscar64

Oscar64 is pinned to the official v1.32.272 release:

- Release archive SHA-256: 36b8ea7bedd79c751117cb6ae0a199037370c36fd29ceda38b08aafa43441fd4
- Installed compiler SHA-256: 718294db4008e00fcd9f729ac5479f96a8768de21393f89f9e2efa9a6e5134a9
- Compiler file version: 1.32.272.0

The upstream manual documents the Windows installer and building from the source repository. The official release also provides a portable archive. KSA64 uses that verified archive under the ignored .toolchains directory so the project does not depend on administrator access or a mutable system PATH.

The expected local compiler location is:

    .toolchains/oscar64/v1.32.272/oscar64/bin/oscar64.exe

The wrapper checks locations in this order:

1. The KSA64_OSCAR64 environment variable.
2. The pinned project-local installation.
3. Current documented and historical Program Files locations.
4. An oscar64 command already available on PATH.

Run it:

    powershell -File tools/toolchains/oscar64.ps1 -h

Compile a C++ source:

    powershell -File tools/toolchains/oscar64.ps1 -tm=c64 -O2 -o=output.prg source.cpp

Oscar64 recognizes the cpp extension as C++ mode. The upstream manual also permits the -pp option.

The v1.32.272 smoke test confirmed that Oscar64 does not accept every standard C++ spelling: explicit and reinterpret_cast were rejected, while constructors, constexpr, static_assert, and zero-storage wrapper structs compiled. Phase 0 must target the tested Oscar64 subset rather than assuming desktop-compiler syntax.

### VICE

Common target timing uses the official portable Windows GTK3 build of VICE 3.10 and its cycle-accurate `x64sc` emulator. The release archive and executable are pinned by SHA-256 in `versions.json`.

Install it under the ignored `.toolchains` directory:

    powershell -File tools/toolchains/setup-vice.ps1

The expected executable is:

    .toolchains/vice/3.10/GTK3VICE-3.10-win64/bin/x64sc.exe

Phase 0 launches it hidden in PAL warp mode, reads the target-visible CIA timing result through VICE's binary monitor, and runs candidates sequentially so emulator instances do not contend for shared UI state.

## Verify everything

From the repository root:

    powershell -File tools/toolchains/verify.ps1

The verification script:

1. Confirms the local Docker image resolves to the pinned repository digest.
2. Reports the Rust, Cargo, LLVM, and Clang versions inside the image.
3. Builds and links the minimal rust-mos C64 smoke fixture.
4. Verifies the Oscar64 compiler hash and file version.
5. Builds and links the minimal Oscar64 C++ C64 smoke fixture.
6. Verifies the pinned VICE executable hash.
7. Reports the resulting PRG paths, sizes, and SHA-256 hashes.

Smoke outputs are ignored. The fixtures are toolchain checks, not simulator code.

## Updating a pin

Treat toolchain updates as benchmark changes:

1. Change one toolchain at a time.
2. Verify the upstream release or image provenance.
3. Record the immutable digest or archive checksum.
4. Update versions.json.
5. Run verify.ps1.
6. Record the compiler versions with Phase 0 results.
7. Rerun all candidate benchmarks before comparing with older numbers.

Never replace the rust-mos tag with latest, and never allow Oscar64 discovery through PATH to silently override the project pin.

## Upstream documentation

- Oscar64 repository: https://github.com/drmortalwombat/oscar64
- Oscar64 reference manual: https://github.com/drmortalwombat/oscar64/blob/main/oscar64.md
- Oscar64 v1.32.272 release: https://github.com/drmortalwombat/oscar64/releases/tag/v1.32.272
- rust-mos fork consumer and image pin: https://github.com/koalabuttz/roguelike
- VICE homepage and current release: https://vice-emu.sourceforge.io/
- VICE binary monitor protocol: https://vice-emu.sourceforge.io/vice_13.html

