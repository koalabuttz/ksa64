#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
cd "$REPO"

FEATURE_ARGS=
RUN_FULL=0
for argument in "$@"; do
    case "$argument" in
        --panic-probe) FEATURE_ARGS="--features panic-probe" ;;
        --full) RUN_FULL=1 ;;
        *) echo "unknown argument: $argument" >&2; exit 2 ;;
    esac
done

# shellcheck disable=SC2086
cargo build -p ksa64-viewer-bridge --profile viewer $FEATURE_ARGS

case "$(uname -s)" in
    Darwin) OS=macos; EXT=dylib; LIBRARY="libksa64_viewer_bridge.dylib"; DL_LIBS= ;;
    Linux) OS=linux; EXT=so; LIBRARY="libksa64_viewer_bridge.so"; DL_LIBS=-ldl ;;
    *) echo "unsupported native harness host: $(uname -s)" >&2; exit 2 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) ARCH=x86_64 ;;
    arm64|aarch64) ARCH=aarch64 ;;
    *) echo "unsupported native harness architecture: $(uname -m)" >&2; exit 2 ;;
esac

COMMIT=$(git rev-parse --short=12 HEAD)
SOURCE="$REPO/target/viewer/$LIBRARY"
STAGED="$REPO/target/viewer/libksa64_viewer_bridge-$COMMIT-120b0001.$EXT"
cp "$SOURCE" "$STAGED"
if command -v sha256sum >/dev/null 2>&1; then
    SHA256=$(sha256sum "$STAGED" | awk '{print $1}')
else
    SHA256=$(shasum -a 256 "$STAGED" | awk '{print $1}')
fi
TARGET_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
CATALOG_IDENTITY=b7456cfdb250c4ee3434a244b75dd5ceb88fc4d8e3fb50058ea17b932df67d13
cat > "$STAGED.json" <<EOF
{
  "schema": "ksa64.viewer-bridge-artifact.v2",
  "abi_version": 1,
  "build_identity": 302710785,
  "source_commit": "$COMMIT",
  "profile": "viewer",
  "library_file": "$(basename "$STAGED")",
  "target_triple": "$TARGET_TRIPLE",
  "operating_system": "$OS",
  "architecture": "$ARCH",
  "sha256": "$SHA256",
  "catalog_identity": "$CATALOG_IDENTITY",
  "structure_sizes": {
    "abi_info": 132,
    "event": 24,
    "operational_view_v1": 208,
    "procedure_view_v1": 376,
    "disposition_v1": 72,
    "action_proposal_v1": 144,
    "action_receipt_v1": 80,
    "timeline_event_v1": 136,
    "release_sample_v1": 112,
    "prediction_path_header_v1": 88,
    "prediction_path_point_v1": 56,
    "transport_status_v1": 96,
    "finish_status_v1": 64,
    "owned_buffer": 32,
    "snapshot": 184,
    "span": 24,
    "start_request_v1": 48
  }
}
EOF

CC=${CC:-cc}
CXX=${CXX:-c++}
mkdir -p "$SCRIPT_DIR/bin"
"$CC" -std=c11 -Wall -Wextra -Werror -pedantic \
    "$SCRIPT_DIR/header_smoke.c" -o "$SCRIPT_DIR/bin/ksa64_viewer_header_smoke"
"$SCRIPT_DIR/bin/ksa64_viewer_header_smoke"
# shellcheck disable=SC2086
"$CXX" -std=c++20 -Wall -Wextra -Werror -pedantic \
    "$SCRIPT_DIR/main.cpp" -o "$SCRIPT_DIR/bin/ksa64_viewer_harness" $DL_LIBS
"$SCRIPT_DIR/bin/ksa64_viewer_harness" "$STAGED"

if [ "$RUN_FULL" -eq 1 ]; then
    # shellcheck disable=SC2086
    "$CXX" -std=c++20 -Wall -Wextra -Werror -pedantic \
        "$SCRIPT_DIR/full_mission.cpp" \
        -o "$SCRIPT_DIR/bin/ksa64_viewer_full_mission_harness" $DL_LIBS
    "$SCRIPT_DIR/bin/ksa64_viewer_full_mission_harness" "$STAGED"
fi
