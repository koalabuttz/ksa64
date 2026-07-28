[CmdletBinding()]
param(
    [string]$Phase12bExpectedSha256 = "",
    [switch]$PanicProbe
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# Keep the frozen Phase 12A harness invocation independent and byte-for-byte
# unchanged, then run the additive Phase 12B full-mission harness explicitly.
& (Join-Path $PSScriptRoot "build.ps1") -PanicProbe:$PanicProbe
if ($LASTEXITCODE -ne 0) {
    throw "frozen Phase 12A harness failed"
}

$fullArguments = @{ SkipBridgeBuild = $true }
if (-not [string]::IsNullOrWhiteSpace($Phase12bExpectedSha256)) {
    $fullArguments.ExpectedSha256 = $Phase12bExpectedSha256
}
& (Join-Path $PSScriptRoot "build-full.ps1") @fullArguments
if ($LASTEXITCODE -ne 0) {
    throw "Phase 12B full-mission harness failed"
}

# Phase 12C is additive: this resolves only GlobalDisplayApiV1 after the
# frozen ABI-v1 and Phase 12B harnesses have completed unchanged.
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$commit = (& git -C $repo rev-parse --short=12 HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($commit)) {
    throw "could not resolve bridge commit identity for GlobalDisplay harness"
}
$bridge = Join-Path $repo "target\viewer\ksa64_viewer_bridge_$commit.dll"
$manifest = "$bridge.json"
if (-not (Test-Path -LiteralPath $bridge -PathType Leaf) -or
    -not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw "commit-qualified bridge DLL or manifest is missing for GlobalDisplay harness"
}
$bin = Join-Path $PSScriptRoot "bin"
$exe = Join-Path $bin "ksa64_viewer_global_display_harness.exe"
$obj = Join-Path $bin "global_display.obj"
$evidence = Join-Path $bin "phase12c-global-display-evidence.json"
& cl.exe /nologo /std:c++20 /EHsc /W4 /WX (Join-Path $PSScriptRoot "global_display.cpp") "/Fo$obj" "/Fe:$exe"
if ($LASTEXITCODE -ne 0) {
    throw "Phase 12C GlobalDisplay native C++ harness build failed"
}
& $exe $bridge $manifest $evidence
if ($LASTEXITCODE -ne 0) {
    throw "Phase 12C GlobalDisplay native C++ harness failed"
}
