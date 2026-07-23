[CmdletBinding()]
param([switch]$SkipMos)

$ErrorActionPreference = "Stop"

function Invoke-Gate([scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Gate command failed with exit code $LASTEXITCODE"
    }
}

Invoke-Gate { python -B phase5/reference/generate_contract.py --check }
Invoke-Gate { python -B phase5/reference/generate_spatial_vectors.py --check }
Invoke-Gate { python -B phase5/reference/generate_rigid_body_vectors.py --check }
Invoke-Gate { python -B phase5/reference/generate_flexible_vectors.py --check }
Invoke-Gate { cargo fmt --all -- --check }
Invoke-Gate { cargo check --workspace --all-targets --features fixtures }
Invoke-Gate { cargo clippy --workspace --all-targets --features fixtures -- -D warnings -A clippy::result-unit-err -A clippy::manual-is-multiple-of -A clippy::manual-flatten -A clippy::needless-range-loop -A clippy::drop-non-drop -A clippy::too-many-arguments }
Invoke-Gate { cargo test --workspace --features fixtures }
if (-not $SkipMos) {
    $rustWrapper = Join-Path (Get-Location) "tools/toolchains/rust-mos.ps1"
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --release `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-spatial-sim
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/release/ksa64-phase5-spatial-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --release `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-rigid-sim
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/release/ksa64-phase5-rigid-sim"
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . cargo build --release `
            --target mos-sim-none --features sim `
            -Z build-std=core `
            -Z build-std-features=compiler-builtins-mem `
            --bin ksa64-phase5-flexible-sim
    }
    Invoke-Gate {
        & $rustWrapper -WorkingDirectory . sh -lc `
            "mos-sim target/mos-sim-none/release/ksa64-phase5-flexible-sim"
    }
}