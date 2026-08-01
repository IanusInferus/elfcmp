#include <stddef.h>

void elfcmp_libdl_marker(void) {}

void *dlopen(const char *filename, int flags) {
    (void)filename;
    (void)flags;
    return (void *)0x1234;
}

