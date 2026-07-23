[CmdletBinding()]
param()
$ErrorActionPreference="Stop"
$root=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions=Get-Content -Raw -LiteralPath (Join-Path $root "toolchains/versions.json")|ConvertFrom-Json
$rust=Join-Path $root "tools/toolchains/rust-mos.ps1"
$vice=Join-Path $root $versions.vice.projectRelativeExecutable.Replace("/","\")
Push-Location $root
try {
 & $rust -ReturnToCaller -WorkingDirectory "." cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase5-history-reu-c64
 if($LASTEXITCODE-ne 0){throw "Phase 5 REU probe build failed"}
 $prg="target/mos-c64-none/c64/ksa64-phase5-history-reu-c64"
 python -B phase5/reference/vice_history_reu.py --vice $vice --prg $prg --output target/phase5-history-reu-matrix-v1.json
 if($LASTEXITCODE-ne 0){throw "Phase 5 REU matrix failed"}
 $actual=[IO.File]::ReadAllBytes((Join-Path $root "target/phase5-history-reu-matrix-v1.json"))
 $expected=[IO.File]::ReadAllBytes((Join-Path $root "phase5/history-reu-matrix-v1.json"))
 if(-not [Linq.Enumerable]::SequenceEqual[byte]($actual,$expected)){throw "Phase 5 REU matrix differs from frozen evidence"}
 Write-Host "PHASE 5 HISTORY REU MATRIX: PASS"
} finally {Pop-Location}
