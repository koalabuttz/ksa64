$ErrorActionPreference = "Stop"

function Invoke-Gate([scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Gate command failed with exit code $LASTEXITCODE"
    }
}

Invoke-Gate { python -B phase5/reference/generate_contract.py --check }
Invoke-Gate { cargo fmt --all -- --check }
Invoke-Gate { cargo check --workspace --all-targets --features fixtures }
Invoke-Gate { cargo clippy --workspace --all-targets --features fixtures -- -D warnings -A clippy::result-unit-err -A clippy::manual-is-multiple-of -A clippy::manual-flatten -A clippy::needless-range-loop -A clippy::drop-non-drop -A clippy::too-many-arguments }
Invoke-Gate { cargo test --workspace --features fixtures }