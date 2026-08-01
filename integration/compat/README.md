# Unversioned compatibility-symbol fixture

This fixture tests:

```text
libc.so.6/explicit_bzero@GLIBC_2.25
  -> libcompat.so.1/elfcmp_explicit_bzero (unversioned)
```

`hello.c` is compiled against the Ubuntu sysroot and imports the versioned
glibc symbol. `compat.c` provides the default versioned export
`elfcmp_explicit_bzero@@COMPAT_1.0`; the mapping deliberately leaves the target
version unspecified. The test
verifies the CentOS comparison reports the original symbol as missing, checks
the mapping, patches the copied bundle, confirms both `$ORIGIN` RPATHs, and
executes the result.

The `map` command writes `mapping-template.yaml` into the printed test artifact
directory. It initially maps `explicit_bzero@GLIBC_2.25` to itself. Compare that
generated template with `mapping.yaml`, which shows the user-edited replacement.
The retained `symbols_before.txt` and `symbols_after.txt` files show the dynamic
symbol table on either side of patching.

Run from the repository root:

```bash
bash integration/compat/test.sh
```

`mapping-versioned.yaml` and `compat.map` are retained as negative-test inputs
for rejecting a target version name that is not already present anywhere in
the object's `.gnu.version_r`. The providing library does not have to match the
version-requirement library.
