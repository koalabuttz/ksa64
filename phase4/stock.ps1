[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$artifact = Join-Path $projectRoot "target\mos-c64-none\c64\ksa64-phase4-stock-c64"
function Assert-ExitCode([string]$label) { if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." } }
Push-Location $projectRoot
try {
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --profile c64 --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase4-stock-c64
    Assert-ExitCode "Phase 4 stock build"
    $json = & python -B phase4/reference/vice_stock.py --vice $vice --prg $artifact --timeout 180
    Assert-ExitCode "Phase 4 stock validation"
    $result = ($json -join "`n") | ConvertFrom-Json
    $bytes = (Get-Item -LiteralPath $artifact).Length
    $image = [IO.File]::ReadAllBytes($artifact)
    $load = [BitConverter]::ToUInt16($image, 0)
    $end = $load + $bytes - 2
    if (-not $result.passed -or $end -gt 0xC000) { throw "Stock-RAM image exceeds the below-C000 gate." }
    $result.titles | ForEach-Object { Write-Host $_ }
    Write-Host $result.campaign
    Write-Host $result.retained
    Write-Host $result.storage
    Write-Host "Stock PRG: $bytes bytes, load end 0x$($end.ToString('X4'))"
    Write-Host "PHASE 4 STOCK STORAGE: PASS"
} finally { Pop-Location }