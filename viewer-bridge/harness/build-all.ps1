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
