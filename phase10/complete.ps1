[CmdletBinding()]
param(
    [switch]$SkipLegacy,
    [switch]$SkipMos,
    [switch]$RunVice,
    [switch]$TargetOnly
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase10-audit-" + [Guid]::NewGuid().ToString("N"))

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

function NoVice {
    $processes = Get-Process -Name x64sc, x64 -ErrorAction SilentlyContinue
    if ($processes) {
        throw "Close VICE PID(s) $($processes.Id -join ', ')"
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

function Assert-Hash([string]$path, [string]$sha256) {
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLower()
    if ($actual -ne $sha256) {
        throw "hash mismatch: $path"
    }
}

function Assert-FlightProbe([string]$actualPath, [string]$expectedPath) {
    $actual = Get-Content -LiteralPath $actualPath -Raw | ConvertFrom-Json
    $expected = Get-Content -LiteralPath $expectedPath -Raw | ConvertFrom-Json
    foreach ($field in "schema", "releases", "transition_probe", "target", "result_raw") {
        if ($actual.$field -ne $expected.$field) {
            throw "flight probe mismatch: $field"
        }
    }
    foreach ($field in "bytes", "sha256", "load_address", "load_end_exclusive", "stock_fit") {
        if ($actual.artifact.$field -ne $expected.artifact.$field) {
            throw "flight artifact mismatch: $field"
        }
    }
    $actualLine = @($actual.broker -split "`r?`n")[-1]
    $expectedLine = @($expected.broker -split "`r?`n")[-1]
    if ($actualLine -ne $expectedLine) {
        throw "flight checksum-chain mismatch"
    }
}

function Assert-StoredEvidence {
    $completion = Get-Content phase10/completion-audit.json -Raw | ConvertFrom-Json
    if ($completion.status -ne "pass" -or -not $completion.compatibility.phase_0_through_9_5) {
        throw "completion record is not accepted"
    }

    $artifacts = Get-Content phase10/evidence/artifact-manifest-v1.json -Raw | ConvertFrom-Json
    foreach ($property in $artifacts.files.PSObject.Properties) {
        Assert-Hash (Join-Path phase10/evidence $property.Name) $property.Value
    }

    Assert-Hash phase10/campaign/ksa-g10r-64.kra10 `
        "2cc8e089ecfbc6f470ef61cd2aca684e53dc1b4a9bcdb1f2dd821c85714c05d1"
    Assert-Hash phase10/campaign/ksa-g10r-256.kra10 `
        "18c56e7537a8393376e0444033170319f74449b51826a4916cce74c5bc2f4daf"

    $routine = Get-Content phase10/campaign/ksa-g10r-64.json -Raw | ConvertFrom-Json
    $accepted = Get-Content phase10/campaign/ksa-g10r-256.json -Raw | ConvertFrom-Json
    if (
        $routine.runs -ne 64 -or $routine.physical_recoveries -ne 64 -or
        $accepted.runs -ne 256 -or $accepted.physical_recoveries -ne 256 -or
        $accepted.numeric_frame_time_faults -ne 0 -or
        $accepted.model_envelope_exceeded -ne 0
    ) {
        throw "campaign acceptance changed"
    }

    $release = Get-Content phase10/evidence/vice-global-flight-release-classes-v1.json -Raw | ConvertFrom-Json
    $transition = Get-Content phase10/evidence/vice-global-flight-transitions-v1.json -Raw | ConvertFrom-Json
    $replay = Get-Content phase10/evidence/vice-stock-replay-v1.json -Raw | ConvertFrom-Json
    $timing = Get-Content phase10/evidence/vice-global-flight-timing-v1.json -Raw | ConvertFrom-Json
    if (
        $release.releases -ne 33 -or
        $transition.releases -ne 5 -or
        -not $transition.transition_probe -or
        $replay.transition_mask -ne "0f" -or
        $replay.points -ne 128 -or
        $replay.warp -or
        $timing.warp -or
        $timing.realtime_requirement -or
        $timing.cycles.worst -ne 3512697
    ) {
        throw "stored target evidence changed"
    }
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    if (-not $SkipLegacy -and -not $TargetOnly) {
        Gate "frozen Phase 0-9.5 audit" {
            & phase9_5/complete.ps1 -SkipMos
        }
    }

    if (-not $TargetOnly) {
        Gate "Phase 10 native audit" {
            cargo fmt --all -- --check
            Check
            cargo clippy --workspace --all-targets --features fixtures --target-dir target/phase10 -- -D warnings
            Check
            cargo test --workspace --features fixtures --target-dir target/phase10
            Check
            cargo build -p ksa64-host --release `
                --bin phase10_launch `
                --bin phase10_bridge `
                --bin phase10_campaign `
                --bin phase10_world_reference `
                --target-dir target/phase10
        }

        Gate "Earth, time, frame, environment, and vehicle fixtures" {
            python -B phase10/reference/check_numeric.py --check
            Check
            python -B phase10/reference/generate_frames.py --check
            Check
            python -B phase10/reference/generate_environment.py --check
            Check
            python -B phase10/reference/compile_vehicle.py --check
        }

        Gate "independent trajectory and orbital corroboration" {
            $worldObject = (& target/phase10/release/phase10_world_reference.exe | Out-String) | ConvertFrom-Json
            $expectedObject = Get-Content phase10/generated/uninstrumented-exact-v1.json -Raw | ConvertFrom-Json
            $worldCanonical = $worldObject | ConvertTo-Json -Depth 12 -Compress
            $expectedCanonical = $expectedObject | ConvertTo-Json -Depth 12 -Compress
            if ($worldCanonical -ne $expectedCanonical) {
                throw "production uninstrumented reference changed"
            }
            python -B phase10/reference/analyze_nominal.py --check
            Check
            python -B phase10/reference/analyze_ksa5_coast.py --check
        }

        Gate "nominal, campaign, report, and replay integrity" {
            python -B phase10/reference/build_artifact_manifest.py --check
            Check
            python -B phase10/reference/build_stock_replay.py --check
            Check
            python -B phase10/reference/finalize_campaign.py --check
            Assert-StoredEvidence
        }
    } else {
        Gate "stored Phase 10 evidence" {
            Assert-StoredEvidence
        }
    }

    if (-not $SkipMos) {
        Gate "MOS packaging" {
            & tools/toolchains/rust-mos.ps1 -WorkingDirectory . cargo build `
                --profile c64 `
                --target mos-c64-none `
                --features c64 `
                -Z build-std=core `
                -Z build-std-features=compiler-builtins-mem `
                --bin ksa64-phase10-flight-endpoint-c64 `
                --bin ksa64-phase10-flight-timed-c64 `
                --bin ksa64-phase10-replay-c64
            Check
            Assert-StockArtifact `
                target/mos-c64-none/c64/ksa64-phase10-flight-endpoint-c64 `
                "48d41a09ec0cc3e8a2699dee5111fa7ffff46bc8d15d2783c8becbdcd8e8c59b" `
                37403
            Assert-StockArtifact `
                target/mos-c64-none/c64/ksa64-phase10-flight-timed-c64 `
                "081612e97c330f01b42635f64643148ff8bcc0109bd516d2a8c6ff4fbb97461d" `
                35247
            Assert-StockArtifact `
                target/mos-c64-none/c64/ksa64-phase10-replay-c64 `
                "e85d710e9e31dda9a28a37a8d42f6ee50c93ba8d18ca4ea8710db76e1c1c673e" `
                17002
        }
    }

    if ($RunVice) {
        if ($SkipMos) {
            throw "-RunVice requires MOS artifacts"
        }
        Gate "build host bridge for finite target probes" {
            cargo build -p ksa64-host --release --bin phase10_bridge --target-dir target/phase10
        }
        $versions = Get-Content toolchains/versions.json -Raw | ConvertFrom-Json
        $vice = (Resolve-Path $versions.vice.projectRelativeExecutable).Path
        $flight = "target/mos-c64-none/c64/ksa64-phase10-flight-endpoint-c64"
        $timed = "target/mos-c64-none/c64/ksa64-phase10-flight-timed-c64"
        $replay = "target/mos-c64-none/c64/ksa64-phase10-replay-c64"
        $bridge = "target/phase10/release/phase10_bridge.exe"

        Gate "finite one-instance VICE probes" {
            NoVice
            python -B phase10/reference/vice_stock_replay.py `
                --vice $vice --prg $replay `
                --output phase10/evidence/vice-stock-replay-v1.json --check
            Check
            NoVice
            Start-Sleep -Seconds 20

            $releaseActual = Join-Path $auditRoot "release-classes.json"
            python -B phase10/reference/vice_global_flight.py `
                --vice $vice --prg $flight --broker $bridge `
                --max-releases 33 --output $releaseActual
            Check
            NoVice
            Assert-FlightProbe $releaseActual phase10/evidence/vice-global-flight-release-classes-v1.json
            Start-Sleep -Seconds 20

            $transitionActual = Join-Path $auditRoot "transitions.json"
            python -B phase10/reference/vice_global_flight.py `
                --vice $vice --prg $flight --broker $bridge `
                --transition-probe --max-releases 5 --output $transitionActual
            Check
            NoVice
            Assert-FlightProbe $transitionActual phase10/evidence/vice-global-flight-transitions-v1.json
            Start-Sleep -Seconds 20

            python -B phase10/reference/vice_global_flight_timing.py `
                --vice $vice --prg $timed `
                --output phase10/evidence/vice-global-flight-timing-v1.json --check
            Check
            NoVice
        }
    }

    Write-Host ""
    Write-Host "PHASE 10 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) {
        throw "unsafe audit cleanup"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
