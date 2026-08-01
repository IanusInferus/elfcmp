#!/usr/bin/env bash
set -euo pipefail
TEST_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$TEST_DIR/../common.sh"

build_elfcmp
mkdir -p "$TEST_TMP/build" "$TEST_TMP/runtime/lib64" \
    "$TEST_TMP/source-sysroot/custom"

CENTOS_LIBDL=$(find "$CENTOS_SYSROOT" \( -type f -o -type l \) -name libdl.so.2 \
    -print -quit)
if [[ -z $CENTOS_LIBDL ]]; then
    echo "CentOS sysroot contains no libdl.so.2: $CENTOS_SYSROOT" >&2
    echo "Extract the CentOS glibc runtime package into that sysroot and retry." >&2
    exit 2
fi

ubuntu_cc -O2 -fPIC -shared "$TEST_DIR/marker.c" \
    -Wl,--version-script="$TEST_DIR/libdl.map" \
    -Wl,-soname,libversioncarrier.so.1 \
    -o "$TEST_TMP/build/libversioncarrier.so.1"
cp "$TEST_TMP/build/libversioncarrier.so.1" "$TEST_TMP/source-sysroot/custom/"
ln -s "$UBUNTU_SYSROOT/lib" "$TEST_TMP/source-sysroot/lib"
ln -s "$UBUNTU_SYSROOT/usr" "$TEST_TMP/source-sysroot/usr"
ubuntu_cc -O2 -fPIC -shared "$TEST_DIR/target_libdl.c" \
    -Wl,--version-script="$TEST_DIR/libdl.map" \
    -Wl,-soname,libdl.so.2 -o "$TEST_TMP/runtime/lib64/libdl.so.2"
ubuntu_cc -O2 -Wall -Wextra "$TEST_DIR/hello.c" \
    -L"$TEST_TMP/build" -Wl,--no-as-needed -Wl,-l:libversioncarrier.so.1 \
    -o "$TEST_TMP/build/hello"

"$READELF" --dyn-syms --wide "$TEST_TMP/build/hello" >"$TEST_TMP/symbols_before.txt"
assert_contains "$TEST_TMP/symbols_before.txt" "dlopen@GLIBC_2.34"
assert_contains "$TEST_TMP/symbols_before.txt" "elfcmp_libdl_marker@GLIBC_2.2.5"

"$ELFCMP" copy "$TEST_TMP/build/hello" "$TEST_TMP/bundle" \
    --sysroot "$TEST_TMP/source-sysroot" \
    --system-lib-search-paths "/custom:$UBUNTU_SYSTEM_LIB_SEARCH_PATHS"
"$ELFCMP" map "$TEST_TMP/bundle/elfcmp-reference.yaml" "$CENTOS_SYSROOT" \
    "$TEST_TMP/mapping-template.yaml" \
    --system-lib-search-paths "$CENTOS_SYSTEM_LIB_SEARCH_PATHS"
assert_contains "$TEST_TMP/mapping-template.yaml" "symbol: dlopen"
assert_contains "$TEST_TMP/mapping-template.yaml" "version: GLIBC_2.34"
"$ELFCMP" check "$TEST_TMP/bundle/elfcmp-reference.yaml" "$CENTOS_SYSROOT" \
    "$TEST_DIR/mapping.yaml" \
    --system-lib-search-paths "$CENTOS_SYSTEM_LIB_SEARCH_PATHS"

cp "$TEST_TMP/runtime/lib64/libdl.so.2" "$TEST_TMP/bundle/lib/"
"$ELFCMP" patch "$TEST_TMP/bundle" \
    --mapping "$TEST_DIR/mapping.yaml" --patchelf "$PATCHELF"

"$READELF" --dyn-syms --wide "$TEST_TMP/bundle/hello" >"$TEST_TMP/symbols_after.txt"
assert_contains "$TEST_TMP/symbols_after.txt" "dlopen@GLIBC_2.2.5"
if grep -F "dlopen@GLIBC_2.34" "$TEST_TMP/symbols_after.txt" >/dev/null; then
    echo "old dlopen version remains after patch" >&2
    exit 1
fi
"$TEST_TMP/bundle/hello" | grep -Fx "dlopen version mapping passed"

echo "PASS: dlopen reused a GLIBC_2.2.5 version index recorded for another library"
