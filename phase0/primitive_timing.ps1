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
    "ksa64-phase0-rust-primitive-timed-c64"
)
$oscarArtifact = Join-Path $projectRoot (
    "phase0\candidates\oscar64\out\phase0-primitive-timed-oscar64.prg"
)

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}

function Measure-Candidate(
    [string]$artifact,
    [int]$candidateId,
    [string]$candidateName
) {
    $json = & python -B phase0/reference/vice_primitive_timing.py `
        --vice $vice `
        --prg $artifact `
        --candidate-id $candidateId `
        --candidate-name $candidateName `
        --runs $Runs `
        --timeout 120
    Assert-ExitCode "VICE primitive timing for $candidateName"
    $result = ($json -join "`n") | ConvertFrom-Json
    if (-not $result.stable) { throw "$candidateName primitive timing was not stable." }
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

    Write-Host "== Primitive-timed C64 builds =="
    & $rustWrapper -ReturnToCaller -WorkingDirectory "phase0/candidates/rust" `
        cargo build --release --target mos-c64-none --features c64 `
        -Z build-std=core `
        -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase0-rust-primitive-timed-c64
    Assert-ExitCode "Primitive-timed rust-mos C64 build"

    New-Item -ItemType Directory -Force -Path (Split-Path $oscarArtifact) | Out-Null
    & $oscarWrapper -ReturnToCaller `
        "-tm=c64" "-pp" "-O2" "-dKSA64_OSCAR64" `
        "-o=$oscarArtifact" `
        phase0/candidates/oscar64/primitive_timed_main.cpp `
        phase0/candidates/oscar64/arithmetic.cpp `
        phase0/candidates/oscar64/optimized.cpp `
        phase0/candidates/oscar64/c64_timer.cpp
    Assert-ExitCode "Primitive-timed Oscar64 C64 build"

    Write-Host ""
    Write-Host "== PAL VICE $($versions.vice.version), 512 calls per primitive =="
    $rust = Measure-Candidate $rustArtifact 1 "rust-mos"
    $oscar = Measure-Candidate $oscarArtifact 2 "oscar64"

    foreach ($field in @(
        "iterations", "multiply_accumulator", "divide_accumulator",
        "fraction_accumulator"
    )) {
        if ($rust.$field -ne $oscar.$field) {
            throw "Primitive result mismatch in $field."
        }
    }

    $rows = @(
        [pscustomobject]@{
            Primitive = "Scaled multiply"
            RustCycles = [long]$rust.multiply_cycles
            RustPerCall = "{0:N2}" -f [double]$rust.multiply_cycles_per_call
            OscarCycles = [long]$oscar.multiply_cycles
            OscarPerCall = "{0:N2}" -f [double]$oscar.multiply_cycles_per_call
        },
        [pscustomobject]@{
            Primitive = "General scaled divide"
            RustCycles = [long]$rust.divide_cycles
            RustPerCall = "{0:N2}" -f [double]$rust.divide_cycles_per_call
            OscarCycles = [long]$oscar.divide_cycles
            OscarPerCall = "{0:N2}" -f [double]$oscar.divide_cycles_per_call
        },
        [pscustomobject]@{
            Primitive = "Fast fraction divide"
            RustCycles = [long]$rust.fraction_cycles
            RustPerCall = "{0:N2}" -f [double]$rust.fraction_cycles_per_call
            OscarCycles = [long]$oscar.fraction_cycles
            OscarPerCall = "{0:N2}" -f [double]$oscar.fraction_cycles_per_call
        }
    )

    $rows | Format-Table -AutoSize
    Write-Host "Rust artifact:  $((Get-Item -LiteralPath $rustArtifact).Length) bytes"
    Write-Host "Oscar artifact: $((Get-Item -LiteralPath $oscarArtifact).Length) bytes"
    Write-Host "Each candidate was identical across $Runs run(s)."
    Write-Host "PHASE 0 PRIMITIVE TIMING: PASS"
} finally {
    Pop-Location
}
