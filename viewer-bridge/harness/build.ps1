[CmdletBinding()]
param([switch]$PanicProbe)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Import-VsDevEnvironment {
    if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
        return
    }

    $programFilesX86 = [Environment]::GetFolderPath("ProgramFilesX86")
    $vswhere = Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "Visual Studio environment is not active and vswhere.exe was not found."
    }
    $vswhereArgs = @(
        "-latest",
        "-products", "*",
        "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        "-property", "installationPath"
    )
    $installation = (& $vswhere @vswhereArgs).Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($installation)) {
        throw "Could not locate a Visual Studio installation with the x64 C++ toolchain."
    }
    $vsDevCmd = Join-Path $installation "Common7\Tools\VsDevCmd.bat"
    if (-not (Test-Path -LiteralPath $vsDevCmd -PathType Leaf)) {
        throw "Visual Studio developer environment script was not found: $vsDevCmd"
    }

    $lines = & cmd.exe /d /s /c "`"$vsDevCmd`" -no_logo -arch=x64 -host_arch=x64 && set"
    if ($LASTEXITCODE -ne 0) {
        throw "Visual Studio developer environment initialization failed."
    }
    foreach ($line in $lines) {
        if ($line -match "^([^=]+)=(.*)$") {
            [Environment]::SetEnvironmentVariable($matches[1], $matches[2], "Process")
        }
    }
    if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
        throw "Visual Studio environment initialized without exposing cl.exe."
    }
}

function Get-Sha256([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    } finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
}

$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Import-VsDevEnvironment
Push-Location $repo
try {
    $featureArgs = if ($PanicProbe) { @("--features", "panic-probe") } else { @() }
    & cargo build -p ksa64-viewer-bridge --profile viewer @featureArgs
    if ($LASTEXITCODE -ne 0) { throw "viewer bridge build failed" }

    $commit = (& git rev-parse --short=12 HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $commit) {
        throw "could not resolve bridge commit identity"
    }
    $source = Join-Path $repo "target\viewer\ksa64_viewer_bridge.dll"
    $staged = Join-Path $repo "target\viewer\ksa64_viewer_bridge_$commit.dll"
    Copy-Item -LiteralPath $source -Destination $staged -Force
    [ordered]@{
        schema = "ksa64.viewer-bridge-artifact.v1"
        abi_version = 1
        commit = $commit
        file = (Split-Path $staged -Leaf)
        sha256 = Get-Sha256 $staged
    } | ConvertTo-Json | Set-Content -LiteralPath "$staged.json" -Encoding utf8

    $harnessBin = Join-Path $PSScriptRoot "bin"
    New-Item -ItemType Directory -Path $harnessBin -Force | Out-Null
    $harnessExe = Join-Path $harnessBin "ksa64_viewer_harness.exe"
    $harnessObj = Join-Path $harnessBin "main.obj"
    $compilerArgs = @(
        "/nologo", "/std:c++20", "/EHsc", "/W4", "/WX",
        (Join-Path $PSScriptRoot "main.cpp"),
        "/Fo$harnessObj",
        "/Fe:$harnessExe"
    )
    & cl.exe @compilerArgs
    if ($LASTEXITCODE -ne 0) { throw "native C++ harness build failed" }
    & $harnessExe $staged
    if ($LASTEXITCODE -ne 0) { throw "native C++ harness failed" }
} finally {
    Pop-Location
}
