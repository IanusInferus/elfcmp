# How to Get Sysroot for clang

To build a program or library with minimal dependency for Linux, we usually need a sysroot for a distribution with an old glibc.

We can always copy files from an installed system. But it is more clean to just build it from the distribution image.

CentOS has the lowest glibc version in famous distributions. So we present the process to generate a sysroot for CentOS 7 x64.

## Generate CentOS Sysroot

Network repos for CentOS 6.x are fading out and it's easy to encounter repos with only partial packages, so it's best to grab an image for CentOS 7 x64. We need `CentOS-7-x86_64-DVD-2009.iso`.

Mount the ISO and we can found rpm packages under `Packages`.

Copy all packages needed for build. For a basic setup, you need

    gcc-4.8.5-44.el7.x86_64.rpm
    glibc-2.17-317.el7.x86_64.rpm
    glibc-devel-2.17-317.el7.x86_64.rpm
    glibc-headers-2.17-317.el7.x86_64.rpm
    kernel-headers-3.10.0-1160.el7.x86_64.rpm
    libselinux-2.5-15.el7.x86_64.rpm
    libselinux-devel-2.5-15.el7.x86_64.rpm

On an ext4 partition, extract these rpm packages

    # CentOS 7
    sudo yum install rpm2cpio cpio

    # Debian/Ubuntu
    sudo apt install rpm2cpio cpio

    mkdir centos-7-x86_64_basic
    cd centos-7-x86_64_basic
    find /path/to/packages -name "*.rpm" -exec sh -c "rpm2cpio {} | cpio -idmv" \;

A sysroot is generated successfully. There may be warnings saying "newer or same age version exists", but they can be ignored.

To add new packages, copy it to `/path/to/packages` and repeat the process.

## Download Musl Sysroot

Download toolchain from [musl libc toolchain](https://musl.cc/). Choose the ones ends with `-native.tgz`.

For cross-compilation, additional flags are needed both in compiler flags (CommonFlags) and linker flags (LinkerFlags).

For example, if we place the sysroots in `/root`, we need to add the following flags for x64, arm64, riscv64, x86, armv7a and mipsel respectively.

    -target x86_64-linux-musl --prefix=/root/x86_64-linux-musl-native/usr/lib/gcc/x86_64-linux-musl/11.2.1 -L/root/x86_64-linux-musl-native/usr/lib/gcc/x86_64-linux-musl/11.2.1
    -target aarch64-linux-musl --prefix=/root/aarch64-linux-musl-native/usr/lib/gcc/aarch64-linux-musl/11.2.1 -L/root/aarch64-linux-musl-native/usr/lib/gcc/aarch64-linux-musl/11.2.1
    -target riscv64-linux-musl --prefix=/root/riscv64-linux-musl-native/usr/lib/gcc/riscv64-linux-musl/11.2.1 -L/root/riscv64-linux-musl-native/usr/lib/gcc/riscv64-linux-musl/11.2.1

    -target i686-linux-musl --prefix=/root/i686-linux-musl-native/usr/lib/gcc/i686-linux-musl/11.2.1 -L/root/i686-linux-musl-native/usr/lib/gcc/i686-linux-musl/11.2.1
    -target arm-linux-musleabihf --prefix=/root/arm-linux-musleabihf-native/usr/lib/gcc/arm-linux-musleabihf/11.2.1 -L/root/arm-linux-musleabihf-native/usr/lib/gcc/arm-linux-musleabihf/11.2.1
    -target mipsel-linux-musl --prefix=/root/mipsel-linux-musl-native/usr/lib/gcc/mipsel-linux-musl/11.2.1 -L/root/mipsel-linux-musl-native/usr/lib/gcc/mipsel-linux-musl/11.2.1

For riscv64, `-mno-relax` is needed in compiler flags (CommonFlags).

For mipsel, `-latomic` is needed in linker flags (LinkerFlags).

## Debug

To debug for library search path, call

    clang -v
    clang --sysroot=/path/to/sysroot --gcc-toolchain=/path/to/sysroot/usr -v
    clang -target <target-triplet> --sysroot=/path/to/sysroot --gcc-toolchain=/path/to/sysroot/usr -v

and check `Selected GCC installation`. If there is no `Selected GCC installation` shown, add `-target <target-triplet> --prefix=/path/to/sysroot/usr/lib/gcc/<target-triplet>/<gcc-version> -L/path/to/sysroot/usr/lib/gcc/<target-triplet>/<gcc-version>` to compiler and linker flags (CommonFlags LinkerFlags).

To check glibc dependency of a built executable or dynamic library, call

    ldd -v /path/to/executable
    nm -D --with-symbol-versions /path/to/executable
    readelf -d /path/to/executable

and check symbols with GLIBC_2.* versions.
