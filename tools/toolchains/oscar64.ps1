[CmdletBinding()]
param(
    [switch]$ReturnToCaller,

    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$CompilerArguments
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$versionsPath = Join-Path $projectRoot "toolchains\versions.json"
$versions = Get-Content -Raw -LiteralPath $versionsPath | ConvertFrom-Json

$candidates = [System.Collections.Generic.List[string]]::new()

if ($env:KSA64_OSCAR64) {
    $candidates.Add($env:KSA64_OSCAR64)
}

$localCompiler = Join-Path (
    $projectRoot
) $versions.oscar64.projectRelativeCompiler.Replace("/", "\")
$candidates.Add($localCompiler)
$candidates.Add("C:\Program Files\oscar64\bin\oscar64.exe")
$candidates.Add("C:\Program Files (x86)\oscar64\bin\oscar64.exe")

$pathCompiler = Get-Command oscar64 -ErrorAction SilentlyContinue
if ($pathCompiler) {
    $candidates.Add($pathCompiler.Source)
}

$compiler = $candidates |
    Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } |
    Select-Object -First 1

if (-not $compiler) {
    throw @"
Oscar64 was not found.

Expected project-local path:
  $localCompiler

See toolchains/README.md for the pinned release and setup instructions.
"@
}

if (-not $CompilerArguments -or $CompilerArguments.Count -eq 0) {
    $CompilerArguments = @("-h")
}

& $compiler @CompilerArguments
$compilerExitCode = $LASTEXITCODE
if ($ReturnToCaller) {
    $global:LASTEXITCODE = $compilerExitCode
    return
}
exit $compilerExitCode

