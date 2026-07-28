[CmdletBinding()]
param(
    [string]$ExpectedSha256 = "",
    [switch]$SkipBridgeBuild
)

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

$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Import-VsDevEnvironment
Push-Location $repo
try {
    if (-not $SkipBridgeBuild) {
        & cargo build -p ksa64-viewer-bridge --profile viewer --locked
        if ($LASTEXITCODE -ne 0) { throw "viewer bridge build failed" }
    }

    $source = Join-Path $repo "target\viewer\ksa64_viewer_bridge.dll"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "viewer bridge DLL was not found: $source"
    }

    $harnessBin = Join-Path $PSScriptRoot "bin"
    New-Item -ItemType Directory -Path $harnessBin -Force | Out-Null
    $harnessExe = Join-Path $harnessBin "ksa64_viewer_full_mission_harness.exe"
    $harnessObj = Join-Path $harnessBin "full_mission.obj"
    $compilerArgs = @(
        "/nologo", "/std:c++20", "/EHsc", "/W4", "/WX",
        (Join-Path $PSScriptRoot "full_mission.cpp"),
        "/Fo$harnessObj",
        "/Fe:$harnessExe"
    )
    & cl.exe @compilerArgs
    if ($LASTEXITCODE -ne 0) { throw "Phase 12B native C++ harness build failed" }

    $harnessArgs = @($source)
    if (-not [string]::IsNullOrWhiteSpace($ExpectedSha256)) {
        $harnessArgs += $ExpectedSha256
    }
    & $harnessExe @harnessArgs
    if ($LASTEXITCODE -ne 0) { throw "Phase 12B native C++ harness failed" }
} finally {
    Pop-Location
}
