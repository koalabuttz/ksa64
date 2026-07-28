#!/usr/bin/env sh
set -eu

FINAL_OUTPUT=${1:-evidence/phase12b5/duet}
STAGING_OUTPUT=$(mktemp -d)
trap 'rm -rf "$STAGING_OUTPUT"' EXIT HUP INT TERM
OUTPUT=$STAGING_OUTPUT
case "$(uname -m)" in
  aarch64|arm64) ;;
  *) echo "Duet acceptance requires a physical ARM64 host; got $(uname -m)" >&2; exit 2 ;;
esac

mkdir -p "$OUTPUT"
{
  echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "kernel=$(uname -a)"
  echo "rustc=$(rustc --version)"
  echo "cargo=$(cargo --version)"
  echo "node=$(node --version)"
  echo "npm=$(npm --version)"
  echo "commit=$(git rev-parse HEAD)"
  echo "logical_cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc)"
  echo "memory_bytes=$(awk '/MemTotal/ { print $2 * 1024 }' /proc/meminfo)"
  echo "storage_available_bytes=$(df -B1 --output=avail "$OUTPUT" | tail -n 1 | tr -d ' ')"
} > "$OUTPUT/toolchain.txt"

cargo build --release --locked -p ksa64-host --bin ksa64
cargo build --release --locked -p ksa64-session --example exact_gnss_loss
if command -v /usr/bin/time >/dev/null 2>&1; then
  /usr/bin/time -v -o "$OUTPUT/startup-time.txt" \
    target/release/ksa64 catalog list --json > "$OUTPUT/catalog.json"
else
  target/release/ksa64 catalog list --json > "$OUTPUT/catalog.json"
  echo "GNU time unavailable; startup RSS pending" > "$OUTPUT/startup-time.txt"
fi
NATIVE_KSB="$OUTPUT/gnss-loss-native.ksb11"
if command -v /usr/bin/time >/dev/null 2>&1; then
  /usr/bin/time -v -o "$OUTPUT/native-time.txt" \
    target/release/examples/exact_gnss_loss "$NATIVE_KSB" > "$OUTPUT/native-result.json"
else
  target/release/examples/exact_gnss_loss "$NATIVE_KSB" > "$OUTPUT/native-result.json"
  echo "GNU time unavailable; peak RSS pending" > "$OUTPUT/native-time.txt"
fi
NATIVE_BYTES=$(wc -c < "$NATIVE_KSB" | tr -d ' ')
NATIVE_SHA=$(sha256sum "$NATIVE_KSB" | cut -d ' ' -f 1)
test "$NATIVE_BYTES" = "2911464"
test "$NATIVE_SHA" = "7554111f28d8f3628ae3ca9d069fad34204e12f86252efd00ecf744c0ee0fcd4"

if command -v /usr/bin/time >/dev/null 2>&1; then
  /usr/bin/time -v -o "$OUTPUT/archive-write-time.txt" \
    sh phase12/package-native.sh --output "$OUTPUT/native-package"
else
  sh phase12/package-native.sh --output "$OUTPUT/native-package"
  echo "GNU time unavailable; archive timing pending" > "$OUTPUT/archive-write-time.txt"
fi

cargo build --manifest-path session-wasm/Cargo.toml --target wasm32-unknown-unknown --release --locked
WASM=target/wasm32-unknown-unknown/release/ksa64_session_wasm.wasm
if command -v /usr/bin/time >/dev/null 2>&1; then
  /usr/bin/time -v -o "$OUTPUT/wasm-time.txt" \
    node session-wasm/tools/harness.mjs "$WASM" > "$OUTPUT/wasm-result.json"
else
  node session-wasm/tools/harness.mjs "$WASM" > "$OUTPUT/wasm-result.json"
  echo "GNU time unavailable; peak RSS pending" > "$OUTPUT/wasm-time.txt"
fi

npm ci --prefix web
npm test --prefix web
npm run build --prefix web

{
  echo "evidence_bytes=$(du -sb "$OUTPUT" | cut -f 1)"
  echo "native_package_bytes=$(du -sb "$OUTPUT/native-package" | cut -f 1)"
  echo "web_dist_bytes=$(du -sb web/dist | cut -f 1)"
} > "$OUTPUT/storage.txt"

mkdir -p "$FINAL_OUTPUT"
cp -R "$OUTPUT/." "$FINAL_OUTPUT/"

printf '%s\n' \
  "Acceptance evidence copied to $FINAL_OUTPUT." \
  'Native and WASM exactness completed. Physical ChromeOS browser checks remain:' \
  'WebGPU, forced WebGL2, 2-D fallback, offline/update, worker failure, suspension.' \
  'Record them in phase12/DUET_ACCEPTANCE.md before claiming device acceptance.'
