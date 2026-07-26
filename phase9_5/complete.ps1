[CmdletBinding()]
param([switch]$SkipLegacy, [switch]$SkipMos, [switch]$RunVice, [switch]$TargetOnly)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase9-5-audit-" + [Guid]::NewGuid().ToString("N"))

function Check {
    if ($LASTEXITCODE -ne 0) {
        throw "command failed: $LASTEXITCODE"
    }
}

function Gate([string]$label, [scriptblock]$action) {
    Write-Host ""
    Write-Host "=== $label ==="
    $global:LASTEXITCODE = 0
    & $action
    Check
}

function Equal([string]$actual, [string]$expected) {
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $actual).Hash
    $expectedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $expected).Hash
    if ($actualHash -ne $expectedHash) {
        throw "artifact mismatch: $actual / $expected"
    }
}

function NoVice {
    if ($process = Get-Process x64sc -ErrorAction SilentlyContinue) {
        throw "Close VICE PID(s) $($process.Id -join ', ')"
    }
}

function Assert-StockArtifact(
    [string]$path,
    [string]$sha256,
    [int]$expectedBytes
) {
    $resolved = Resolve-Path -LiteralPath $path
    $bytes = [IO.File]::ReadAllBytes($resolved)
    $load = $bytes[0] + 256 * $bytes[1]
    $end = $load + $bytes.Length - 2
    if ($bytes.Length -ne $expectedBytes) {
        throw "unexpected artifact size: $path ($($bytes.Length), expected $expectedBytes)"
    }
    if ($end -gt 0xC000) {
        throw "stock artifact crosses `$C000: $path ends at $($end.ToString('X4'))"
    }
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $resolved).Hash.ToLower()
    if ($actualHash -ne $sha256) {
        throw "artifact hash mismatch: $path"
    }
}

function Assert-WorkbenchReport([string]$actualPath, [string]$expectedPath) {
    $actual = Get-Content -LiteralPath $actualPath -Raw | ConvertFrom-Json
    $expected = Get-Content -LiteralPath $expectedPath -Raw | ConvertFrom-Json
    foreach ($field in "schema", "seed", "smoke", "campaign_crc32") {
        if ($actual.$field -ne $expected.$field) {
            throw "workbench report mismatch: $field"
        }
    }
    if ($actual.studies.Count -ne $expected.studies.Count) {
        throw "workbench study-count mismatch"
    }
    $fields = @(
        "study", "engine", "manifest", "generations", "evaluations",
        "cache_hits", "finalists", "feasible_finalists", "archive_bytes",
        "archive_crc32", "finalist_bytes"
    )
    for ($index = 0; $index -lt $actual.studies.Count; $index++) {
        foreach ($field in $fields) {
            if ($actual.studies[$index].$field -ne $expected.studies[$index].$field) {
                throw "workbench report mismatch: study $index field $field"
            }
        }
    }
}

function Assert-Probe([string]$actualPath, [string]$expectedPath) {
    $actual = Get-Content -LiteralPath $actualPath -Raw | ConvertFrom-Json
    $expected = Get-Content -LiteralPath $expectedPath -Raw | ConvertFrom-Json
    foreach ($field in "schema", "releases", "target", "result_raw") {
        if ($actual.$field -ne $expected.$field) {
            throw "VICE probe mismatch: $field"
        }
    }
    foreach ($field in "bytes", "sha256", "load_address", "load_end_exclusive", "stock_fit") {
        if ($actual.artifact.$field -ne $expected.artifact.$field) {
            throw "VICE artifact mismatch: $field"
        }
    }
    $actualBounded = @($actual.broker -split "`r?`n")[-1]
    $expectedBounded = @($expected.broker -split "`r?`n")[-1]
    if ($actualBounded -ne $expectedBounded) {
        throw "VICE broker checksum mismatch"
    }
    foreach ($field in "finalist_index", "package", "bootstrap_sha256") {
        if ($null -ne $expected.$field -and $actual.$field -ne $expected.$field) {
            throw "VICE finalist mismatch: $field"
        }
    }
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    if (-not $SkipLegacy -and -not $TargetOnly) {
        Gate "frozen Phase 0-9 audit" {
            & phase9/complete.ps1 -SkipLegacy -SkipMos
        }
    }

    if (-not $TargetOnly) {
    Gate "Phase 9.5 native audit" {
        cargo fmt --all -- --check
        Check
        cargo clippy --workspace --all-targets --features fixtures --target-dir target/phase95 -- -D warnings
        Check
        cargo test --workspace --features fixtures --target-dir target/phase95
        Check
        cargo build -p ksa64-host --release --bin phase9_5_workbench --bin phase9_5_bridge --bin phase9_5_finalist_bridge --target-dir target/phase95
    }

    Gate "independent model and contract audit" {
        python -B phase9_5/reference/generate_numeric.py --check
        Check
        python -B phase9_5/reference/generate_contract_vectors.py --check
        Check
        python -B phase9_5/reference/generate_canard_vectors.py --check --report
        Check
        python -B phase9_5/reference/generate_rcs_vectors.py --check --report
        Check
        python -B phase9_5/reference/generate_allocator_vectors.py --check --report
        Check
        python -B phase9_5/reference/verify_reference_packs.py
        Check
        python -B phase9_5/reference/build_integrated_manifest.py --check
        Check
        python -B phase9_5/reference/analyze_integrated_float.py --check
    }

    Gate "accepted campaign and search reproduction" {
        $workbench = Join-Path $auditRoot "workbench"
        & target/phase95/release/phase9_5_workbench.exe $workbench 8
        Check
        foreach ($name in @(
            "mixed-64-campaign.ksc9-kas9",
            "canard-grid.kae9", "canard-grid.kfe9",
            "canard-nsga2.kae9", "canard-nsga2.kfe9",
            "rcs-grid.kae9", "rcs-grid.kfe9",
            "rcs-nsga2.kae9", "rcs-nsga2.kfe9",
            "mixed-grid.kae9", "mixed-grid.kfe9",
            "mixed-nsga2.kae9", "mixed-nsga2.kfe9",
            "research-nsga2.kae9", "research-nsga2.kfe9"
        )) {
            Equal (Join-Path $workbench $name) (Join-Path phase9_5/evidence/workbench $name)
        }
        Assert-WorkbenchReport `
            (Join-Path $workbench "accepted-report.json") `
            "phase9_5/evidence/workbench/accepted-report.json"
    }
    }

    Gate "checked target and presentation evidence" {
        $boundary = Get-Content phase9_5/evidence/stock-target-boundary.json -Raw | ConvertFrom-Json
        if ($boundary.decision -ne "stop_at_explicit_gate10_boundary") {
            throw "unexpected stock-target decision"
        }
        if (-not $boundary.flight_endpoint.fit_pass -or $boundary.flight_endpoint.realtime_pass) {
            throw "stock flight boundary changed"
        }
        if ($boundary.world_endpoint.fit_pass) {
            throw "portable stock world unexpectedly accepted"
        }
        $missionControl = Get-Content phase9_5/evidence/mission-control-gate-v1.json -Raw | ConvertFrom-Json
        if (-not $missionControl.presentation.passive_render_invariance -or $missionControl.host_host_releases -ne 64) {
            throw "Mission Control evidence failed"
        }
        foreach ($name in "finalist-split-canard-v1.json", "finalist-split-rcs-v1.json", "finalist-split-mixed-v1.json") {
            $probe = Get-Content (Join-Path phase9_5/evidence $name) -Raw | ConvertFrom-Json
            if ($probe.releases -ne 8 -or -not $probe.artifact.stock_fit) {
                throw "invalid finalist VICE evidence: $name"
            }
        }
    }

    if (-not $SkipMos) {
        Gate "MOS packaging and finite exact probes" {
            & tools/toolchains/rust-mos.ps1 -WorkingDirectory . cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem `
                --bin ksa64-phase9-5-contract-c64 `
                --bin ksa64-phase9-5-canard-c64 `
                --bin ksa64-phase9-5-rcs-c64 `
                --bin ksa64-phase9-5-avionics-probe-c64 `
                --bin ksa64-phase9-5-allocator-probe-c64 `
                --bin ksa64-phase9-5-flight-endpoint-c64 `
                --bin ksa64-phase9-5-finalist-flight-endpoint-c64 `
                --bin ksa64-phase9-5-finalist-c64
            Check
            Assert-StockArtifact `
                "target/mos-c64-none/c64/ksa64-phase9-5-flight-endpoint-c64" `
                "2cbffcc866fa857f6eac1fb47efc7315adea5dd5fc7ec14f5d37d538742a3d8f" `
                44306
            Assert-StockArtifact `
                "target/mos-c64-none/c64/ksa64-phase9-5-finalist-flight-endpoint-c64" `
                "ea1c315aa44abccfbc112601319fa11997abcc2f351b47844e01399d2ff23597" `
                39963
            Assert-StockArtifact `
                "target/mos-c64-none/c64/ksa64-phase9-5-finalist-c64" `
                "adb685508b154db1c28c7ef6753f22eefa3c9f997cd4408febb4240ce887d0cd" `
                29010
        }
    }

    if ($RunVice) {
        if ($SkipMos) {
            throw "-RunVice requires MOS artifacts"
        }
        $versions = Get-Content toolchains/versions.json -Raw | ConvertFrom-Json
        $vice = (Resolve-Path $versions.vice.projectRelativeExecutable).Path
        $baseline = "target/mos-c64-none/c64/ksa64-phase9-5-flight-endpoint-c64"
        $finalist = "target/mos-c64-none/c64/ksa64-phase9-5-finalist-flight-endpoint-c64"
        $browser = "target/mos-c64-none/c64/ksa64-phase9-5-finalist-c64"
        $bridge = "target/phase95/release/phase9_5_bridge.exe"
        $finalistBridge = "target/phase95/release/phase9_5_finalist_bridge.exe"

        Gate "finite one-instance VICE evidence" {
            NoVice
            python -B phase9_5/reference/vice_finalist_browser.py `
                --vice $vice --prg $browser `
                --output phase9_5/evidence/finalist-browser-v1.json --check
            Check
            NoVice
            Start-Sleep -Seconds 20

            $baselineOutput = Join-Path $auditRoot "baseline.json"
            python -B phase9_5/reference/vice_mailbox_bridge.py `
                --vice $vice --prg $baseline --broker $bridge `
                --max-releases 8 --output $baselineOutput
            Check
            NoVice
            Assert-Probe $baselineOutput "phase9_5/evidence/split-flight-v1.json"
            Start-Sleep -Seconds 20

            foreach ($study in "canard", "rcs", "mixed") {
                $actual = Join-Path $auditRoot ("finalist-" + $study + ".json")
                python -B phase9_5/reference/vice_finalist_split.py `
                    --vice $vice --prg $finalist --broker $finalistBridge `
                    --package ("phase9_5/evidence/workbench/" + $study + "-nsga2.kfe9") `
                    --index 0 --max-releases 8 --output $actual
                Check
                NoVice
                Assert-Probe $actual ("phase9_5/evidence/finalist-split-" + $study + "-v1.json")
                Start-Sleep -Seconds 20
            }
        }
    }

    Write-Host ""
    Write-Host "PHASE 9.5 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) {
        throw "unsafe audit cleanup"
    }
    if (Test-Path -LiteralPath $resolved) {
        $cleanupAttempts = 0
        while (Test-Path -LiteralPath $resolved) {
            try {
                Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction Stop
            } catch {
                $cleanupAttempts += 1
                if ($cleanupAttempts -ge 8) {
                    throw
                }
                Start-Sleep -Milliseconds 250
            }
        }
    }
}
