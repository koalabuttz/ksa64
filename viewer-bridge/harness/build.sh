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
cargo build -p ksa64-viewer-bridge --profile viewer --locked $FEATURE_ARGS

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
cargo run --locked -p ksa64-viewer-bridge --bin bridge-manifest --quiet -- \
    "$COMMIT" viewer "$(basename "$STAGED")" "$TARGET_TRIPLE" "$OS" "$ARCH" \
    "$SHA256" "$CATALOG_IDENTITY" > "$STAGED.json"

CC=${CC:-cc}
CXX=${CXX:-c++}
mkdir -p "$SCRIPT_DIR/bin"
"$CC" -std=c11 -Wall -Wextra -Werror -pedantic \
    "$SCRIPT_DIR/header_smoke.c" -o "$SCRIPT_DIR/bin/ksa64_viewer_header_smoke"
"$CC" -std=c11 -Wall -Wextra -Werror -pedantic \
    "$REPO/presentation/c/kps1_vectors.c" -o "$SCRIPT_DIR/bin/ksa64_kps1_c_vectors"
"$SCRIPT_DIR/bin/ksa64_kps1_c_vectors"
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
