[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$rustCandidate = Join-Path $projectRoot "phase0\candidates\rust"
$oscarOutput = Join-Path $projectRoot "phase0\candidates\oscar64\out"
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$oscarWrapper = Join-Path $projectRoot "tools\toolchains\oscar64.ps1"

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) {
        throw "$label failed with exit code $LASTEXITCODE."
    }
}

function Build-RustKernel([string]$binary) {
    & $rustWrapper -ReturnToCaller -WorkingDirectory "phase0/candidates/rust" `
        cargo build --release --target mos-sim-none --features sim `
        -Z build-std=core `
        -Z build-std-features=compiler-builtins-mem `
        --bin $binary
    Assert-ExitCode "rust-mos build for $binary"
}

function Measure-RustKernel([string]$binary) {
    $output = & $rustWrapper -ReturnToCaller `
        -WorkingDirectory "phase0/candidates/rust" `
        sh -lc "mos-sim --cycles target/mos-sim-none/release/$binary" 2>&1
    $exitCode = $LASTEXITCODE
    $text = ($output | ForEach-Object { $_.ToString() }) -join "`n"
    Write-Host $text
    if ($exitCode -ne 0) {
        throw "rust-mos execution for $binary failed with exit code $exitCode."
    }
    $match = [regex]::Match($text, '(?m)^(\d+) cycles$')
    if (-not $match.Success) {
        throw "Could not parse rust-mos cycle count for $binary."
    }
    return [long]$match.Groups[1].Value
}

function Measure-OscarKernel(
    [string]$artifactName,
    [string[]]$sources
) {
    $artifact = Join-Path $oscarOutput $artifactName
    $output = & $oscarWrapper -ReturnToCaller `
        "-tm=c64" "-pp" "-O2" "-dKSA64_OSCAR64" "-ep" `
        "-o=$artifact" @sources 2>&1
    $exitCode = $LASTEXITCODE
    $text = ($output | ForEach-Object { $_.ToString() }) -join "`n"
    Write-Host $text
    if ($exitCode -ne 0) {
        throw "Oscar64 execution for $artifactName failed with exit code $exitCode."
    }
    $match = [regex]::Match($text, '(?m)^Total Cycles (\d+)$')
    if (-not $match.Success) {
        throw "Could not parse Oscar64 cycle count for $artifactName."
    }
    return [pscustomobject]@{
        Cycles = [long]$match.Groups[1].Value
        Artifact = $artifact
    }
}

Push-Location $projectRoot
try {
    New-Item -ItemType Directory -Force -Path $oscarOutput | Out-Null

    $rustBaselineBinary = "ksa64-phase0-rust-vertical-kernel-sim"
    $rustOptimizedBinary = "ksa64-phase0-rust-vertical-kernel-optimized-sim"

    Write-Host "== Rust dynamics kernels =="
    Build-RustKernel $rustBaselineBinary
    $rustBaselineCycles = Measure-RustKernel $rustBaselineBinary
    Build-RustKernel $rustOptimizedBinary
    $rustOptimizedCycles = Measure-RustKernel $rustOptimizedBinary

    Write-Host ""
    Write-Host "== Oscar64 dynamics kernels =="
    $oscarBaseline = Measure-OscarKernel `
        "phase0-vertical-kernel-oscar64.prg" `
        @(
            "phase0/candidates/oscar64/vertical_kernel_main.cpp",
            "phase0/candidates/oscar64/vertical.cpp",
            "phase0/candidates/oscar64/arithmetic.cpp"
        )
    $oscarOptimized = Measure-OscarKernel `
        "phase0-vertical-kernel-optimized-oscar64.prg" `
        @(
            "phase0/candidates/oscar64/vertical_kernel_optimized_main.cpp",
            "phase0/candidates/oscar64/vertical.cpp",
            "phase0/candidates/oscar64/arithmetic.cpp",
            "phase0/candidates/oscar64/optimized.cpp"
        )

    $rustBaselineArtifact = Join-Path `
        $rustCandidate "target\mos-sim-none\release\$rustBaselineBinary"
    $rustOptimizedArtifact = Join-Path `
        $rustCandidate "target\mos-sim-none\release\$rustOptimizedBinary"

    $rows = @(
        [pscustomobject]@{
            Candidate = "Rust baseline"
            Cycles = $rustBaselineCycles
            Bytes = (Get-Item -LiteralPath $rustBaselineArtifact).Length
            Reduction = "-"
        },
        [pscustomobject]@{
            Candidate = "Rust optimized"
            Cycles = $rustOptimizedCycles
            Bytes = (Get-Item -LiteralPath $rustOptimizedArtifact).Length
            Reduction = "{0:N2}%" -f `
                (100.0 * ($rustBaselineCycles - $rustOptimizedCycles) / $rustBaselineCycles)
        },
        [pscustomobject]@{
            Candidate = "Oscar64 baseline"
            Cycles = $oscarBaseline.Cycles
            Bytes = (Get-Item -LiteralPath $oscarBaseline.Artifact).Length
            Reduction = "-"
        },
        [pscustomobject]@{
            Candidate = "Oscar64 optimized"
            Cycles = $oscarOptimized.Cycles
            Bytes = (Get-Item -LiteralPath $oscarOptimized.Artifact).Length
            Reduction = "{0:N2}%" -f `
                (100.0 * ($oscarBaseline.Cycles - $oscarOptimized.Cycles) / $oscarBaseline.Cycles)
        }
    )

    Write-Host ""
    Write-Host "== Dynamics-only summary =="
    $rows | Format-Table -AutoSize
    Write-Host "Cycle reductions are comparable within each toolchain/emulator pair."
    Write-Host "Cross-toolchain cycle totals remain preliminary until one common timer is used."
} finally {
    Pop-Location
}
