[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$rustCandidate = Join-Path $projectRoot "phase0\candidates\rust"
$oscarCandidate = Join-Path $projectRoot "phase0\candidates\oscar64"
$rustWrapper = Join-Path $projectRoot "tools\toolchains\rust-mos.ps1"
$oscarWrapper = Join-Path $projectRoot "tools\toolchains\oscar64.ps1"

Push-Location $projectRoot
try {
    Write-Host "== Generated inputs =="
    & python -B phase0/reference/generate_vectors.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 0 JSON vectors are stale." }
    & python -B phase0/reference/emit_candidate_vectors.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 0 candidate bindings are stale." }
    & python -B phase0/reference/emit_vertical_bindings.py --check
    if ($LASTEXITCODE -ne 0) { throw "Phase 0 vertical bindings are stale." }

    Write-Host ""
    Write-Host "== Native Rust =="
    & cargo fmt --manifest-path phase0/candidates/rust/Cargo.toml -- --check
    if ($LASTEXITCODE -ne 0) { throw "Rust candidate formatting check failed." }
    & cargo test --manifest-path phase0/candidates/rust/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw "Native Rust candidate tests failed." }

    $rustBuildArguments = @(
        "cargo", "build", "--release",
        "--target", "mos-sim-none",
        "--features", "sim",
        "-Z", "build-std=core",
        "-Z", "build-std-features=compiler-builtins-mem"
    )

    Write-Host ""
    Write-Host "== rust-mos specialized two-word path =="
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustBuildArguments `
        --bin ksa64-phase0-rust-manual-sim
    if ($LASTEXITCODE -ne 0) { throw "Specialized rust-mos build failed." }
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" sh -lc `
        "mos-sim target/mos-sim-none/release/ksa64-phase0-rust-manual-sim"
    if ($LASTEXITCODE -ne 0) { throw "Specialized rust-mos vectors failed." }
    Write-Host "Specialized rust-mos vectors: PASS"

    Write-Host ""
    Write-Host "== rust-mos compiler-provided u64 baseline =="
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustBuildArguments `
        --bin ksa64-phase0-rust-sim
    if ($LASTEXITCODE -ne 0) { throw "Baseline rust-mos build failed." }
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" sh -lc `
        "mos-sim target/mos-sim-none/release/ksa64-phase0-rust-sim"
    $baselineResult = $LASTEXITCODE
    if ($baselineResult -ne 2) {
        throw "Expected the documented u64 baseline result 2, got $baselineResult."
    }
    Write-Host "Baseline rust-mos result: expected 2 failures (documented toolchain risk)"

    Write-Host ""
    Write-Host "== rust-mos vertical workload =="
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustBuildArguments `
        --bin ksa64-phase0-rust-vertical-sim
    if ($LASTEXITCODE -ne 0) { throw "Rust vertical workload build failed." }
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" sh -lc `
        "mos-sim --cycles target/mos-sim-none/release/ksa64-phase0-rust-vertical-sim"
    if ($LASTEXITCODE -ne 0) { throw "Rust vertical workload failed." }
    Write-Host "Rust vertical workload: PASS"

    Write-Host ""
    Write-Host "== rust-mos optimized vertical workload =="
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustBuildArguments `
        --bin ksa64-phase0-rust-vertical-optimized-sim
    if ($LASTEXITCODE -ne 0) { throw "Optimized Rust vertical workload build failed." }
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" sh -lc `
        "mos-sim target/mos-sim-none/release/ksa64-phase0-rust-vertical-optimized-sim"
    if ($LASTEXITCODE -ne 0) { throw "Optimized Rust vertical workload failed." }
    Write-Host "Optimized Rust vertical workload: PASS"
    Write-Host ""
    Write-Host "== rust-mos C64 artifacts =="
    $rustC64Arguments = @(
        "cargo", "build", "--release",
        "--target", "mos-c64-none",
        "--features", "c64",
        "-Z", "build-std=core",
        "-Z", "build-std-features=compiler-builtins-mem"
    )
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustC64Arguments `
        --bin ksa64-phase0-rust-c64
    if ($LASTEXITCODE -ne 0) { throw "Baseline Rust C64 build failed." }
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustC64Arguments `
        --bin ksa64-phase0-rust-manual-c64
    if ($LASTEXITCODE -ne 0) { throw "Specialized Rust C64 build failed." }
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustC64Arguments `
        --bin ksa64-phase0-rust-vertical-c64
    if ($LASTEXITCODE -ne 0) { throw "Rust vertical C64 build failed." }
    & $rustWrapper -WorkingDirectory "phase0/candidates/rust" @rustC64Arguments `
        --bin ksa64-phase0-rust-vertical-optimized-c64
    if ($LASTEXITCODE -ne 0) { throw "Optimized Rust vertical C64 build failed." }
    Write-Host ""
    Write-Host "== Native Oscar64-compatible C++ =="
    $vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "Visual Studio locator not found: $vswhere"
    }
    $visualStudio = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if (-not $visualStudio) { throw "Visual C++ build tools were not found." }
    $vcvarsall = Join-Path $visualStudio "VC\Auxiliary\Build\vcvarsall.bat"
    $oscarOutput = Join-Path $oscarCandidate "out"
    New-Item -ItemType Directory -Force -Path $oscarOutput | Out-Null
    $nativeExecutable = Join-Path $oscarOutput "phase0-native.exe"
    $compileCommand =
        "call `"$vcvarsall`" x64 >nul && " +
        "cl /nologo /std:c++14 /O2 /W4 /WX /GR- " +
        "phase0\candidates\oscar64\main.cpp " +
        "phase0\candidates\oscar64\arithmetic.cpp " +
        "/Fo:`"$oscarOutput\\`" /Fe:`"$nativeExecutable`""
    & cmd.exe /d /c $compileCommand
    if ($LASTEXITCODE -ne 0) { throw "Native C++ candidate build failed." }
    & $nativeExecutable
    if ($LASTEXITCODE -ne 0) { throw "Native C++ candidate vectors failed." }

    $nativeVerticalExecutable = Join-Path $oscarOutput "phase0-vertical-native.exe"
    $verticalCompileCommand =
        "call `"$vcvarsall`" x64 >nul && " +
        "cl /nologo /std:c++14 /O2 /W4 /WX /GR- " +
        "phase0\candidates\oscar64\vertical_main.cpp " +
        "phase0\candidates\oscar64\vertical.cpp " +
        "phase0\candidates\oscar64\arithmetic.cpp " +
        "phase0\candidates\oscar64\optimized.cpp " +
        "/Fo:`"$oscarOutput\\`" /Fe:`"$nativeVerticalExecutable`""
    & cmd.exe /d /c $verticalCompileCommand
    if ($LASTEXITCODE -ne 0) { throw "Native C++ vertical build failed." }
    & $nativeVerticalExecutable
    if ($LASTEXITCODE -ne 0) { throw "Native C++ vertical workload failed." }

    $nativeOptimizedVerticalExecutable = Join-Path $oscarOutput "phase0-vertical-optimized-native.exe"
    $optimizedVerticalCompileCommand =
        "call `"$vcvarsall`" x64 >nul && " +
        "cl /nologo /std:c++14 /O2 /W4 /WX /GR- " +
        "phase0\candidates\oscar64\vertical_optimized_main.cpp " +
        "phase0\candidates\oscar64\vertical.cpp " +
        "phase0\candidates\oscar64\arithmetic.cpp " +
        "phase0\candidates\oscar64\optimized.cpp " +
        "/Fo:`"$oscarOutput\\`" /Fe:`"$nativeOptimizedVerticalExecutable`""
    & cmd.exe /d /c $optimizedVerticalCompileCommand
    if ($LASTEXITCODE -ne 0) { throw "Native optimized C++ vertical build failed." }
    & $nativeOptimizedVerticalExecutable
    if ($LASTEXITCODE -ne 0) { throw "Native optimized C++ vertical workload failed." }
    Write-Host ""
    Write-Host "== Oscar64 C64 execution =="
    $oscarArtifact = Join-Path $oscarOutput "phase0-oscar64.prg"
    & $oscarWrapper -ReturnToCaller "-tm=c64" "-pp" "-O2" "-dKSA64_OSCAR64" "-e" `
        "-o=$oscarArtifact" `
        phase0/candidates/oscar64/main.cpp `
        phase0/candidates/oscar64/arithmetic.cpp
    if ($LASTEXITCODE -ne 0) { throw "Oscar64 C64 vectors failed." }
    $oscarVerticalArtifact = Join-Path $oscarOutput "phase0-vertical-oscar64.prg"
    & $oscarWrapper -ReturnToCaller "-tm=c64" "-pp" "-O2" "-dKSA64_OSCAR64" "-e" `
        "-o=$oscarVerticalArtifact" `
        phase0/candidates/oscar64/vertical_main.cpp `
        phase0/candidates/oscar64/vertical.cpp `
        phase0/candidates/oscar64/arithmetic.cpp `
        phase0/candidates/oscar64/optimized.cpp
    if ($LASTEXITCODE -ne 0) { throw "Oscar64 C64 vertical workload failed." }
    $oscarOptimizedVerticalArtifact = Join-Path $oscarOutput "phase0-vertical-optimized-oscar64.prg"
    & $oscarWrapper -ReturnToCaller "-tm=c64" "-pp" "-O2" "-dKSA64_OSCAR64" "-e" `
        "-o=$oscarOptimizedVerticalArtifact" `
        phase0/candidates/oscar64/vertical_optimized_main.cpp `
        phase0/candidates/oscar64/vertical.cpp `
        phase0/candidates/oscar64/arithmetic.cpp `
        phase0/candidates/oscar64/optimized.cpp
    if ($LASTEXITCODE -ne 0) { throw "Oscar64 optimized vertical workload failed." }
    Write-Host ""
    Write-Host "== Artifact sizes =="
    $artifacts = @(
        Join-Path $rustCandidate "target\mos-c64-none\release\ksa64-phase0-rust-c64"
        Join-Path $rustCandidate "target\mos-c64-none\release\ksa64-phase0-rust-manual-c64"
        Join-Path $rustCandidate "target\mos-c64-none\release\ksa64-phase0-rust-vertical-c64"
        Join-Path $rustCandidate "target\mos-c64-none\release\ksa64-phase0-rust-vertical-optimized-c64"
        $oscarArtifact
        $oscarVerticalArtifact
        $oscarOptimizedVerticalArtifact
    )
    foreach ($artifact in $artifacts) {
        $file = Get-Item -LiteralPath $artifact
        Write-Host "$($file.Name): $($file.Length) bytes"
    }

    Write-Host ""
    Write-Host "PHASE 0 CORRECTNESS GATES: PASS"
} finally {
    Pop-Location
}
