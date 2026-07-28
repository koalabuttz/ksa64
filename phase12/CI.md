# Phase 12B.5 continuous integration

Status: implemented; post-push hosted qualification pending.

The repository pins native and WebAssembly development to Rust 1.93 through
the root `rust-toolchain.toml`. The custom rust-mos Docker image and pinned
Vita nightly remain separate toolchains; the root pin does not replace them.

## Hosted workflows

`phase12b5-fast.yml` runs for pull requests, pushes to `main`, and manual
dispatch. Its native matrix is:

| Lane | GitHub runner | Expected Rust host |
|---|---|---|
| Windows x64 | `windows-2025-vs2026` | `x86_64-pc-windows-msvc` |
| Linux x64 | `ubuntu-24.04` | `x86_64-unknown-linux-gnu` |
| Linux ARM64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` |
| macOS ARM64 | `macos-15` | `aarch64-apple-darwin` |

Every job asserts its Rust host before compiling. The fast lane runs
formatting, warnings-denied Clippy, workspace tests, and the bridge library's
contained-panic tests. The web job requires the committed `web/package-lock.json`, then unconditionally runs
`npm ci`, tests, and a production build. A missing lockfile is a failing gate, never
a skipped web-product check.

`phase12b5-acceptance.yml` runs on `main` and manual dispatch. It reproduces
the complete four-action GNSS-loss reference mission on every native runner
and invokes the frozen assertions for release 21,591, the 2,911,464-byte
KSB11, its accepted SHA-256, the detailed products, and the disposition axes.
It also proves that inactive operations preserve the accepted Phase 10
mission.

The WebAssembly acceptance job builds the real Rust Worker authority and reproduces
the same 2,911,464-byte KSB11 and accepted SHA-256. It is not replaced by a
TypeScript mock.

## Engineering archives

The platform matrix builds a checksum-qualified archive with the native script for
its own operating system: `package-native.ps1` on Windows and
`package-native.sh` on Linux/macOS. Both scripts build with Cargo lockfile
enforcement and derive manifest-v2 public structure sizes by executing the Rust
bridge-manifest binary; no shell copy of ABI sizes exists.

Qualified archives require a clean source tree. The explicit `-AllowDirty` or
`--allow-dirty` escape hatch produces a separately named
`-unqualified-local` archive and stamps that status in its README, so it cannot
be mistaken for CI qualification.

## What hosted CI does not claim

Hosted CI does not claim:

- Unreal Editor or packaged-renderer acceptance;
- physical Lenovo Duet behavior;
- physical Vita controls, networking, suspend, memory, or timing;
- C64 or VICE acceptance;
- rust-mos or Vita nightly reproducibility; or
- application signing, notarization, installer, or store readiness.

Those gates retain their qualified hosts, pinned toolchains, and physical
devices. Their reports bind the source commit and accepted hashes.

Hosted jobs do not fetch Git LFS presentation assets because no current Rust,
protocol, or compact-web gate consumes them. Later renderer jobs must opt into
LFS explicitly.

The current hosted-runner labels and architectures are recorded from the official
[GitHub runner-images matrix](https://github.com/actions/runner-images).
Private-repository jobs consume the account's Actions allowance.
