[CmdletBinding()]
param([switch]$SkipMos)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase7-audit-" + [Guid]::NewGuid().ToString("N"))

function Invoke-PhaseGate([string]$Label, [scriptblock]$Action) {
    Write-Host ""
    Write-Host "============================================================"
    Write-Host $Label
    Write-Host "============================================================"
    & $Action
    if ($LASTEXITCODE -ne 0) { throw "$Label failed with exit code $LASTEXITCODE" }
}

function Assert-NoVice {
    $running = Get-Process -Name "x64sc" -ErrorAction SilentlyContinue
    if ($running) { throw "Refusing to launch another VICE instance; close PID(s) $($running.Id -join ', ')." }
}

function Assert-FileEqual([string]$Expected, [string]$Actual) {
    $expectedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Expected).Hash
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Actual).Hash
    if ($expectedHash -ne $actualHash) {
        throw "Artifact mismatch: $Actual differs from $Expected"
    }
}

function Assert-JsonEqual([string]$Expected, [string]$Actual) {
    $expectedJson = (Get-Content -Raw -LiteralPath $Expected | ConvertFrom-Json | ConvertTo-Json -Depth 20 -Compress)
    $actualJson = (Get-Content -Raw -LiteralPath $Actual | ConvertFrom-Json | ConvertTo-Json -Depth 20 -Compress)
    if ($expectedJson -ne $actualJson) { throw "JSON mismatch: $Actual differs from $Expected" }
}

function Assert-StockPrg([string]$Path) {
    $raw = [IO.File]::ReadAllBytes($Path)
    if ($raw.Length -lt 3) { throw "Invalid PRG: $Path" }
    $load = [BitConverter]::ToUInt16($raw, 0)
    $end = $load + $raw.Length - 2
    if ($end -gt 0xC000) { throw "$Path ends at 0x$($end.ToString('X4')), beyond the stock endpoint boundary." }
    Write-Host "$([IO.Path]::GetFileName($Path)): $($raw.Length) bytes, 0x$($load.ToString('X4'))-0x$($end.ToString('X4'))"
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    Invoke-PhaseGate "PHASE 7 NATIVE CONTRACT AND REGRESSION AUDIT" {
        cargo fmt --all -- --check
        cargo clippy --workspace --all-targets --features fixtures -- -D warnings
        cargo test --workspace --features fixtures
        python -B phase7/reference/generate_numeric.py --check
        python -B phase7/reference/generate_environment.py --check
    }

    Invoke-PhaseGate "PHASE 7 OFFLINE PACK AND MISSION REPRODUCTION" {
        $packs = Join-Path $auditRoot "packs"
        $mission = Join-Path $auditRoot "mission"
        cargo run -q -p ksa64-host --bin phase7_compile -- phase7/source-data $packs
        foreach ($name in @("firestorm54.kvp7", "aerotech-i211w.kmp7", "firestorm-i211.kmc7")) {
            Assert-FileEqual (Join-Path "phase7/examples" $name) (Join-Path $packs $name)
        }
        cargo run -q -p ksa64-host --bin phase7_run -- $packs $mission
        foreach ($name in @("firestorm-i211.kst7", "firestorm-i211.ksr7", "firestorm-i211.kph7")) {
            Assert-FileEqual (Join-Path "phase7/examples" $name) (Join-Path $mission $name)
        }
    }

    Invoke-PhaseGate "PHASE 7 1,024-RUN REFERENCE CAMPAIGN" {
        $serial = Join-Path $auditRoot "serial"
        $parallel = Join-Path $auditRoot "parallel"
        cargo run -q -p ksa64-host --release --bin phase7_campaign -- phase7/examples $serial 1024 1
        cargo run -q -p ksa64-host --release --bin phase7_campaign -- phase7/examples $parallel 1024 4
        foreach ($name in @("campaign-1024.ksc7", "campaign-1024.kra7")) {
            Assert-FileEqual (Join-Path "phase7/examples" $name) (Join-Path $serial $name)
            Assert-FileEqual (Join-Path "phase7/examples" $name) (Join-Path $parallel $name)
        }
        python -B phase7/reference/analyze_campaign.py --ksc phase7/examples/campaign-1024.ksc7 --kra phase7/examples/campaign-1024.kra7 --vehicle phase7/examples/firestorm54.kvp7 --motor phase7/examples/aerotech-i211w.kmp7 --mission phase7/examples/firestorm-i211.kmc7 --output phase7/reference-campaign-analysis.json --check
        $trace = Join-Path $auditRoot "host-trace-v1.json"
        cargo run -q -p ksa64-host --bin phase7_trace | Set-Content -LiteralPath $trace
        Assert-JsonEqual "phase7/host-trace-v1.json" $trace
    }

    if (-not $SkipMos) {
        $versions = Get-Content -Raw -LiteralPath "toolchains/versions.json" | ConvertFrom-Json
        $vice = (Resolve-Path -LiteralPath $versions.vice.projectRelativeExecutable).Path
        $rustWrapper = Join-Path $projectRoot "tools/toolchains/rust-mos.ps1"
        Invoke-PhaseGate "PHASE 7 STOCK-C64 PACKAGING" {
            & $rustWrapper -WorkingDirectory . cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase7-full-c64 --bin ksa64-phase7-replay-c64 --bin ksa64-phase7-trace-c64
            foreach ($name in @("ksa64-phase7-full-c64", "ksa64-phase7-replay-c64", "ksa64-phase7-trace-c64")) {
                Assert-StockPrg (Join-Path "target/mos-c64-none/c64" $name)
            }
            $evidence = Get-Content -Raw -LiteralPath "phase7/c64-execution-v1.json" | ConvertFrom-Json
            foreach ($property in @("full_mission", "replay")) {
                $artifact = $evidence.artifacts.$property
                $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path "target/mos-c64-none/c64" $artifact.path)).Hash.ToLower()
                if ($actual -ne $artifact.sha256) { throw "Frozen C64 artifact hash changed: $($artifact.path)" }
            }
        }

        Invoke-PhaseGate "PHASE 7 FINITE TARGET EXACTNESS AND REPLAY" {
            Assert-NoVice
            python -B phase7/reference/vice_phase7_trace.py --vice $vice --prg target/mos-c64-none/c64/ksa64-phase7-trace-c64 --host phase7/host-trace-v1.json --output phase7/c64-trace-v1.json --check
            Assert-NoVice
            python -B phase7/reference/vice_phase7.py --vice $vice --full-prg target/mos-c64-none/c64/ksa64-phase7-full-c64 --replay-prg target/mos-c64-none/c64/ksa64-phase7-replay-c64 --output phase7/c64-execution-v1.json --check --replay-only
            Assert-NoVice
        }
    }

    Write-Host ""
    if ($SkipMos) { Write-Host "PHASE 7 PARTIAL COMPLETION AUDIT: PASS (MOS/VICE skipped)" }
    else { Write-Host "PHASE 7 COMPLETION AUDIT: PASS" }
} finally {
    Pop-Location
    $resolvedAudit = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolvedAudit.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) {
        throw "Unsafe audit cleanup target: $resolvedAudit"
    }
    if (Test-Path -LiteralPath $resolvedAudit) {
        Remove-Item -LiteralPath $resolvedAudit -Recurse -Force
    }
}
