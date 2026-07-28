#!/usr/bin/env sh
set -eu

OUTPUT_DIRECTORY=dist/phase12b5
SKIP_BUILD=0
ALLOW_DIRTY=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) OUTPUT_DIRECTORY=$2; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --allow-dirty) ALLOW_DIRTY=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO"

QUALIFICATION=qualified
if [ "$ALLOW_DIRTY" -eq 1 ]; then
  QUALIFICATION=unqualified-local
elif [ -n "$(git status --porcelain=v1 --untracked-files=normal)" ]; then
  echo "qualified packaging requires a clean source tree; commit changes or use --allow-dirty for an explicitly unqualified local archive" >&2
  exit 2
fi

TARGET_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
case "$TARGET_TRIPLE" in
  x86_64-unknown-linux-gnu)
    OS=linux; ARCH=x86_64; EXECUTABLE=ksa64; LIBRARY=libksa64_viewer_bridge.so ;;
  aarch64-unknown-linux-gnu)
    OS=linux; ARCH=aarch64; EXECUTABLE=ksa64; LIBRARY=libksa64_viewer_bridge.so ;;
  aarch64-apple-darwin)
    OS=macos; ARCH=aarch64; EXECUTABLE=ksa64; LIBRARY=libksa64_viewer_bridge.dylib ;;
  *) echo "unsupported Phase 12B.5 engineering target: $TARGET_TRIPLE" >&2; exit 2 ;;
esac

if [ "$SKIP_BUILD" -eq 0 ]; then
  cargo build -p ksa64-host --bin ksa64 --release --locked
  cargo build -p ksa64-viewer-bridge --profile viewer --locked
fi

COMMIT=$(git rev-parse --short=12 HEAD)
QUALIFIED_NAME="ksa64-phase12b5-$OS-$ARCH-$COMMIT"
if [ "$QUALIFICATION" != qualified ]; then
  QUALIFIED_NAME="$QUALIFIED_NAME-$QUALIFICATION"
fi
case "$OUTPUT_DIRECTORY" in
  /*) OUTPUT_ROOT=$OUTPUT_DIRECTORY ;;
  *) OUTPUT_ROOT=$REPO/$OUTPUT_DIRECTORY ;;
esac
mkdir -p "$OUTPUT_ROOT"
OUTPUT_ROOT=$(CDPATH= cd -- "$OUTPUT_ROOT" && pwd)
STAGE="$OUTPUT_ROOT/$QUALIFIED_NAME"
case "$STAGE" in
  "$OUTPUT_ROOT"/*) ;;
  *) echo "refusing staging path outside output directory" >&2; exit 2 ;;
esac
if [ -e "$STAGE" ]; then
  rm -rf -- "$STAGE"
fi
mkdir -p "$STAGE"

EXECUTABLE_SOURCE="$REPO/target/release/$EXECUTABLE"
LIBRARY_SOURCE="$REPO/target/viewer/$LIBRARY"
for REQUIRED in "$EXECUTABLE_SOURCE" "$LIBRARY_SOURCE"; do
  if [ ! -f "$REQUIRED" ]; then
    echo "required build output missing: $REQUIRED" >&2
    exit 2
  fi
done
cp "$EXECUTABLE_SOURCE" "$STAGE/$EXECUTABLE"
cp "$LIBRARY_SOURCE" "$STAGE/$LIBRARY"
cp "$REPO/viewer-bridge/ksa64_viewer_bridge.h" "$STAGE/ksa64_viewer_bridge.h"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
LIBRARY_SHA=$(sha256_file "$STAGE/$LIBRARY")
CATALOG_IDENTITY=b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13
cargo run --locked -p ksa64-viewer-bridge --bin bridge-manifest --quiet -- \
  "$COMMIT" viewer "$LIBRARY" "$TARGET_TRIPLE" "$OS" "$ARCH" \
  "$LIBRARY_SHA" "$CATALOG_IDENTITY" > "$STAGE/$LIBRARY.json"
cat > "$STAGE/README.txt" <<EOF
KSA64 Phase 12B.5 engineering archive
Target: $TARGET_TRIPLE
Source: $COMMIT
Qualification: $QUALIFICATION

This is an unsigned engineering build. Run 'ksa64' for product discovery.
The bridge ABI is described by ksa64_viewer_bridge.h and the adjacent manifest.
No installer, code-signing, notarization, or app-store claim is implied.
EOF
(
  cd "$STAGE"
  for FILE in $(find . -maxdepth 1 -type f -print | sed 's#^./##' | sort); do
    printf '%s  %s
' "$(sha256_file "$FILE")" "$FILE"
  done > SHA256SUMS
)
ARCHIVE="$OUTPUT_ROOT/$QUALIFIED_NAME.tar.gz"
if [ -e "$ARCHIVE" ]; then
  rm -f -- "$ARCHIVE"
fi
tar -C "$OUTPUT_ROOT" -czf "$ARCHIVE" "$QUALIFIED_NAME"
printf '%s
' "$ARCHIVE"
