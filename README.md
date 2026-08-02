# elfcmp

`elfcmp` (ELF copy-map-patch) builds relocatable Linux application bundles while
making ABI differences between source and target sysroots explicit.

## Build

```sh
cargo build --release
```

Or a musl-static build:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

ELF inspection is implemented in Rust. The `patch` command invokes
[`patchelf`](https://github.com/NixOS/patchelf), which must be installed on the
machine where patching is performed.

## Workflow

### 1. Copy

Copy syntax:

```text
elfcmp copy INPUT OUTPUT \
  [--sysroot SYSROOT] \
  [--system-lib-search-paths PATHS] \
  [--system-lib-basenames BASENAMES] \
  [--reference REFERENCE]
```

Executable example:

```sh
elfcmp copy ./my-program ./executable-bundle \
  --sysroot /source/sysroot \
  --system-lib-search-paths /lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu \
  --reference ./executable-bundle/elfcmp-reference.yaml
```

This produces:

```text
executable-bundle/
├── my-program
├── elfcmp-reference.yaml
└── lib/
    ├── libdependency-a.so.1
    └── libdependency-b.so.2
```

Dynamic-library example:

```sh
elfcmp copy ./liba.b.c.so.1.2.3 ./library-bundle \
  --sysroot /source/sysroot \
  --system-lib-search-paths /lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu \
  --reference ./library-bundle/elfcmp-reference.yaml
```

This produces a flat bundle without an additional `lib/` directory:

```text
library-bundle/
├── liba.b.c.so.1.2.3
├── elfcmp-reference.yaml
├── libdependency-a.so.1
└── libdependency-b.so.2
```

In both layouts, dependencies are resolved recursively and the reference table
records imported symbols supplied by skipped system libraries.

- `INPUT` is the ELF executable or shared library to scan and copy. It has no
  default. A filename matching `lib<name>.so` followed by optional numeric
  version components, such as `liba.b.c.so.1.2.3`, is treated as a shared
  library.

- `OUTPUT` is the bundle destination. It has no default. The executable is
  copied to its root and dependencies to `OUTPUT/lib/`. For a shared-library
  input, the input and its dependencies are all copied directly to the `OUTPUT`
  root; no additional `lib/` directory is created.

- `--sysroot` selects the root filesystem used to resolve the input's
  dependencies. Its default is `/`, which uses the current system root.

- `--system-lib-search-paths` supplies additional colon-separated library
  directories. Absolute entries are interpreted inside `--sysroot`, so
  `/lib/x86_64-linux-gnu` in the example means
  `/source/sysroot/lib/x86_64-linux-gnu`. Its default is empty. The built-in
  search directories are `lib`, `lib64`, `usr/lib`, `usr/lib64`, and
  `usr/local/lib`, plus one-level multiarch directories beneath `lib` and
  `usr/lib`.

  When an input or recursively scanned library contains `DT_RUNPATH`, its
  colon-separated directories are searched first for that library's direct
  dependencies. Literal absolute entries are interpreted beneath `--sysroot`.
  `$ORIGIN` and `${ORIGIN}` are expanded to the directory containing the ELF,
  including when the input itself is outside the sysroot.

  Candidate libraries must match the input ELF's machine architecture, bitness,
  and endianness. A same-named library for another architecture is skipped; if
  no compatible candidate exists, `elfcmp` reports the rejected paths.

- `--reference` selects the reference-table output path. Its default is
  `OUTPUT/elfcmp-reference.yaml`, where `OUTPUT` is the `copy` destination. For
  an example destination `./executable-bundle`, the default is therefore
  `./executable-bundle/elfcmp-reference.yaml`.

- `--system-lib-basenames` identifies system libraries that are referenced but
  not copied. Its default is
  `libc,libdl,libm,libpthread,librt,libselinux,ld-linux-x86-64`. Supplying the
  option replaces that complete list; it does not add to the defaults. Matching removes only
  the final `.so` and its numeric version suffix: `liba.b.c.so.1.2.3` has
  basename `liba.b.c`. Dots within the library name remain significant. Thus
  `libc` matches `libc.so.6`, but not `libc++.so.1` or `libcrypt.so.1`.

### 2. Map and check

Map syntax:

```text
elfcmp map REFERENCE TARGET_SYSROOT MAPPING_TEMPLATE \
  [--system-lib-search-paths PATHS] [--lib-search-paths HOST_PATHS]
```

Example:

```sh
elfcmp map bundle/elfcmp-reference.yaml /target/sysroot mapping-template.yaml \
  --system-lib-search-paths /lib64:/usr/lib64 \
  --lib-search-paths /opt/elfcmp/lib:/work/compat
```

- `REFERENCE` is the YAML reference table produced by `elfcmp copy`.

- `TARGET_SYSROOT` is the root filesystem whose system libraries are checked
  for the required functions and symbol versions.

- `MAPPING_TEMPLATE` is the required output path. It has no default. `map`
  writes every function missing from the target with identical `from` and `to`
  endpoints for the user to edit.

- `--system-lib-search-paths` supplies additional colon-separated library
  directories inside `TARGET_SYSROOT`. Its default is empty. For example,
  `/lib64:/usr/lib64` means `TARGET_SYSROOT/lib64` and
  `TARGET_SYSROOT/usr/lib64`. The built-in search directories described in the
  copy step are still searched.

- `--lib-search-paths` supplies colon-separated directories on the host
  filesystem. They are searched before the sysroot paths and are not
  interpreted relative to `TARGET_SYSROOT`. Its default is empty.

An unedited entry looks like:

```yaml
format: 1
mappings:
- from:
    library: libc.so.6
    symbol: old_api
    version: GLIBC_2.17
  to:
    library: libc.so.6
    symbol: old_api
    version: GLIBC_2.17
```

The user can then change `to.library`, `to.symbol`, and `to.version` to the
intended replacement. Omitting `to.version` requests an unversioned target.

Check syntax:

```text
elfcmp check REFERENCE TARGET_SYSROOT MAPPING \
  [--system-lib-search-paths PATHS] [--lib-search-paths HOST_PATHS]
```

Example:

```sh
elfcmp check bundle/elfcmp-reference.yaml /target/sysroot mapping.yaml \
  --system-lib-search-paths /lib64:/usr/lib64 \
  --lib-search-paths /opt/elfcmp/lib:/work/compat
```

- `REFERENCE` and `TARGET_SYSROOT` have the same meanings as in `map`.

- `MAPPING` is the completed mapping file to validate. It has no default.

- `--system-lib-search-paths` has the same meaning and empty default as in
  `map`.

- `--lib-search-paths` has the same host-directory meaning, precedence, and
  empty default as in `map`.

`check` verifies that mapping sources occur in the reference table, every
target-missing reference has a mapping, mappings do not duplicate a source, and
each target library exports the requested function and version. For versioned
targets it also verifies that every affected object can reuse the target
version name from a requirement recorded by `copy`. The library associated
with that existing requirement may differ from the mapped target library.

A source sysroot and bundle path are not needed: the required functions,
per-object imports, and existing version requirements are already stored in
`REFERENCE`.

An unversioned symbol requirement matches an export of the same name whether
that export is unversioned or versioned. A versioned requirement still requires
the exact requested version. The same rule is used by `map`, `check`, and
`patch`; an unversioned requirement is not reported as unresolved merely because
the providing library exposes the symbol with a version.

### 3. Patch

Patch syntax:

```text
elfcmp patch DIRECTORY [--mapping MAPPING] [--patchelf PATCHELF] \
  [--target-sysroot TARGET_SYSROOT] [--system-lib-search-paths PATHS] \
  [--lib-search-paths HOST_PATHS]
```

Example with a mapping:

```sh
elfcmp patch ./bundle --mapping ./mapping.yaml --patchelf /usr/bin/patchelf \
  --target-sysroot /target/sysroot --system-lib-search-paths /lib64:/usr/lib64 \
  --lib-search-paths /opt/elfcmp/lib:/work/compat
```

- `DIRECTORY` is the bundle directory produced by `copy`. It has no default.
  Every ELF file below it is considered for patching.

- `--mapping` supplies the completed and preferably checked mapping file. Its
  default is unset. With a mapping, `patch` renames dynamic symbols, rewrites
  reusable symbol-version indices, and adds missing `DT_NEEDED` entries.

- `--patchelf` selects the `patchelf` executable. Its default is `patchelf`,
  resolved through `PATH`.

- `--lib-search-paths` supplies colon-separated host directories used to
  validate every mapped target library, symbol, version, and ELF architecture
  before patching. Host directories are searched first. Its default is empty.

- `--target-sysroot` selects the target root filesystem used for the same
  pre-patch validation. `--system-lib-search-paths` supplies colon-separated
  directories interpreted beneath that sysroot, followed by the built-in
  system directories. If validation is enabled without `--target-sysroot`, its
  default is `/`.

Supplying any of these three search options enables target-export validation.
They are used only for checking and never copy libraries into the bundle.

For every run, including one without `--mapping`, the executable RPATH is set
to `$ORIGIN/lib` and library RPATHs are set to `$ORIGIN`. Therefore this command
performs RPATH-only patching:

```sh
elfcmp patch ./bundle
```

For a versioned replacement, the target version name must already occur in each
importing object's `.gnu.version_r` table; it may be associated with any
library. `check` reports mappings that cannot reuse such an entry, and `patch`
repeats the check before changing any file. The mapped target library must still
export the mapped symbol at that exact version. In the supported case, `elfcmp`
rewrites the existing 16-bit
`.gnu.version` index without resizing the ELF. Unversioned targets use the global
version index. Adding brand-new `.gnu.version_r` records is not yet supported.

## Commands

Run `elfcmp help` or `elfcmp <command> --help` for all options.

### Library logging

Every command logs library use to standard error with its operation name.
`copy`, `map`, and `check` report resolved SONAMEs and filesystem paths. `map`
and `check` log and inspect each library only once even when many symbols use
it. `check` additionally logs every unresolved mapping with the library, symbol,
and version of both its `from` and `to` endpoints; an absent version is shown as
`<unversioned>`.
`patch` reports each shared-library object it processes, every `DT_NEEDED`
library it observes, and every library it adds. Standard error is used so these
logs can be redirected independently from normal command output.

## Integration tests

Reproducible tests for unversioned compatibility symbols and reuse of an
existing GNU symbol-version requirement are documented in
[`integration/README.md`](integration/README.md). They compile against explicit
Ubuntu and CentOS sysroots and retain their new temporary bundles for
inspection. The combined runner removes artifacts from earlier runs first.
