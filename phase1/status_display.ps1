[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (
    Join-Path $projectRoot "toolchains\versions.json"
) | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot (
    "target\mos-c64-none\release\ksa64-phase1-telemetry-status-c64"
)

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}

Push-Location $projectRoot
try {
    if (-not (Test-Path -LiteralPath $vice -PathType Leaf)) {
        throw "Pinned VICE is missing. Run tools/toolchains/setup-vice.ps1."
    }
    $viceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $vice).Hash.ToLowerInvariant()
    if ($viceHash -ne $versions.vice.executableSha256) {
        throw "VICE executable hash does not match the pinned release."
    }

    Write-Host "== Phase 1 C64 status-display build =="
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --release --target mos-c64-none --features c64 `
        -Z build-std=core `
        -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase1-telemetry-status-c64
    Assert-ExitCode "C64 status-display build"

    Write-Host ""
    Write-Host "== PAL VICE screen-memory verification =="
    $json = & python -B phase1/reference/vice_status_display.py `
        --vice $vice `
        --prg $artifact `
        --timeout 180
    Assert-ExitCode "C64 status-display verification"
    $result = ($json -join "`n") | ConvertFrom-Json
    @(
        $result.title,
        $result.mission_time,
        $result.altitude,
        $result.velocity,
        $result.acceleration,
        $result.mass,
        $result.propellant,
        $result.step,
        $result.frames,
        $result.stride,
        $result.checksum,
        $result.events,
        $result.raw_rate,
        $result.recorded_rate,
        $result.timing_note
    ) | ForEach-Object { Write-Host $_ }
    Write-Host "Status PRG: $((Get-Item -LiteralPath $artifact).Length) bytes"
    Write-Host "PHASE 1 C64 STATUS DISPLAY: PASS"
} finally {
    Pop-Location
}
