#!/usr/bin/env bash
set -euo pipefail
TEST_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$TEST_DIR/../common.sh"

build_elfcmp
mkdir -p "$TEST_TMP/build" "$TEST_TMP/host-lib" "$TEST_TMP/target/lib64"
CENTOS_LIBC=$(find "$CENTOS_SYSROOT" \( -type f -o -type l \) -name libc.so.6 \
    -print -quit)
if [[ -z $CENTOS_LIBC ]]; then
    echo "CentOS sysroot contains no libc.so.6: $CENTOS_SYSROOT" >&2
    exit 2
fi
cp -L "$CENTOS_LIBC" "$TEST_TMP/target/lib64/libc.so.6"

ubuntu_cc -O2 -U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -Wall -Wextra \
    "$TEST_DIR/hello.c" -o "$TEST_TMP/build/hello"
ubuntu_cc -O2 -Wall -Wextra -fPIC -shared "$TEST_DIR/compat.c" \
    -Wl,-soname,libcompat.so.1 -Wl,--version-script="$TEST_DIR/compat.map" \
    -o "$TEST_TMP/host-lib/libcompat.so.1"

"$READELF" --dyn-syms --wide "$TEST_TMP/host-lib/libcompat.so.1" \
    >"$TEST_TMP/compat-symbols.txt"
assert_contains "$TEST_TMP/compat-symbols.txt" "elfcmp_explicit_bzero@@COMPAT_1.0"

"$READELF" --dyn-syms --wide "$TEST_TMP/build/hello" >"$TEST_TMP/symbols_before.txt"
assert_contains "$TEST_TMP/symbols_before.txt" "explicit_bzero@GLIBC_2.25"

"$ELFCMP" copy "$TEST_TMP/build/hello" "$TEST_TMP/bundle" \
    --sysroot "$UBUNTU_SYSROOT" \
    --system-lib-search-paths "$UBUNTU_SYSTEM_LIB_SEARCH_PATHS"
"$ELFCMP" map "$TEST_TMP/bundle/elfcmp-reference.yaml" "$CENTOS_SYSROOT" \
    "$TEST_TMP/mapping-template.yaml" \
    --system-lib-search-paths "$CENTOS_SYSTEM_LIB_SEARCH_PATHS"
assert_contains "$TEST_TMP/mapping-template.yaml" "symbol: explicit_bzero"
assert_contains "$TEST_TMP/mapping-template.yaml" "version: GLIBC_2.25"

cp "$TEST_TMP/host-lib/libcompat.so.1" "$TEST_TMP/bundle/lib/"
"$ELFCMP" check "$TEST_TMP/bundle/elfcmp-reference.yaml" "$TEST_TMP/target" \
    "$TEST_DIR/mapping.yaml" --system-lib-search-paths /lib64 \
    --lib-search-paths "$TEST_TMP/host-lib"
"$ELFCMP" patch "$TEST_TMP/bundle" --mapping "$TEST_DIR/mapping.yaml" \
    --patchelf "$PATCHELF" --target-sysroot "$TEST_TMP/target" \
    --system-lib-search-paths /lib64 \
    --lib-search-paths "$TEST_TMP/host-lib"

"$READELF" --dyn-syms --wide "$TEST_TMP/bundle/hello" >"$TEST_TMP/symbols_after.txt"
assert_contains "$TEST_TMP/symbols_after.txt" "UND elfcmp_explicit_bzero"
if grep -F "explicit_bzero@GLIBC_2.25" "$TEST_TMP/symbols_after.txt" >/dev/null; then
    echo "old explicit_bzero version remains after patch" >&2
    exit 1
fi
[[ $($PATCHELF --print-rpath "$TEST_TMP/bundle/hello") == '$ORIGIN/lib' ]]
[[ $($PATCHELF --print-rpath "$TEST_TMP/bundle/lib/libcompat.so.1") == '$ORIGIN' ]]
"$TEST_TMP/bundle/hello" | grep -Fx "hello from elfcmp"

echo "PASS: versioned explicit_bzero mapped through an unversioned requirement to a versioned default export"
