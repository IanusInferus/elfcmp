#!/usr/bin/env bash
set -euo pipefail
TEST_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$TEST_DIR/../common.sh"

build_elfcmp
mkdir -p "$TEST_TMP/build"

LIBRARY_NAME=liba.b.c.so.1.2.3
ubuntu_cc -O2 -Wall -Wextra -fPIC -shared "$TEST_DIR/library.c" \
    -Wl,-soname,"$LIBRARY_NAME" -o "$TEST_TMP/build/$LIBRARY_NAME"

"$ELFCMP" copy "$TEST_TMP/build/$LIBRARY_NAME" "$TEST_TMP/bundle" \
    --sysroot "$UBUNTU_SYSROOT" \
    --system-lib-search-paths "$UBUNTU_SYSTEM_LIB_SEARCH_PATHS"

test -f "$TEST_TMP/bundle/$LIBRARY_NAME"
if [[ -d $TEST_TMP/bundle/lib ]]; then
    echo "shared-library bundle unexpectedly contains a lib/ directory" >&2
    exit 1
fi
test -f "$TEST_TMP/bundle/elfcmp-reference.yaml"

"$ELFCMP" patch "$TEST_TMP/bundle" --patchelf "$PATCHELF"
[[ $($PATCHELF --print-rpath "$TEST_TMP/bundle/$LIBRARY_NAME") == '$ORIGIN' ]]

echo "PASS: shared-library bundle keeps its files at the bundle root"

