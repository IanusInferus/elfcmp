#!/usr/bin/env bash
set -euo pipefail
TEST_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$TEST_DIR/../common.sh"

build_elfcmp
require_command objcopy
mkdir -p "$TEST_TMP/build" "$TEST_TMP/source-root/wrong" \
    "$TEST_TMP/source-root/right" "$TEST_TMP/source-root/fallback"
ln -s "$UBUNTU_SYSROOT/lib" "$TEST_TMP/source-root/lib"
ln -s "$UBUNTU_SYSROOT/usr" "$TEST_TMP/source-root/usr"

LIBRARY_NAME=liba.b.c.so.1.2.3
DEPENDENCY_NAME=libdependency.so.1
ubuntu_cc -O2 -Wall -Wextra -fPIC -shared "$TEST_DIR/dependency.c" \
    -Wl,-soname,"$DEPENDENCY_NAME" -o "$TEST_TMP/source-root/right/$DEPENDENCY_NAME"
cp "$TEST_TMP/source-root/right/$DEPENDENCY_NAME" \
    "$TEST_TMP/source-root/wrong/$DEPENDENCY_NAME"
objcopy --alt-machine-code=3 "$TEST_TMP/source-root/wrong/$DEPENDENCY_NAME"
ubuntu_cc -O2 -Wall -Wextra -fPIC -shared "$TEST_DIR/dependency.c" \
    -DELFCMP_DEPENDENCY_VALUE=7 -Wl,-soname,"$DEPENDENCY_NAME" \
    -o "$TEST_TMP/source-root/fallback/$DEPENDENCY_NAME"
ubuntu_cc -O2 -Wall -Wextra -fPIC -shared "$TEST_DIR/library.c" \
    -L"$TEST_TMP/source-root/right" -Wl,-l:libdependency.so.1 \
    -Wl,-soname,"$LIBRARY_NAME" -Wl,-rpath,/right \
    -o "$TEST_TMP/build/$LIBRARY_NAME"

"$ELFCMP" copy "$TEST_TMP/build/$LIBRARY_NAME" "$TEST_TMP/bundle" \
    --sysroot "$TEST_TMP/source-root" \
    --system-lib-search-paths "/fallback:/wrong:$UBUNTU_SYSTEM_LIB_SEARCH_PATHS"

test -f "$TEST_TMP/bundle/$LIBRARY_NAME"
test -f "$TEST_TMP/bundle/$DEPENDENCY_NAME"
cmp "$TEST_TMP/bundle/$DEPENDENCY_NAME" "$TEST_TMP/source-root/right/$DEPENDENCY_NAME"
if [[ -d $TEST_TMP/bundle/lib ]]; then
    echo "shared-library bundle unexpectedly contains a lib/ directory" >&2
    exit 1
fi
test -f "$TEST_TMP/bundle/elfcmp-reference.yaml"
"$READELF" --file-header "$TEST_TMP/bundle/$DEPENDENCY_NAME" \
    | grep -F "Machine:" | grep -F "Advanced Micro Devices X86-64"

cp "$TEST_TMP/bundle/$DEPENDENCY_NAME" \
    "$TEST_TMP/bundle/ld-linux-x86-64.so.2"
"$ELFCMP" patch "$TEST_TMP/bundle" --patchelf "$PATCHELF"
[[ $($PATCHELF --print-rpath "$TEST_TMP/bundle/$LIBRARY_NAME") == '$ORIGIN' ]]
[[ $($PATCHELF --print-rpath "$TEST_TMP/bundle/ld-linux-x86-64.so.2") == '$ORIGIN' ]]

echo "PASS: shared-library bundle keeps its files at the bundle root"
