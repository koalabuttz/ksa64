[CmdletBinding()]
param(
    [switch]$SkipLegacy,
    [switch]$SkipMos,
    [switch]$RunVice,
    [switch]$TargetOnly
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase11-audit-" + [Guid]::NewGuid().ToString("N"))

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

function Assert-Hash([string]$path, [string]$sha256) {
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLower()
    if ($actual -ne $sha256) {
        throw "hash mismatch: $path"
    }
}

function Assert-StockArtifact(
    [string]$path,
    [string]$sha256,
    [int]$expectedBytes,
    [int]$expectedRuntimeEnd = 0
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
    if ($expectedRuntimeEnd -ne 0 -and $expectedRuntimeEnd -gt 0xC000) {
        throw "stock runtime footprint crosses `$C000: $path"
    }
    Assert-Hash $resolved $sha256
}

function Assert-SafeholdMap {
    $map = Get-Content target/phase11-safehold-endpoint.map -Raw
    if (
        $map -notmatch "(?m)^\s*8858\s+8858\s+10ea\s+1\s+\.noinit\s*`$" -or
        $map -notmatch "(?m)^\s*9942\s+9942\s+0\s+1\s+__heap_start"
    ) {
        throw "safehold linker-map footprint changed"
    }
}

function Assert-Safehold([object]$actual, [object]$expected) {
    foreach ($field in @(
        "schema", "releases", "failures", "flight_checksum",
        "navigation_checksum", "command_checksum", "journal_chain",
        "drogue_epoch", "main_epoch", "transition_count", "final_frame",
        "safe", "signature"
    )) {
        if ($actual.$field -ne $expected.$field) {
            throw "safehold mismatch: $field"
        }
    }
}

function Expect-Failure([scriptblock]$action, [string]$label) {
    $global:LASTEXITCODE = 0
    & $action
    if ($LASTEXITCODE -eq 0) {
        throw "$label unexpectedly succeeded"
    }
    $global:LASTEXITCODE = 0
}

function Assert-BankedBundle {
    $manifestPath = "target/phase11-c64-banked/reference-ops-banked-manifest.json"
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if (
        $manifest.schema -ne "ksa64.phase11.reference-ops-banked-bundle.v1" -or
        $manifest.bundle_bytes -ne 55423 -or
        $manifest.bundle_sha256 -ne "cb1979fd4e5abf5c26bfbc71ef031ed4d4e0c2d2a9eee4e0522fdc306d9ab377" -or
        $manifest.entry -ne "0x080d" -or
        $manifest.package_state_and_static_stack.bytes -ne 8702 -or
        $manifest.emergency_software_stack.bytes -ne 279 -or
        $manifest.banking.reu_required
    ) {
        throw "banked reference-operations manifest changed"
    }
    foreach ($artifact in $manifest.artifacts) {
        Assert-Hash (Join-Path "target/phase11-c64-banked" $artifact.file) $artifact.sha256
    }
    Assert-Hash `
        "target/phase11-c64-banked/reference-ops-transcript.bin" `
        "4af467b2353a02111a29c3605d70c39af183192c09870075aaab3205cd9e46af"
}

function Assert-StoredEvidence {
    $legacy = Get-Content phase10/completion-audit.json -Raw | ConvertFrom-Json
    if ($legacy.status -ne "pass" -or -not $legacy.compatibility.phase_0_through_9_5) {
        throw "Phase 10 completion record is not accepted"
    }

    $completion = Get-Content phase11/completion-audit.json -Raw | ConvertFrom-Json
    if (
        $completion.status -ne "pass" -or
        -not $completion.compatibility.phase_0_through_10 -or
        -not $completion.compatibility.frozen_artifacts_unchanged
    ) {
        throw "Phase 11 completion record is not accepted"
    }

    $hostSafehold = Get-Content phase11/evidence/host-safehold-v1.json -Raw | ConvertFrom-Json
    $viceSafehold = Get-Content phase11/evidence/vice-safehold-v1.json -Raw | ConvertFrom-Json
    Assert-Safehold $viceSafehold $hostSafehold
    if (
        $viceSafehold.target -ne "PAL stock C64 via pinned x64sc 3.10" -or
        $viceSafehold.warp -or
        $viceSafehold.complete_mission -or
        $viceSafehold.artifact.bytes -ne 28137 -or
        $viceSafehold.artifact.sha256 -ne "c340315bd1ec05138e6252004fe3c76b77f883559239cea837c19989d436a630" -or
        -not $viceSafehold.artifact.stock_fit
    ) {
        throw "stored safehold target evidence changed"
    }

    $sdk = Get-Content phase11/evidence/session-sdk-v1.json -Raw | ConvertFrom-Json
    if (
        $sdk.session.definition_identity -ne "0xc5884398" -or
        $sdk.session.action_identity -ne "0x7ea2c50a" -or
        $sdk.session.completed_evidence_identity -ne "0x6d4122a0" -or
        $sdk.session.validated_bytes -ne 22369 -or
        $sdk.session.segment_count -ne 17 -or
        $sdk.session.manifest_sha256 -ne "6db058a1af462090ec0c4b32391518748ccc2486feb1a18408d4eb02e3066486" -or
        $sdk.session.file_sha256 -ne "38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8"
    ) {
        throw "stored session SDK evidence changed"
    }

    $fit = Get-Content phase11/evidence/reference-ops-stock-fit-boundary-v1.json -Raw | ConvertFrom-Json
    $banked = Get-Content phase11/evidence/reference-ops-banked-vice-v1.json -Raw | ConvertFrom-Json
    if (
        $fit.status -ne "resolved-by-authorized-banked-stopgap" -or
        $fit.accepted_safehold_endpoint.sha256 -ne "353ecad4d65030233509db14fd30ee2ef02040a1a4320938043242e571eab779" -or
        $fit.accepted_banked_stopgap.bundle_sha256 -ne "cb1979fd4e5abf5c26bfbc71ef031ed4d4e0c2d2a9eee4e0522fdc306d9ab377" -or
        $banked.warp -or
        $banked.reu_required -or
        $banked.records -ne 13 -or
        $banked.emergency_stack_high_water_bytes -ne 16 -or
        -not $banked.segment_guards_preserved -or
        -not $banked.code_segments_preserved -or
        $banked.navigation_checksum -ne "c73060d2" -or
        $banked.flight_checksum -ne "6e07595c" -or
        $banked.command_checksum -ne "6ab926f2"
    ) {
        throw "stored reference-operations target evidence changed"
    }

    $layout = Get-Content phase11/evidence/safehold-target-layout-v1.json -Raw |
        ConvertFrom-Json
    if (
        $layout.package_resource_evidence_sha256 -ne "6f73177936f0e65362ad453fe38739f2948c4a2602a37e5fb45c73e76c04c405" -or
        $layout.artifact_sha256 -ne "353ecad4d65030233509db14fd30ee2ef02040a1a4320938043242e571eab779" -or
        $layout.compiler_static_stack.bytes -ne 4330 -or
        $layout.compiler_static_stack.runtime_end_exclusive -ne "0x9942" -or
        $layout.margin_before_c000_bytes -ne 9918 -or
        -not $layout.stock_fit -or
        $layout.reu_required
    ) {
        throw "safehold target-layout evidence changed"
    }
}

New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    if (-not $SkipLegacy -and -not $TargetOnly) {
        Gate "frozen Phase 0-10 audit" {
            & phase10/complete.ps1 -SkipLegacy -SkipMos
        }
    }

    if (-not $TargetOnly) {
        Gate "Phase 11 native audit" {
            cargo fmt --all -- --check
            Check
            cargo clippy --workspace --all-targets --features fixtures --target-dir target/phase11 -- -D warnings
            Check
            cargo test --workspace --features fixtures --target-dir target/phase11
            Check
            cargo build -p ksa64-host --release `
                --bin phase11 `
                --bin phase11_mission_control `
                --bin phase11_reference_ops_fixture `
                --target-dir target/phase11
            Check
            cargo build -p ksa64-sim --release --features sim `
                --bin ksa64-phase11-safehold-reference `
                --target-dir target/phase11
        }

        Gate "prediction and portable safehold exactness" {
            python -B phase11/reference/prediction_vectors.py --check
            Check
            $actual = (& target/phase11/release/ksa64-phase11-safehold-reference.exe | Out-String) |
                ConvertFrom-Json
            $expected = Get-Content phase11/evidence/host-safehold-v1.json -Raw | ConvertFrom-Json
            Assert-Safehold $actual $expected
        }

        Gate "headless mission SDK, replay, and corruption boundaries" {
            $sdk = "target/phase11/release/phase11.exe"
            $source = "phase11/examples/gnss-loss.json"
            $safeholdSource = "phase11/examples/safehold-recovery.json"
            $definition = Join-Path $auditRoot "definition.ksb11"
            $sessionA = Join-Path $auditRoot "session-a.ksb11"
            $sessionB = Join-Path $auditRoot "session-b.ksb11"
            $safeholdSession = Join-Path $auditRoot "safehold.ksb11"
            $reports = Join-Path $auditRoot "reports"

            & $sdk lint $source
            Check
            & $sdk compile $source $definition
            Check
            & $sdk inspect $definition
            Check
            & $sdk script $source $sessionA
            Check
            & $sdk script $source $sessionB
            Check
            $hashA = (Get-FileHash -Algorithm SHA256 -LiteralPath $sessionA).Hash.ToLower()
            $hashB = (Get-FileHash -Algorithm SHA256 -LiteralPath $sessionB).Hash.ToLower()
            if (
                $hashA -ne $hashB -or
                $hashA -ne "38a3ef2e497b8e24d1cf53a56db85b3d8bea0bdb27586215a02ff75d0ee39dc8"
            ) {
                throw "scripted session bundle is not frozen and deterministic"
            }
            & $sdk verify $sessionA
            Check
            & $sdk replay $sessionA
            Check
            & $sdk debrief $sessionA $reports
            Check

            & $sdk lint $safeholdSource
            Check
            & $sdk script $safeholdSource $safeholdSession
            Check
            & $sdk verify $safeholdSession
            Check

            $bytes = [IO.File]::ReadAllBytes($sessionA)
            $truncated = Join-Path $auditRoot "truncated.ksb11"
            $corrupt = Join-Path $auditRoot "corrupt.ksb11"
            [IO.File]::WriteAllBytes($truncated, $bytes[0..($bytes.Length - 2)])
            $bytes[128] = $bytes[128] -bxor 1
            [IO.File]::WriteAllBytes($corrupt, $bytes)
            Expect-Failure { & $sdk verify $truncated } "truncated session"
            Expect-Failure { & $sdk verify $corrupt } "corrupt session"
        }

        Gate "stored Phase 11 evidence" {
            Assert-StoredEvidence
        }
    } else {
        Gate "stored Phase 11 evidence" {
            Assert-StoredEvidence
        }
    }

    if (-not $SkipMos) {
        Gate "MOS packaging and stock-memory boundaries" {
            & tools/toolchains/rust-mos.ps1 -ReturnToCaller -WorkingDirectory . cargo build `
                --profile c64 `
                --target mos-c64-none `
                --features c64 `
                -Z build-std=core `
                -Z build-std-features=compiler-builtins-mem `
                --bin ksa64-phase11-safehold-probe-c64 `
                --bin ksa64-phase11-safehold-endpoint-c64
            Check
            & tools/toolchains/rust-mos.ps1 -ReturnToCaller -WorkingDirectory . bash -lc 'RUSTFLAGS="-C link-arg=-Wl,-Map=/workspace/target/phase11-safehold-endpoint.map" cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase11-safehold-endpoint-c64'
            Check
            Assert-SafeholdMap
            Assert-StockArtifact `
                target/mos-c64-none/c64/ksa64-phase11-safehold-probe-c64 `
                "c340315bd1ec05138e6252004fe3c76b77f883559239cea837c19989d436a630" `
                28137
            Assert-StockArtifact `
                target/mos-c64-none/c64/ksa64-phase11-safehold-endpoint-c64 `
                "353ecad4d65030233509db14fd30ee2ef02040a1a4320938043242e571eab779" `
                32857 `
                0x9942

            & phase11/c64-banked/build.ps1
            Check
            Assert-BankedBundle
        }
    }

    if ($RunVice) {
        if ($SkipMos) {
            throw "-RunVice requires MOS artifacts"
        }
        $versions = Get-Content toolchains/versions.json -Raw | ConvertFrom-Json
        $vice = (Resolve-Path $versions.vice.projectRelativeExecutable).Path

        Gate "finite one-instance VICE probes" {
            NoVice
            $safeholdActual = Join-Path $auditRoot "vice-safehold.json"
            python -B phase11/reference/vice_safehold.py `
                --vice $vice `
                --prg target/mos-c64-none/c64/ksa64-phase11-safehold-probe-c64 `
                --expected phase11/evidence/host-safehold-v1.json `
                --output $safeholdActual
            Check
            NoVice
            $actual = Get-Content $safeholdActual -Raw | ConvertFrom-Json
            $expected = Get-Content phase11/evidence/vice-safehold-v1.json -Raw | ConvertFrom-Json
            Assert-Safehold $actual $expected
            if (
                $actual.artifact.sha256 -ne $expected.artifact.sha256 -or
                $actual.warp -or $actual.complete_mission
            ) {
                throw "safehold VICE evidence changed"
            }

            Start-Sleep -Seconds 20
            NoVice
            $bankedActual = Join-Path $auditRoot "vice-reference-ops.json"
            python -B phase11/reference/vice_reference_ops_banked.py `
                --vice $vice `
                --image-dir target/phase11-c64-banked `
                --transcript target/phase11-c64-banked/reference-ops-transcript.bin `
                --output $bankedActual
            Check
            NoVice
            $actualBanked = Get-Content $bankedActual -Raw | ConvertFrom-Json
            $expectedBanked = Get-Content phase11/evidence/reference-ops-banked-vice-v1.json -Raw |
                ConvertFrom-Json
            foreach ($field in @(
                "schema", "target", "warp", "reu_required", "records",
                "transcript_sha256", "bundle_sha256", "entry",
                "emergency_stack_capacity_bytes", "emergency_stack_high_water_bytes",
                "segment_guards_preserved", "code_segments_preserved", "final_epoch",
                "navigation_checksum", "flight_checksum", "command_checksum",
                "timing_claim", "complete_mission"
            )) {
                if ($actualBanked.$field -ne $expectedBanked.$field) {
                    throw "banked VICE mismatch: $field"
                }
            }
        }
    }

    Write-Host ""
    Write-Host "PHASE 11 COMPLETION AUDIT: PASS"
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
