# Phase 12B.5 Vita feasibility build helper.
# Requires the versions recorded in toolchain-manifest.toml. This script does
# not install tools and never contacts a network service.
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$clientManifest = Join-Path $root "client\Cargo.toml"

if (-not $env:VITASDK) {
    throw "VITASDK is required. Install the pinned VitaSDK before building."
}
if (-not (Get-Command cargo-vita -ErrorAction SilentlyContinue)) {
    throw "cargo-vita is required. Install the pinned tool recorded in toolchain-manifest.toml."
}

$arguments = @(
    "vita", "vpk",
    "--manifest-path", $clientManifest,
    "--bin", "ksa64-vita",
    "--target", "armv7-sony-vita-newlibeabihf"
)
if ($Profile -eq "release") {
    $arguments += "--release"
}

& cargo +nightly @arguments
if ($LASTEXITCODE -ne 0) {
    throw "cargo-vita failed with exit code $LASTEXITCODE"
}
