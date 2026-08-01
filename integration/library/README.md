# Shared-library copy fixture

This fixture compiles `liba.b.c.so.1.2.3` and passes it to `elfcmp copy`.
Because the input filename matches the shared-library pattern, the input and any
copied dependencies are placed directly in the bundle root; no additional
`lib/` directory is created. RPATH-only patching sets its RPATH to `$ORIGIN`.

Run from the repository root:

```bash
bash integration/library/test.sh
```

