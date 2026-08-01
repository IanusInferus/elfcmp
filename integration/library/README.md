# Shared-library copy fixture

This fixture compiles `liba.b.c.so.1.2.3` and passes it to `elfcmp copy`.
Because the input filename matches the shared-library pattern, the input and any
copied dependencies are placed directly in the bundle root; no additional
`lib/` directory is created. RPATH-only patching sets its RPATH to `$ORIGIN`.

The fixture also builds two candidates named `libdependency.so.1`. The first
search directory contains a candidate marked for the wrong ELF machine; the
second contains the matching x86-64 library. The test confirms `copy` rejects
the first and bundles the matching dependency.

Run from the repository root:

```bash
bash integration/library/test.sh
```
