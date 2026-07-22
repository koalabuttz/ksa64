[CmdletBinding()]
param(
    [switch]$ReturnToCaller,

    [string]$WorkingDirectory = ".",

    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$Command
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$versionsPath = Join-Path $projectRoot "toolchains\versions.json"
$versions = Get-Content -Raw -LiteralPath $versionsPath | ConvertFrom-Json
$image = $versions.rustMos.image

if (-not $Command -or $Command.Count -eq 0) {
    $Command = @("rustc", "--version", "--verbose")
}

$requestedDirectory = Join-Path $projectRoot $WorkingDirectory
$resolvedDirectory = (Resolve-Path -LiteralPath $requestedDirectory).Path
$rootPrefix = $projectRoot.TrimEnd("\") + "\"

if (
    $resolvedDirectory -ne $projectRoot -and
    -not $resolvedDirectory.StartsWith(
        $rootPrefix,
        [System.StringComparison]::OrdinalIgnoreCase
    )
) {
    throw "Working directory must remain inside the KSA64 project."
}

$relativeDirectory = if ($resolvedDirectory -eq $projectRoot) {
    "."
} else {
    $resolvedDirectory.Substring($rootPrefix.Length).Replace("\", "/")
}

$containerDirectory = if ($relativeDirectory -eq ".") {
    "/workspace"
} else {
    "/workspace/$relativeDirectory"
}

$dockerArguments = @(
    "run",
    "--rm",
    "-e", "PATH=/usr/local/rust-mos/bin:/usr/local/bin:/usr/bin:/bin",
    "-e", "CARGO_HOME=/tmp/cargo",
    "-v", "$($projectRoot):/workspace",
    "-w", $containerDirectory,
    $image
) + $Command

& docker @dockerArguments
$dockerExitCode = $LASTEXITCODE
if ($ReturnToCaller) {
    $global:LASTEXITCODE = $dockerExitCode
    return
}
exit $dockerExitCode

