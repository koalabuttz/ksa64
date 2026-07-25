[CmdletBinding()]
param([switch]$SkipLegacy, [switch]$SkipMos, [switch]$RunVice)
$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$auditRoot = Join-Path $projectRoot (".phase9-audit-" + [Guid]::NewGuid().ToString("N"))
function Check { if ($LASTEXITCODE -ne 0) { throw "command failed: $LASTEXITCODE" } }
function Gate([string]$label, [scriptblock]$action) { Write-Host ""; Write-Host "=== $label ==="; $global:LASTEXITCODE = 0; & $action; Check }
function Equal([string]$a, [string]$b) { if ((Get-FileHash -Algorithm SHA256 -LiteralPath $a).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $b).Hash) { throw "artifact mismatch: $a / $b" } }
function NoVice { if ($p = Get-Process x64sc -ErrorAction SilentlyContinue) { throw "Close VICE PID(s) $($p.Id -join ', ')" } }
New-Item -ItemType Directory -Path $auditRoot | Out-Null
Push-Location $projectRoot
try {
    if (-not $SkipLegacy) { Gate "frozen Phase 0-8.5 audit" { & phase8_5/complete.ps1 -SkipMos } }
    Gate "Phase 9 native audit" {
        cargo fmt --all -- --check; Check
        cargo clippy --workspace --all-targets --features fixtures --target-dir target/phase9 -- -D warnings; Check
        cargo test --workspace --features fixtures --target-dir target/phase9; Check
        cargo build -p ksa64-host --release --bin phase9 --target-dir target/phase9
    }
    Gate "independent accepted-evidence audit" {
        $independent = Join-Path $auditRoot "independent-audit-v1.json"
        python -B phase9/reference/verify_algorithms.py --evidence phase9/evidence --output $independent; Check
        Equal $independent phase9/evidence/independent-audit-v1.json
    }
    Gate "external optimizer protocol" {
        $protocol = Join-Path $auditRoot "external-protocol-v1.json"
        $transcript = Join-Path $auditRoot "external-protocol-v1.jsonl"
        python -B phase9/examples/external_optimizer.py --phase9 target/phase9/release/phase9.exe --manifest phase9/evidence/quick-study-a/manifest.kom9 --transcript $transcript --output $protocol; Check
        Equal $protocol phase9/evidence/external-protocol-v1.json
        Equal $transcript phase9/evidence/external-protocol-v1.jsonl
    }
    Gate "quick worker exactness" {
        foreach ($workers in 1, 4, 8) {
            $out = Join-Path $auditRoot ("quick-study-a-w" + $workers)
            & target/phase9/release/phase9.exe search-kom9 phase9/evidence/quick-study-a/manifest.kom9 $out $workers; Check
            foreach ($name in "finalists.kfp9", "manifest.kom9", "report.csv", "report.html", "report.json", "search.kra9", "sensitivity.ksn9") {
                Equal (Join-Path $out $name) (Join-Path phase9/evidence/quick-study-a $name)
            }
        }
    }
    Gate "stock finalist package" {
        $pack = [IO.File]::ReadAllBytes((Resolve-Path phase9/examples/phase9-finalists.kfp9))
        if ($pack.Length -gt 160KB) { throw "KFP9 exceeds one-volume default" }
        if ([Text.Encoding]::ASCII.GetString($pack, 0, 4) -ne "KFP9") { throw "invalid KFP9 magic" }
    }
    if (-not $SkipMos) {
        Gate "MOS finalist browser packaging" {
            & tools/toolchains/rust-mos.ps1 -WorkingDirectory . cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase9-finalist-c64; Check
            $binary = "target/mos-c64-none/c64/ksa64-phase9-finalist-c64"
            $bytes = [IO.File]::ReadAllBytes((Resolve-Path $binary))
            $load = $bytes[0] + 256 * $bytes[1]
            $end = $load + $bytes.Length - 2
            if ($end -ge 0xC000) { throw ("C64 finalist browser ends at $" + $end.ToString("X4")) }
            if ((Get-FileHash $binary -Algorithm SHA256).Hash.ToLower() -ne "b953f152daafdcf98d15407241f3029f5f9aecfdc222ace08875025c1ffd275d") { throw "C64 finalist browser hash mismatch" }
        }
    }
    if ($RunVice) {
        if ($SkipMos) { throw "-RunVice requires MOS artifacts" }
        Gate "finite VICE finalist-browser probe" {
            NoVice
            $versions = Get-Content toolchains/versions.json -Raw | ConvertFrom-Json
            $vice = (Resolve-Path $versions.vice.projectRelativeExecutable).Path
            python -B phase9/reference/vice_phase9_finalist.py --vice $vice --prg target/mos-c64-none/c64/ksa64-phase9-finalist-c64 --output phase9/reference/vice-finalist.json --check; Check
            NoVice
        }
    }
    Write-Host ""
    Write-Host "PHASE 9 COMPLETION AUDIT: PASS"
} finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($auditRoot)
    if (-not $resolved.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar)) { throw "unsafe audit cleanup" }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
