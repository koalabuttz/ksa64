[CmdletBinding()]
param(
    [ValidateRange(1, 20)]
    [int]$Runs = 3
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (
    Join-Path $projectRoot "toolchains\versions.json"
) | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$oscarWrapper = Join-Path $projectRoot "tools\toolchains\oscar64.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$rustArtifact = Join-Path $projectRoot (
    "phase0\candidates\rust\target\mos-c64-none\release\" +
    "ksa64-phase0-rust-vertical-timed-c64"
)
$oscarArtifact = Join-Path $projectRoot (
    "phase0\candidates\oscar64\out\phase0-vertical-timed-oscar64.prg"
)

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}

function Measure-Candidate(
    [string]$artifact,
    [int]$candidateId,
    [string]$candidateName
) {
    $json = & python -B phase0/reference/vice_timing.py `
        --vice $vice `
        --prg $artifact `
        --candidate-id $candidateId `
        --candidate-name $candidateName `
        --runs $Runs `
        --timeout 120
    Assert-ExitCode "VICE timing for $candidateName"
    $result = ($json -join "`n") | ConvertFrom-Json
    if (-not $result.stable) { throw "$candidateName timing was not stable." }
    return $result.runs[0]
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

    Write-Host "== Timed C64 builds =="
    & $rustWrapper -ReturnToCaller -WorkingDirectory "phase0/candidates/rust" `
        cargo build --release --target mos-c64-none --features c64 `
        -Z build-std=core `
        -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase0-rust-vertical-timed-c64
    Assert-ExitCode "Timed rust-mos C64 build"

    New-Item -ItemType Directory -Force -Path (Split-Path $oscarArtifact) | Out-Null
    & $oscarWrapper -ReturnToCaller `
        "-tm=c64" "-pp" "-O2" "-dKSA64_OSCAR64" `
        "-o=$oscarArtifact" `
        phase0/candidates/oscar64/vertical_timed_main.cpp `
        phase0/candidates/oscar64/vertical.cpp `
        phase0/candidates/oscar64/arithmetic.cpp `
        phase0/candidates/oscar64/optimized.cpp `
        phase0/candidates/oscar64/c64_timer.cpp
    Assert-ExitCode "Timed Oscar64 C64 build"

    Write-Host ""
    Write-Host "== PAL VICE $($versions.vice.version), CIA1 32-bit timer =="
    $rust = Measure-Candidate $rustArtifact 1 "rust-mos"
    $oscar = Measure-Candidate $oscarArtifact 2 "oscar64"

    foreach ($field in @(
        "altitude_q12", "velocity_q24", "acceleration_q28",
        "mass_q12", "propellant_q12", "cutoff_events"
    )) {
        if ($rust.$field -ne $oscar.$field) {
            throw "Timed final-state mismatch in $field."
        }
    }

    $rows = @(
        [pscustomobject]@{
            Candidate = "rust-mos"
            Cycles = [long]$rust.net_cycles
            CyclesPerStep = "{0:N2}" -f [double]$rust.cycles_per_step
            Boundary = [long]$rust.boundary_overhead_cycles
            Bytes = (Get-Item -LiteralPath $rustArtifact).Length
        },
        [pscustomobject]@{
            Candidate = "Oscar64"
            Cycles = [long]$oscar.net_cycles
            CyclesPerStep = "{0:N2}" -f [double]$oscar.cycles_per_step
            Boundary = [long]$oscar.boundary_overhead_cycles
            Bytes = (Get-Item -LiteralPath $oscarArtifact).Length
        }
    )

    $rows | Format-Table -AutoSize
    $cycleReduction = 100.0 * ($oscar.net_cycles - $rust.net_cycles) / $oscar.net_cycles
    Write-Host ("Rust uses {0:N2}% fewer cycles than Oscar64." -f $cycleReduction)
    Write-Host "Each candidate was identical across $Runs run(s)."
    Write-Host "PHASE 0 COMMON TIMING GATE: PASS"
} finally {
    Pop-Location
}
