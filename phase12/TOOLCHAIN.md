# Phase 12A native Windows toolchain

Status: installation and lock procedure. Populate a repository-local copy of
`toolchain-lock.example.toml` only after verifying every value.

The lock is noncanonical environment metadata. It does not change any mission,
catalog, evidence, physics, or target identity.

## Required layout

| Purpose | Locked path |
|---|---|
| Phase 12 checkout | `C:\dev\KSA64` |
| Unreal Engine 5.8 Launcher install | Exact resolved path under `E:` |
| Unreal Derived Data Cache | Exact resolved path under `E:` |
| Unreal project | `C:\dev\KSA64\foundry\Ksa64MissionFoundry` |

Do not move the Codex-managed project mirror. Create a new short-path checkout
from verified `origin/main`. Do not keep simultaneous writers active in the
mirror and short-path checkout.

## Observed Phase 12A installation

The verified Launcher build is UE 5.8.0, changelist 55116800. The user-selected Launcher path resolved to `D:\Games\UE_5.8`, rather than the planned E: engine volume. The exact path is recorded and accepted as a non-authority-affecting toolchain deviation; the Derived Data Cache remains on `E:\Unreal\DDC`.

This Launcher installation exposes **Templates and Feature Packs** but not a separate **Starter Content** component. The installed feature-pack set contains no `StarterContent.upack`. Phase 12A therefore keeps the accepted empty C++ shell instead of acquiring unrelated assets through an unpinned source. Starter Content is presentation-only and is not required by any 12A acceptance criterion.

After changing `UE-LocalDataCachePath`, restart Unreal Editor and every process that launches it, including Epic Launcher and Visual Studio. Automated gates also set the variable explicitly and verify the resolved path in the Editor log.

## Required components

- Fully updated Windows 11 and a current stable GPU driver.
- Current stable Git for Windows and Git LFS.
- Rustup MSVC host toolchain used by the workspace.
- Epic Games Launcher and the current UE 5.8 Launcher patch.
- Visual Studio 2026:
  - Game Development with C++;
  - Desktop Development with C++;
  - Visual Studio Tools for Unreal Engine where offered;
  - C++ profiling tools and AddressSanitizer;
  - the compiler toolset and Windows SDK supported by the installed UE patch.

Epic's 5.8 release notes list Visual Studio 2026 as recommended, MSVC 14.50 as
recommended, and Windows SDK 10.0.26100.0 as default. The installed engine's
own supported-toolchain output is the final compatibility check; record actual
resolved versions rather than copying those examples blindly.

## Verification procedure

1. Confirm local and remote baseline commit `8208d53`.
2. Confirm Git LFS is installed before checking out governed binary assets.
3. Record OS build, CPU, physical RAM, GPU, VRAM, and driver.
4. Record `rustc`, Cargo, Git, Git LFS, Visual Studio, MSVC, and SDK identities.
5. Record the Unreal semantic version, build/version files, Launcher item
   identity, installation path, enabled plugin versions, and Derived Data Cache
   path.
6. Record the checkout path, clean Git state, baseline commit, and remote URL.
7. Build the clean Editor target, then record command, duration, result, and log
   path.
8. Hash the completed lock file and include it in the 12A completion record.

Never record Epic, GitHub, Microsoft, or MCP credentials, access tokens,
machine secrets, or user-specific authentication files.

## MCP lock and safety

Record the `ModelContextProtocol`, Toolset Registry, and AllToolsets plugin
identities. Bind the Unreal MCP server only to loopback. The documented default
is `http://127.0.0.1:8000/mcp`; there is no authentication layer, editor tool
calls execute serially on the game thread, and interfaces may change while the
feature is experimental.

Generate/review the Codex configuration from the workspace root. Do not commit
machine-local MCP configuration unless it is explicitly sanitized and intended
as a template.

## Upgrade rule

Never upgrade the only project copy in place. For an engine patch:

1. begin from clean pushed state;
2. use an upgrade branch and open-as-copy when requested;
3. regenerate project files and rebuild;
4. run native/bridge tests, Unreal automation, cook, and packaged smoke;
5. inspect every resaved asset and LFS object;
6. replace the lock only with a reviewed decision and passing evidence.

## References

- [UE 5.8 hardware and software specifications](https://dev.epicgames.com/documentation/unreal-engine/hardware-and-software-specifications-for-unreal-engine)
- [UE 5.8 release notes](https://dev.epicgames.com/documentation/unreal-engine/unreal-engine-5-8-release-notes)
- [Unreal MCP documentation](https://dev.epicgames.com/documentation/unreal-engine/unreal-mcp-in-unreal-editor)