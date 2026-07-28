# Phase 12B.5 Vita VPK build helper.
# Uses the pinned WSL VitaSDK lane recorded in toolchain-manifest.toml.
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [string]$Distro = "Ubuntu",
    [string]$VitaSdk = "/home/david/.local/vitasdk"
)

$ErrorActionPreference = "Stop"
$vitaWindows = (Resolve-Path -LiteralPath $PSScriptRoot).Path
if ($vitaWindows -notmatch '^([A-Za-z]):\\(.*)$') {
    throw "The Vita workspace must be on a Windows drive visible to WSL."
}
$drive = $Matches[1].ToLowerInvariant()
$tail = $Matches[2].Replace("\", "/")
$vitaWsl = "/mnt/$drive/$tail"
$profileArgs = if ($Profile -eq "release") { "--release" } else { "" }

$command = @"
export PATH=/home/david/.cargo/bin:/usr/bin:/bin
export VITASDK='$VitaSdk'
cd '$vitaWsl'
test -x '$VitaSdk/bin/arm-vita-eabi-gcc'
cargo +nightly-2026-07-20 vita build vpk -- --manifest-path client/Cargo.toml --no-default-features --features vita-target --bin ksa64-vita $profileArgs
"@

& wsl -d $Distro -- bash -lc $command
if ($LASTEXITCODE -ne 0) {
    throw "cargo-vita failed with exit code $LASTEXITCODE"
}

$profileDir = if ($Profile -eq "release") { "release" } else { "debug" }
$vpk = Join-Path $vitaWindows "target\armv7-sony-vita-newlibeabihf\$profileDir\ksa64-vita.vpk"
if (-not (Test-Path -LiteralPath $vpk)) {
    throw "Expected VPK was not produced: $vpk"
}
$artifact = Get-Item -LiteralPath $vpk
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $vpk).Hash.ToLowerInvariant()
Write-Host "VPK: $($artifact.FullName)"
Write-Host "Bytes: $($artifact.Length)"
Write-Host "SHA-256: $hash"
Write-Host "Packaging evidence only; Vita3K and physical Vita acceptance remain pending."
