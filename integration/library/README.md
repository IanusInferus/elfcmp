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

Before patching, the fixture also places a shared object in the bundle under
the loader-style name `ld-linux-x86-64.so.2`. It verifies that RPATH selection
uses ELF metadata rather than a `lib*.so` filename heuristic, so this object
receives `$ORIGIN` rather than `$ORIGIN/lib`.

Run from the repository root:

```bash
bash integration/library/test.sh
```
