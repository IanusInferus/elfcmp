# Shared-library copy fixture

This fixture compiles `liba.b.c.so.1.2.3` and passes it to `elfcmp copy`.
Because the input filename matches the shared-library pattern, the input and any
copied dependencies are placed directly in the bundle root; no additional
`lib/` directory is created. RPATH-only patching sets its RPATH to `$ORIGIN`.

The fixture also builds multiple candidates named `libdependency.so.1`. The
input library has `RUNPATH=/right`, while the configured fallback search path
contains a different, architecture-compatible implementation. The test confirms
that `copy` searches the library's RUNPATH first and bundles the `/right`
candidate. A wrong-architecture candidate remains in the fallback paths to
cover architecture filtering.

Run from the repository root:

```bash
bash integration/library/test.sh
```
