[CmdletBinding()]
param([switch]$SkipMos)
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase8-audit-" + [Guid]::NewGuid().ToString("N"))
$nativeTarget = Join-Path $auditRoot "native-target"
function Check { if ($LASTEXITCODE -ne 0) { throw "native command failed: $LASTEXITCODE" } }
function Gate([string]$label, [scriptblock]$action) { Write-Host "`n=== $label ==="; $global:LASTEXITCODE = 0; & $action; Check }
function Equal([string]$a, [string]$b) { if ((Get-FileHash -Algorithm SHA256 -LiteralPath $a).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $b).Hash) { throw "artifact mismatch: $b" } }
function NoVice { if ($p = Get-Process x64sc -ErrorAction SilentlyContinue) { throw "Close VICE PID(s) $($p.Id -join ', ')" } }
function Stock([string]$path) { $b = [IO.File]::ReadAllBytes($path); $load = [BitConverter]::ToUInt16($b, 0); $end = $load + $b.Length - 2; if ($end -ge 0xC000) { throw "$path ends at 0x$($end.ToString('X4'))" }; Write-Host "$path $($b.Length) bytes end 0x$($end.ToString('X4'))" }
New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
$previousCargoTarget = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $nativeTarget
try {
    Gate "native regression" {
        cargo fmt --all -- --check; Check
        cargo clippy --workspace --all-targets --features fixtures -- -D warnings; Check
        cargo test --workspace --features fixtures; Check
        python -B phase8/reference/generate_numeric.py --check; Check
        python -B phase8/reference/generate_aero_vectors.py --check; Check
        python -B phase8/reference/generate_wind_vectors.py --check; Check
        python -B phase8/reference/analyze_geometry.py --check; Check
        python -B phase8/reference/analyze_float.py --check
    }
    Gate "packs and mission artifacts" {
        $packs = Join-Path $auditRoot packs
        $art = Join-Path $auditRoot artifacts
        cargo run -q -p ksa64-host --bin phase8_compile -- phase8/source-data $packs; Check
        foreach ($n in 'firestorm54.kvp8', 'aerotech-i211w.kmp8', 'firestorm-i211.kmc8', 'firestorm-calm.kwp8') { Equal "phase8/examples/$n" "$packs/$n" }
        cargo run -q -p ksa64-host --bin phase8_artifacts -- $art; Check
        foreach ($n in 'firestorm-i211.kst8', 'firestorm-i211.ksr8', 'firestorm-i211.kph8') { Equal "phase8/examples/$n" "$art/$n" }
    }
    Gate "campaign reproducibility and independent evidence" {
        $one = Join-Path $auditRoot one
        $four = Join-Path $auditRoot four
        cargo run -q -p ksa64-host --release --bin phase8_campaign -- phase8/examples $one 1024 1; Check
        cargo run -q -p ksa64-host --release --bin phase8_campaign -- phase8/examples $four 1024 4; Check
        foreach ($n in 'campaign-1024.ksc8', 'campaign-1024.kra8') { Equal "phase8/examples/$n" "$one/$n"; Equal "$one/$n" "$four/$n" }
        python -B phase8/reference/analyze_campaign.py --ksc phase8/examples/campaign-1024.ksc8 --kra phase8/examples/campaign-1024.kra8 --output phase8/reference-campaign-analysis.json --check; Check
        python -B phase8/reference/openrocket/build_manifest.py --check; Check
        python -B phase8/reference/openrocket/compare.py --check; Check
        python -B phase8/reference/openrocket/verify_evidence.py
    }
    if (-not $SkipMos) {
        $env:CARGO_TARGET_DIR = $null
        $versions = Get-Content toolchains/versions.json -Raw | ConvertFrom-Json
        $vice = (Resolve-Path $versions.vice.projectRelativeExecutable).Path
        $mos = Join-Path $projectRoot tools/toolchains/rust-mos.ps1
        Gate "stock C64 packaging" {
            & $mos -WorkingDirectory . cargo build --profile c64 --target mos-c64-none --features c64,fixtures -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase8-full-c64 --bin ksa64-phase8-trace-c64 --bin ksa64-phase8-replay-c64; Check
            foreach ($n in 'ksa64-phase8-full-c64', 'ksa64-phase8-trace-c64', 'ksa64-phase8-replay-c64') { Stock "target/mos-c64-none/c64/$n" }
        }
        Gate "finite target exactness and replay" {
            NoVice
            python -B phase8/reference/vice_phase8_trace.py --vice $vice --prg target/mos-c64-none/c64/ksa64-phase8-trace-c64 --host phase8/host-trace-v1.json --output phase8/c64-exact-trace-v1.json --check; Check
            NoVice
            python -B phase8/reference/vice_phase8_replay.py --vice $vice --prg target/mos-c64-none/c64/ksa64-phase8-replay-c64 --output phase8/c64-stock-replay-v1.json --check; Check
            NoVice
        }
    }
    Write-Host "`nPHASE 8 COMPLETION AUDIT: PASS"
} finally {
    $env:CARGO_TARGET_DIR = $previousCargoTarget
    Pop-Location
    $resolvedAudit = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolvedAudit.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) { throw "unsafe cleanup" }
    if (Test-Path $resolvedAudit) { Remove-Item -LiteralPath $resolvedAudit -Recurse -Force }
}