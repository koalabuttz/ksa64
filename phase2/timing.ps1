[CmdletBinding()]
param(
    [ValidateRange(1, 20)]
    [int]$Runs = 3
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot "target\mos-c64-none\c64\ksa64-phase2-timed-c64"
$acceptedPath = Join-Path $projectRoot "phase2\timing-v1.json"

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
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --profile c64 --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase2-timed-c64
    Assert-ExitCode "Phase 2 C64 timing build"

    $json = & python -B phase2/reference/vice_timing.py `
        --vice $vice --prg $artifact --runs $Runs --timeout 300
    Assert-ExitCode "Phase 2 PAL VICE timing"
    $payload = ($json -join "`n") | ConvertFrom-Json
    if (-not $payload.stable) { throw "Phase 2 target timing was not stable." }
    $result = $payload.runs[0]
    $accepted = Get-Content -Raw -LiteralPath $acceptedPath | ConvertFrom-Json
    $artifactBytes = (Get-Item -LiteralPath $artifact).Length
    if ([long]$result.raw_net_cycles -ne [long]$accepted.raw.net_cycles `
        -or [long]$result.recorded_net_cycles -ne [long]$accepted.recorded.net_cycles `
        -or [long]$result.boundary_overhead_cycles -ne [long]$accepted.boundary_overhead_cycles `
        -or [long]$result.checksum -ne [Convert]::ToUInt32($accepted.result.state_checksum.Substring(2), 16) `
        -or [long]$result.final_frame_crc32 -ne [Convert]::ToUInt32($accepted.result.final_frame_crc32.Substring(2), 16) `
        -or $artifactBytes -ne [long]$accepted.timed_prg_bytes) {
        throw "Phase 2 timing differs from timing-v1.json."
    }
    $rows = @(
        [pscustomobject]@{
            Path = "Raw mission"
            CyclesPerStep = "{0:N3}" -f [double]$result.raw_cycles_per_step
            StepsPerSecond = "{0:N3}" -f (985248.0 / [double]$result.raw_cycles_per_step)
        },
        [pscustomobject]@{
            Path = "Checksummed KST2"
            CyclesPerStep = "{0:N3}" -f [double]$result.recorded_cycles_per_step
            StepsPerSecond = "{0:N3}" -f (985248.0 / [double]$result.recorded_cycles_per_step)
        }
    )
    $rows | Format-Table -AutoSize
    Write-Host "Recorded delta: $($result.recorded_overhead_cycles) cycles across $($result.step) steps."
    Write-Host "Frames: $($result.frames_written), bytes: $($result.bytes_written)"
    Write-Host "Timed PRG: $artifactBytes bytes"
    Write-Host "No minimum rate is an acceptance condition for Phase 2."
    Write-Host "PHASE 2 C64 TIMING: PASS"
} finally {
    Pop-Location
}
