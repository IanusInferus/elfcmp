# elfcmp

`elfcmp` (ELF copy-map-patch) builds relocatable Linux application bundles while
making ABI differences between source and target sysroots explicit.

## Build

```sh
cargo build --release
```

ELF inspection is implemented in Rust. The `patch` command invokes
[`patchelf`](https://github.com/NixOS/patchelf), which must be installed on the
machine where patching is performed.

## Workflow

### 1. Copy

Copy syntax:

```text
elfcmp copy EXECUTABLE OUTPUT \
  [--sysroot SYSROOT] \
  [--system-lib-search-paths PATHS] \
  [--system-lib-basenames BASENAMES] \
  [--reference REFERENCE]
```

Example:

```sh
elfcmp copy ./my-program ./bundle \
  --sysroot /source/sysroot \
  --system-lib-search-paths /lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu \
  --reference ./bundle/elfcmp-reference.yaml
```

The executable is copied to `bundle/`, non-system dependencies are recursively
copied to `bundle/lib/`, and `bundle/elfcmp-reference.yaml` records imported
symbols supplied by skipped system libraries.

- `EXECUTABLE` is the ELF executable to scan and copy. It has no default.

- `OUTPUT` is the bundle destination. It has no default. The executable is
  copied to the root of this directory and copied shared libraries are placed
  in `OUTPUT/lib/`.

- `--sysroot` selects the root filesystem used to resolve the executable's
  dependencies. Its default is `/`, which uses the current system root.

- `--system-lib-search-paths` supplies additional colon-separated library
  directories. Absolute entries are interpreted inside `--sysroot`, so
  `/lib/x86_64-linux-gnu` in the example means
  `/source/sysroot/lib/x86_64-linux-gnu`. Its default is empty. The built-in
  search directories are `lib`, `lib64`, `usr/lib`, `usr/lib64`, and
  `usr/local/lib`, plus one-level multiarch directories beneath `lib` and
  `usr/lib`.

- `--reference` selects the reference-table output path. Its default is
  `OUTPUT/elfcmp-reference.yaml`, where `OUTPUT` is the `copy` destination. For
  the example destination `./bundle`, the default is therefore already
  `./bundle/elfcmp-reference.yaml`.

- `--system-lib-basenames` identifies system libraries that are referenced but
  not copied. Its default is
  `libc,libdl,libm,libpthread,librt,libselinux`. Supplying the option replaces
  that complete list; it does not add to the defaults. Matching removes only
  the final `.so` and its numeric version suffix: `liba.b.c.so.1.2.3` has
  basename `liba.b.c`. Dots within the library name remain significant. Thus
  `libc` matches `libc.so.6`, but not `libc++.so.1` or `libcrypt.so.1`.

### 2. Map and check

Map syntax:

```text
elfcmp map REFERENCE TARGET_SYSROOT MAPPING_TEMPLATE \
  [--system-lib-search-paths PATHS]
```

Example:

```sh
elfcmp map bundle/elfcmp-reference.yaml /target/sysroot mapping-template.yaml \
  --system-lib-search-paths /lib64:/usr/lib64
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
  [--system-lib-search-paths PATHS]
```

Example:

```sh
elfcmp check bundle/elfcmp-reference.yaml /target/sysroot mapping.yaml \
  --system-lib-search-paths /lib64:/usr/lib64
```

- `REFERENCE` and `TARGET_SYSROOT` have the same meanings as in `map`.

- `MAPPING` is the completed mapping file to validate. It has no default.

- `--system-lib-search-paths` has the same meaning and empty default as in
  `map`.

`check` verifies that mapping sources occur in the reference table, every
target-missing reference has a mapping, mappings do not duplicate a source, and
each target library exports the requested function and version. For versioned
targets it also verifies that every affected object can reuse the target
library/version requirement recorded by `copy`.

A source sysroot and bundle path are not needed: the required functions,
per-object imports, and existing version requirements are already stored in
`REFERENCE`.

### 3. Patch

Patch syntax:

```text
elfcmp patch DIRECTORY [--mapping MAPPING] [--patchelf PATCHELF]
```

Example with a mapping:

```sh
elfcmp patch ./bundle --mapping ./mapping.yaml --patchelf /usr/bin/patchelf
```

- `DIRECTORY` is the bundle directory produced by `copy`. It has no default.
  Every ELF file below it is considered for patching.

- `--mapping` supplies the completed and preferably checked mapping file. Its
  default is unset. With a mapping, `patch` renames dynamic symbols, rewrites
  reusable symbol-version indices, and adds missing `DT_NEEDED` entries.

- `--patchelf` selects the `patchelf` executable. Its default is `patchelf`,
  resolved through `PATH`.

For every run, including one without `--mapping`, the executable RPATH is set
to `$ORIGIN/lib` and library RPATHs are set to `$ORIGIN`. Therefore this command
performs RPATH-only patching:

```sh
elfcmp patch ./bundle
```

For a versioned replacement, the target library/version pair must already occur
in each importing object's `.gnu.version_r` table. `check` reports mappings that
cannot reuse such an entry, and `patch` repeats the check before changing any
file. In the supported case, `elfcmp` rewrites the existing 16-bit
`.gnu.version` index without resizing the ELF. Unversioned targets use the global
version index. Adding brand-new `.gnu.version_r` records is not yet supported.

## Commands

Run `elfcmp help` or `elfcmp <command> --help` for all options.

## Integration tests

Reproducible tests for unversioned compatibility symbols and reuse of an
existing GNU symbol-version requirement are documented in
[`integration/README.md`](integration/README.md). They compile against explicit
Ubuntu and CentOS sysroots and retain their new temporary bundles for
inspection. The combined runner removes artifacts from earlier runs first.
