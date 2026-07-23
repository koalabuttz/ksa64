[CmdletBinding()]
param([int]$Runs = 3)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$rust = Join-Path $root "tools\toolchains\rust-mos.ps1"
$versions = Get-Content -Raw -LiteralPath (Join-Path $root "toolchains\versions.json") | ConvertFrom-Json
$vice = Join-Path $root $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $root "target\mos-c64-none\release\ksa64-phase2-vacuum-timed-c64"
$acceptedPath = Join-Path $root "phase2\vacuum-timing-v2.json"

Push-Location $root
try {
    & $rust -WorkingDirectory . cargo build --release --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase2-vacuum-timed-c64
    if ($LASTEXITCODE -ne 0) { throw "Phase 2 vacuum timing build failed." }
    $json = & python -B phase2/reference/vice_vacuum_timing.py --vice $vice `
        --prg $artifact --runs $Runs
    if ($LASTEXITCODE -ne 0) { throw "Phase 2 vacuum timing failed." }
    $payload = ($json -join "`n") | ConvertFrom-Json
    $accepted = Get-Content -Raw -LiteralPath $acceptedPath | ConvertFrom-Json
    $result = $payload.runs[0]
    $artifactBytes = (Get-Item -LiteralPath $artifact).Length
    if (-not $payload.stable `
        -or [long]$result.semi_implicit_net_cycles -ne [long]$accepted.semi_implicit.net_cycles `
        -or [long]$result.midpoint_net_cycles -ne [long]$accepted.midpoint.net_cycles `
        -or [long]$result.boundary_overhead -ne [long]$accepted.boundary_overhead_cycles `
        -or [long]$result.semi_final[0] -ne [long]$accepted.semi_implicit.final[0] `
        -or [long]$result.semi_final[1] -ne [long]$accepted.semi_implicit.final[1] `
        -or [long]$result.midpoint_final[0] -ne [long]$accepted.midpoint.final[0] `
        -or [long]$result.midpoint_final[1] -ne [long]$accepted.midpoint.final[1] `
        -or $artifactBytes -ne [long]$accepted.timed_prg_bytes) {
        throw "Phase 2 vacuum timing differs from vacuum-timing-v2.json."
    }
    $rows = @(
        [pscustomobject]@{
            Integrator = "Semi-implicit Euler"
            CyclesPerStep = "{0:N3}" -f [double]$result.semi_implicit_cycles_per_step
        },
        [pscustomobject]@{
            Integrator = "Midpoint RK2"
            CyclesPerStep = "{0:N3}" -f [double]$result.midpoint_cycles_per_step
        }
    )
    $rows | Format-Table -AutoSize
    Write-Host "Terminal state: radius=$($result.semi_final[0]), radial_velocity=$($result.semi_final[1])"
    Write-Host "Timed PRG: $artifactBytes bytes"
    Write-Host "PHASE 2 VACUUM TIMING: PASS"
} finally { Pop-Location }
