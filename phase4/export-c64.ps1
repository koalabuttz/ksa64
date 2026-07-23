[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$versions = Get-Content -Raw -LiteralPath (Join-Path $projectRoot "toolchains\versions.json") | ConvertFrom-Json
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$vice = Join-Path $projectRoot $versions.vice.projectRelativeExecutable.Replace("/", "\")
$c1541 = Join-Path (Split-Path -Parent $vice) "c1541.exe"
$artifact = Join-Path $projectRoot "target\mos-c64-none\c64\ksa64-phase4-export-c64"
$expected = Join-Path $projectRoot "phase4\examples\ksa4-stock-report.kxv4"
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("ksa64-phase4-iec-" + [Guid]::NewGuid().ToString("N"))
function Assert-ExitCode([string]$label) { if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." } }
function Assert-BytesEqual([string]$left, [string]$right) {
    $a = [IO.File]::ReadAllBytes($left)
    $b = [IO.File]::ReadAllBytes($right)
    if ($a.Length -ne $b.Length -or -not [Linq.Enumerable]::SequenceEqual[byte]($a, $b)) {
        throw "C64 IEC readback differs from KXV4 source."
    }
}
New-Item -ItemType Directory -Path $tempRoot | Out-Null
Push-Location $projectRoot
try {
    & $rustWrapper -ReturnToCaller -WorkingDirectory "." `
        cargo build --profile c64 --target mos-c64-none --features c64 `
        -Z build-std=core -Z build-std-features=compiler-builtins-mem `
        --bin ksa64-phase4-export-c64
    Assert-ExitCode "Phase 4 C64 exporter build"

    $disk = Join-Path $tempRoot "stock-export.d64"
    & $c1541 -format "KSA4 EXPORT,04" d64 $disk -write $artifact RUNME
    Assert-ExitCode "Format stock export disk"
    $successJson = & python -B phase4/reference/vice_export.py --vice $vice --prg $artifact --disk $disk --timeout 180
    Assert-ExitCode "C64 IEC export"
    $success = ($successJson -join "`n") | ConvertFrom-Json
    & $c1541 $disk -dir
    Assert-ExitCode "Inspect C64 export disk"
    $extractDir = Join-Path $tempRoot "extract"
    New-Item -ItemType Directory -Path $extractDir | Out-Null
    & $c1541 $disk -cd $extractDir -extract
    Assert-ExitCode "Extract C64-exported report"
    $expectedLength = (Get-Item -LiteralPath $expected).Length
    $readbackCandidates = @(Get-ChildItem -LiteralPath $extractDir -File | Where-Object Length -eq $expectedLength)
    if ($readbackCandidates.Count -ne 1) { throw "Expected one $expectedLength-byte extracted report." }
    $readback = $readbackCandidates[0].FullName
    Assert-BytesEqual $expected $readback

    $fullDisk = Join-Path $tempRoot "full-export.d64"
    $filler = Join-Path $tempRoot "filler.bin"
    $stream = [IO.File]::Create($filler)
    try { $stream.SetLength(161000) } finally { $stream.Dispose() }
    & $c1541 -format "KSA4 FULL,04" d64 $fullDisk -write $artifact RUNME -write $filler FILLER
    Assert-ExitCode "Prepare nearly full disk"
    $failureJson = & python -B phase4/reference/vice_export.py --vice $vice --prg $artifact --disk $fullDisk --expect-error --timeout 180
    Assert-ExitCode "C64 disk-full probe"
    $failure = ($failureJson -join "`n") | ConvertFrom-Json

    $bytes = (Get-Item -LiteralPath $artifact).Length
    Write-Host "C64 IEC exporter: $bytes bytes"
    Write-Host "Success readback: $($success.passed), $((Get-Item $readback).Length) exact bytes"
    Write-Host "Disk-full response: code $($failure.code)"
    Write-Host "PHASE 4 C64 IEC EXPORT: PASS"
} finally {
    Pop-Location
    $resolvedTemp = [IO.Path]::GetFullPath($tempRoot)
    $systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if ($resolvedTemp.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase) -and
        (Split-Path -Leaf $resolvedTemp).StartsWith("ksa64-phase4-iec-")) {
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
    }
}