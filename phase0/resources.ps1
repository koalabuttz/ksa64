[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$oscarWrapper = Join-Path $projectRoot "tools\toolchains\oscar64.ps1"
$rustRelease = Join-Path $projectRoot "phase0\candidates\rust\target\mos-c64-none\release"
$rustArtifact = Join-Path $rustRelease "ksa64-phase0-rust-vertical-timed-c64"
$rustMap = Join-Path $rustRelease "vertical-timed.map"
$oscarOutput = Join-Path $projectRoot "phase0\candidates\oscar64\out"
$oscarArtifact = Join-Path $oscarOutput "phase0-vertical-timed-oscar64.prg"
$oscarMap = Join-Path $oscarOutput "phase0-vertical-timed-oscar64.map"

function Assert-ExitCode([string]$label) {
    if ($LASTEXITCODE -ne 0) { throw "$label failed with exit code $LASTEXITCODE." }
}

function Convert-Hex([string]$value) {
    return [Convert]::ToInt32($value, 16)
}

function Get-LldSection([string]$text, [string]$section) {
    $pattern = '(?m)^\s*[0-9a-f]+\s+[0-9a-f]+\s+([0-9a-f]+)\s+\d+\s+' +
        [regex]::Escape($section) + '\s*$'
    $match = [regex]::Match($text, $pattern)
    if (-not $match.Success) { throw "Rust map is missing section $section." }
    return Convert-Hex $match.Groups[1].Value
}

function Get-OscarSection([string]$text, [string]$kind, [string]$name) {
    $pattern = '(?m)^([0-9a-f]+) - ([0-9a-f]+) : ' +
        [regex]::Escape($kind) + ', ' + [regex]::Escape($name) + '$'
    $match = [regex]::Match($text, $pattern)
    if (-not $match.Success) { throw "Oscar64 map is missing $kind/$name." }
    return (Convert-Hex $match.Groups[2].Value) - (Convert-Hex $match.Groups[1].Value)
}

Push-Location $projectRoot
try {
    Write-Host "== Resource-report builds =="
    $rustCommand =
        "cargo rustc --release --target mos-c64-none --features c64 " +
        "-Z build-std=core -Z build-std-features=compiler-builtins-mem " +
        "--bin ksa64-phase0-rust-vertical-timed-c64 -- " +
        "-C link-arg=-Wl,-Map=target/mos-c64-none/release/vertical-timed.map"
    & $rustWrapper -ReturnToCaller -WorkingDirectory "phase0/candidates/rust" `
        sh -lc $rustCommand
    Assert-ExitCode "Rust resource-report build"

    New-Item -ItemType Directory -Force -Path $oscarOutput | Out-Null
    & $oscarWrapper -ReturnToCaller `
        "-tm=c64" "-pp" "-O2" "-dKSA64_OSCAR64" `
        "-o=$oscarArtifact" `
        phase0/candidates/oscar64/vertical_timed_main.cpp `
        phase0/candidates/oscar64/vertical.cpp `
        phase0/candidates/oscar64/arithmetic.cpp `
        phase0/candidates/oscar64/optimized.cpp `
        phase0/candidates/oscar64/c64_timer.cpp
    Assert-ExitCode "Oscar64 resource-report build"

    $rustText = Get-Content -Raw -LiteralPath $rustMap
    $oscarText = Get-Content -Raw -LiteralPath $oscarMap
    $stackEnd = [regex]::Match(
        $oscarText,
        '(?m)^([0-9a-f]+) - \1 : StackEnd, END:stack$'
    )
    if (-not $stackEnd.Success) { throw "Oscar64 map is missing StackEnd." }
    $oscarStackEnvelope = 0xa000 - (Convert-Hex $stackEnd.Groups[1].Value)

    $rows = @(
        [pscustomobject]@{
            Candidate = "rust-mos"
            PRG = (Get-Item -LiteralPath $rustArtifact).Length
            Code = Get-LldSection $rustText ".text"
            ReadOnlyData = Get-LldSection $rustText ".rodata"
            BSS = Get-LldSection $rustText ".bss"
            ZeroPage = Get-LldSection $rustText ".zp"
            StaticStack = Get-LldSection $rustText ".noinit"
        },
        [pscustomobject]@{
            Candidate = "Oscar64"
            PRG = (Get-Item -LiteralPath $oscarArtifact).Length
            Code = Get-OscarSection $oscarText "DATA" "code"
            ReadOnlyData = Get-OscarSection $oscarText "DATA" "data"
            BSS = Get-OscarSection $oscarText "BSS" "bss"
            ZeroPage = Get-OscarSection $oscarText "ZEROPAGE" "zeropage"
            StaticStack = $oscarStackEnvelope
        }
    )

    Write-Host ""
    Write-Host "== Timed-kernel resource summary (bytes) =="
    $rows | Format-Table -AutoSize
    Write-Host "Rust StaticStack is its linker-reserved .noinit static stack."
    Write-Host "Oscar StaticStack is the map envelope from StackEnd through `$a000."
    Write-Host "PHASE 0 RESOURCE REPORT: PASS"
} finally {
    Pop-Location
}
