#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>

extern void elfcmp_libdl_marker(void);

int main(void) {
    elfcmp_libdl_marker();
    void *handle = dlopen("elfcmp-test", RTLD_LAZY);
    if (handle != (void *)0x1234) {
        fputs("mapped dlopen did not come from the test libdl\n", stderr);
        return 1;
    }
    puts("dlopen version mapping passed");
    return 0;
}

