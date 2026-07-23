[CmdletBinding()]
param([int]$Runs = 3)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$rust = Join-Path $root "tools\toolchains\rust-mos.ps1"
$versions = Get-Content -Raw -LiteralPath (Join-Path $root "toolchains\versions.json") | ConvertFrom-Json
$vice = Join-Path $root $versions.vice.projectRelativeExecutable.Replace("/", "\")

Push-Location $root
try {
    & $rust -WorkingDirectory . cargo build --release --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase2-vacuum-timed-c64
    if ($LASTEXITCODE -ne 0) { throw "Phase 2 vacuum timing build failed." }
    & python -B phase2/reference/vice_vacuum_timing.py --vice $vice `
        --prg target/mos-c64-none/release/ksa64-phase2-vacuum-timed-c64 --runs $Runs
    if ($LASTEXITCODE -ne 0) { throw "Phase 2 vacuum timing failed." }
} finally { Pop-Location }
