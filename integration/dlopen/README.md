# `dlopen` existing-version integration fixture

This fixture tests:

```text
libc.so.6/dlopen@GLIBC_2.34
  -> libdl.so.2/dlopen@GLIBC_2.2.5
```

`marker.c` supplies `elfcmp_libdl_marker@GLIBC_2.2.5` from a link-time
`libdl.so.2`. Consequently, `hello` already has the target
`libdl.so.2/GLIBC_2.2.5` record in `.gnu.version_r`, while its `dlopen` import
comes from the host libc at `GLIBC_2.34`. This is the precondition needed to
test reuse of an existing `.gnu.version` index without growing version tables.

`target_libdl.c` is a controlled runtime test double exporting both symbols at
`GLIBC_2.2.5`. The test also validates `mapping.yaml` separately against the
real CentOS 7 `libdl.so.2` extracted from the supplied glibc RPM.

The `map` command writes `mapping-template.yaml` into the printed test
artifact directory. Its `dlopen@GLIBC_2.34` entry maps to itself; the checked-in
`mapping.yaml` demonstrates how the user changes its target to
`libdl.so.2/dlopen@GLIBC_2.2.5`.
The retained `symbols_before.txt` and `symbols_after.txt` files show the dynamic
symbol-version change directly.
