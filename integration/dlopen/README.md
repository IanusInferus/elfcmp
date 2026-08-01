# `dlopen` existing-version integration fixture

This fixture tests:

```text
libc.so.6/dlopen@GLIBC_2.34
  -> libdl.so.2/dlopen@GLIBC_2.2.5
```

`marker.c` supplies `elfcmp_libdl_marker@GLIBC_2.2.5` from the link-time
`libversioncarrier.so.1`. Consequently, `hello` already has the version name
`GLIBC_2.2.5` in `.gnu.version_r`, but it is associated with a library other
than the mapped target `libdl.so.2`. Its `dlopen` import comes from the host
libc at `GLIBC_2.34`. This verifies that an existing `.gnu.version` index is
reusable by version name without growing the version tables.

`target_libdl.c` is a controlled runtime test double exporting both symbols at
`GLIBC_2.2.5`. The test also validates `mapping.yaml` separately against the
real CentOS 7 `libdl.so.2` extracted from the supplied glibc RPM.

The `map` command writes `mapping-template.yaml` into the printed test
artifact directory. Its `dlopen@GLIBC_2.34` entry maps to itself; the checked-in
`mapping.yaml` demonstrates how the user changes its target to
`libdl.so.2/dlopen@GLIBC_2.2.5`.
The retained `symbols_before.txt` and `symbols_after.txt` files show the dynamic
symbol-version change directly.
