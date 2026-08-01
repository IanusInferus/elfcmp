#!/usr/bin/env bash
set -euo pipefail

INTEGRATION_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_DIR=$(cd -- "$INTEGRATION_DIR/.." && pwd)

UBUNTU_SYSROOT=${UBUNTU_SYSROOT:-"$HOME/ubuntu-24.04-x86_64_basic"}
CENTOS_SYSROOT=${CENTOS_SYSROOT:-"$HOME/centos-7-x86_64_basic"}
UBUNTU_SYSTEM_LIB_SEARCH_PATHS=${UBUNTU_SYSTEM_LIB_SEARCH_PATHS:-"/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu"}
CENTOS_SYSTEM_LIB_SEARCH_PATHS=${CENTOS_SYSTEM_LIB_SEARCH_PATHS:-"/lib64:/usr/lib64"}
PATCHELF=${PATCHELF:-"$INTEGRATION_DIR/patchelf"}
CC=${CC:-gcc}
CARGO=${CARGO:-cargo}
READELF=${READELF:-readelf}

require_file() {
    if [[ ! -e $1 ]]; then
        echo "required file is missing: $1" >&2
        exit 2
    fi
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "required command is missing: $1" >&2
        exit 2
    fi
}

require_command "$CC"
require_command "$CARGO"
require_command "$READELF"
if [[ ! -d $UBUNTU_SYSROOT ]]; then
    echo "Ubuntu sysroot directory is missing: $UBUNTU_SYSROOT" >&2
    echo "Set UBUNTU_SYSROOT or place it at ~/ubuntu-24.04-x86_64_basic." >&2
    exit 2
fi
require_file "$UBUNTU_SYSROOT/usr/include/stdio.h"
require_file "$UBUNTU_SYSROOT/usr/lib/x86_64-linux-gnu/libc.so.6"
if [[ ! -d $CENTOS_SYSROOT ]]; then
    echo "CentOS sysroot directory is missing: $CENTOS_SYSROOT" >&2
    echo "Set CENTOS_SYSROOT or place it at ~/centos-7-x86_64_basic." >&2
    exit 2
fi
if [[ ! -f $PATCHELF ]]; then
    echo "patchelf is missing: $PATCHELF" >&2
    echo "Place the Linux patchelf binary at $INTEGRATION_DIR/patchelf." >&2
    exit 2
fi
if [[ ! -x $PATCHELF ]]; then
    echo "patchelf is not executable: $PATCHELF" >&2
    echo "Run: chmod +x '$PATCHELF'" >&2
    exit 2
fi

ELFCMP="$REPO_DIR/target/release/elfcmp"
TEST_TMP=$(mktemp -d "$INTEGRATION_DIR/.elfcmp-test.XXXXXX")
echo "Test artifacts: $TEST_TMP"

build_elfcmp() {
    "$CARGO" build --release --manifest-path "$REPO_DIR/Cargo.toml"
}

ubuntu_cc() {
    "$CC" --sysroot="$UBUNTU_SYSROOT" "$@"
}

assert_contains() {
    local file=$1
    local text=$2
    if ! grep -F -- "$text" "$file" >/dev/null; then
        echo "expected '$text' in $file" >&2
        exit 1
    fi
}
