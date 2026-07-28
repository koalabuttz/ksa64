# Unreal source portability (Phase 12B.5)

Mission Foundry's Rust bridge is source-portable across three deliberately
separate Unreal lanes:

| Lane | Unreal platform directory | Rust bridge target | Status |
|---|---|---|---|
| Windows x64 | `Win64` | `x86_64-pc-windows-msvc` | Accepted Phase 12A/12B baseline |
| Linux x64 | `Linux` | `x86_64-unknown-linux-gnu` | Source/staging lane; package only on a qualified Linux Unreal host |
| macOS ARM64 | `Mac` | `aarch64-apple-darwin` | Source/staging lane; package only on a qualified macOS ARM64 Unreal host |

The frozen Phase 12B header remains under `ThirdParty/ViewerBridge`; Phase 12B.5
builds use the byte-identical ABI surface with portable export macros from
`ThirdParty/ViewerBridgePortable`. This preserves the accepted header hash while
allowing Linux and macOS compilation.

The bridge uses Unreal's platform-neutral dynamic-library loader and a local
portable SHA-256 implementation to verify a staged library before loading it.
There is no Windows BCrypt, `.dll`, or Win64-only runtime assumption in the
plugin source. The accepted archived Win64 ABI-v1 artifact remains readable;
new staged libraries use the portable `ksa64.viewer-bridge-artifact.v2`
manifest.

## Explicit staging

Run staging outside UnrealBuildTool. It never invokes Cargo itself:

```powershell
pwsh -File phase12/stage-bridge-portable.ps1 -Platform Win64
pwsh -File phase12/stage-bridge-portable.ps1 -Platform Linux
pwsh -File phase12/stage-bridge-portable.ps1 -Platform Mac
```

The script requires a clean source tree, a platform-capable Rust linker, and
exactly the selected platform target. It places one commit/build/platform
qualified library plus manifest beneath:

```text
Plugins/Ksa64Bridge/Binaries/Win64
Plugins/Ksa64Bridge/Binaries/Linux
Plugins/Ksa64Bridge/Binaries/Mac
```

`-VerifyOnly` is non-mutating and validates the manifest platform identity and
library SHA-256. A staged manifest cannot be substituted across platforms.

## Evidence boundary

Windows packaging remains accepted evidence. Linux and macOS targets are
source and staging support only until their respective qualified Unreal hosts
build, package, and run the automation suite. This change makes neither a
release claim and does not make Linux ARM64 Unreal a supported lane.

The Rust world, flight computer, role filtering, and KSB11 evidence remain
outside Unreal. The plugin only loads the versioned presentation bridge.