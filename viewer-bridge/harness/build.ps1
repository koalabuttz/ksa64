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
    & cargo build -p ksa64-viewer-bridge --profile viewer --locked @featureArgs
    if ($LASTEXITCODE -ne 0) { throw "viewer bridge build failed" }

    $commit = (& git rev-parse --short=12 HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $commit) {
        throw "could not resolve bridge commit identity"
    }
    $source = Join-Path $repo "target\viewer\ksa64_viewer_bridge.dll"
    $staged = Join-Path $repo "target\viewer\ksa64_viewer_bridge_$commit.dll"
    Copy-Item -LiteralPath $source -Destination $staged -Force
    $rustcVersion = (& rustc -vV)
    if ($LASTEXITCODE -ne 0) { throw "could not inspect Rust target identity" }
    $targetTriple = (($rustcVersion | Where-Object { $_ -like "host: *" }) -replace "^host: ", "").Trim()
    if (-not $targetTriple) { throw "rustc did not report a host target triple" }
    $catalogIdentity = "b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13"
    $manifestJson = & cargo run --locked -p ksa64-viewer-bridge --bin bridge-manifest --quiet -- `
        $commit viewer (Split-Path $staged -Leaf) $targetTriple windows x86_64 (Get-Sha256 $staged) $catalogIdentity
    if ($LASTEXITCODE -ne 0) { throw "bridge manifest generation failed" }
    $manifestJson | Set-Content -LiteralPath "$staged.json" -Encoding utf8

    $harnessBin = Join-Path $PSScriptRoot "bin"
    New-Item -ItemType Directory -Path $harnessBin -Force | Out-Null
    $headerSmokeExe = Join-Path $harnessBin "ksa64_viewer_header_smoke.exe"
    $headerSmokeObj = Join-Path $harnessBin "header_smoke.obj"
    & cl.exe /nologo /TC /std:c11 /W4 /WX (Join-Path $PSScriptRoot "header_smoke.c") "/Fo$headerSmokeObj" "/Fe:$headerSmokeExe"
    if ($LASTEXITCODE -ne 0) { throw "portable C header smoke build failed" }
    & $headerSmokeExe
    if ($LASTEXITCODE -ne 0) { throw "portable C header smoke failed" }
    $kps1VectorExe = Join-Path $harnessBin "ksa64_kps1_c_vectors.exe"
    $kps1VectorObj = Join-Path $harnessBin "kps1_vectors.obj"
    & cl.exe /nologo /TC /std:c11 /W4 /WX (Join-Path $repo "presentation\c\kps1_vectors.c") "/Fo$kps1VectorObj" "/Fe:$kps1VectorExe"
    if ($LASTEXITCODE -ne 0) { throw "independent C KPS1 vector build failed" }
    & $kps1VectorExe
    if ($LASTEXITCODE -ne 0) { throw "independent C KPS1 vector failed" }
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
