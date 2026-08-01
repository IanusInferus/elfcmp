# Integration test guide

These tests compile all ELF fixtures against an Ubuntu 24.04 sysroot, compare
them with a CentOS 7 sysroot, patch temporary bundles, inspect the resulting ELF
metadata, and execute the patched programs.

## 1. Install host tools

The tests require a Linux environment with Bash, GCC, binutils, Cargo, and a
Linux `patchelf` binary. On Ubuntu:

```bash
sudo apt update
sudo apt install build-essential binutils curl rpm2cpio cpio
```

If Rust is not installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init
sh /tmp/rustup-init -y --profile minimal
source "$HOME/.cargo/env"
```

Confirm the tools are visible:

```bash
gcc --version
readelf --version
cargo --version
```

## 2. Set up the sysroots

The default locations are:

```text
~/ubuntu-24.04-x86_64_basic
~/centos-7-x86_64_basic
```

The Ubuntu sysroot is used by every GCC invocation and must include development
headers, startup objects, the dynamic loader, and libraries. At minimum, these
paths should exist:

```bash
test -f "$HOME/ubuntu-24.04-x86_64_basic/usr/include/stdio.h"
test -f "$HOME/ubuntu-24.04-x86_64_basic/usr/lib/x86_64-linux-gnu/crt1.o"
test -f "$HOME/ubuntu-24.04-x86_64_basic/usr/lib/x86_64-linux-gnu/libc.so.6"
```

The CentOS sysroot must contain `libdl.so.2`. It may use a conventional
`lib64/` layout or place the library directly at its root:

```bash
find "$HOME/centos-7-x86_64_basic" -name libdl.so.2 -print
```

If this prints nothing, extract at least the CentOS glibc runtime package into
the sysroot. The tests search `/lib64:/usr/lib64`, interpreted relative to the
CentOS sysroot.

For example, given a CentOS 7 glibc RPM:

```bash
mkdir -p "$HOME/centos-7-x86_64_basic"
cd "$HOME/centos-7-x86_64_basic"
rpm2cpio /path/to/glibc-2.17-317.el7.x86_64.rpm | cpio -idm
find . -name libdl.so.2 -print
```

Extraction normally creates `lib64/libdl.so.2` along with `libc.so.6` and the
dynamic loader. Do not copy only the symlink: its `libdl-2.17.so` target must be
present as well.

## 3. Install patchelf for the tests

Place the Linux `patchelf` executable directly under `integration/`:

```bash
cd /path/to/elfcmp
cp /path/to/patchelf integration/patchelf
chmod +x integration/patchelf
test -x integration/patchelf
```

The binary is intentionally ignored by Git. The tests stop during preflight
with placement and permission instructions if it is absent or not executable.

## 4. Set sysroot and tool paths

The default sysroot locations can be used without exporting anything. For
different locations, set:

```bash
export UBUNTU_SYSROOT=/path/to/ubuntu-24.04-x86_64_basic
export CENTOS_SYSROOT=/path/to/centos-7-x86_64_basic
export UBUNTU_SYSTEM_LIB_SEARCH_PATHS=/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu
export CENTOS_SYSTEM_LIB_SEARCH_PATHS=/lib64:/usr/lib64
export CC=gcc
export CARGO="$HOME/.cargo/bin/cargo"
export READELF=readelf
```

Only the sysroot paths normally need changing. The library-search defaults suit
the documented x86-64 sysroots and are interpreted inside their respective
sysroot. `patchelf` is found at `integration/patchelf` unless `PATCHELF` is
explicitly set.

## 5. Run the tests

Run both tests:

```bash
bash integration/run-tests.sh
```

Or run them separately:

```bash
bash integration/compat/test.sh
bash integration/dlopen/test.sh
bash integration/library/test.sh
```

The first test maps `explicit_bzero@GLIBC_2.25` to an unversioned compatibility
symbol. The second maps `dlopen@GLIBC_2.34` to
`libdl.so.2/dlopen@GLIBC_2.2.5`, reusing a `GLIBC_2.2.5` version index that was
recorded for another library. The third verifies that a
versioned shared-library input is copied into a flat bundle without `lib/`.

Each test creates a unique `integration/.elfcmp-test.*` directory and prints its
path before building. The directory is deliberately retained after success,
failure, or interruption so its binaries, mappings, and patched bundles can be
inspected. Source fixtures are never modified.

`integration/run-tests.sh` removes all previously retained
`integration/.elfcmp-test.*` directories before starting the combined run. It
does not remove the new artifact directories created by that run. Running an
individual `compat/test.sh` or `dlopen/test.sh` never removes prior artifacts.

To clean retained test artifacts manually, first review the matching paths and
then remove them:

```bash
find integration -maxdepth 1 -type d -name '.elfcmp-test.*' -print
find integration -maxdepth 1 -type d -name '.elfcmp-test.*' -exec rm -rf -- {} +
```
