param(
    [string]$OutputDirectory = "target/phase11-c64-banked"
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$Previous = Get-Location
try {
    Set-Location $Root
    & tools\toolchains\rust-mos.ps1 -ReturnToCaller -WorkingDirectory . bash -lc 'cp /workspace/phase11/c64-banked/mos-c64-banked-linker /tmp/mos-c64-banked-linker && chmod +x /tmp/mos-c64-banked-linker && touch /workspace/sim/src/bin/phase11_reference_ops_endpoint_c64.rs && RUSTFLAGS="-C linker=/tmp/mos-c64-banked-linker -C link-arg=-Wl,-Map=/workspace/target/phase11-reference-banked.map" cargo build --profile c64 --target mos-c64-none --features c64 -Z build-std=core -Z build-std-features=compiler-builtins-mem --bin ksa64-phase11-reference-ops-endpoint-c64'
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    node phase11/c64-banked/package.js target/mos-c64-none/c64/ksa64-phase11-reference-ops-endpoint-c64 $OutputDirectory
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo run -q -p ksa64-host --bin phase11_reference_ops_fixture -- (Join-Path $OutputDirectory "reference-ops-transcript.bin") (Join-Path $OutputDirectory "reference-ops-transcript.json")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Set-Location $Previous
}
