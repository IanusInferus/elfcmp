#define _GNU_SOURCE
/* Source-side GLIBC_2.25 import for the unversioned compatibility test. */
#include <stdio.h>
#include <string.h>

int main(void) {
    char secret[] = "erase me";
    explicit_bzero(secret, sizeof(secret));
    if (secret[0] != '\0') {
        fputs("explicit_bzero failed\n", stderr);
        return 1;
    }
    puts("hello from elfcmp");
    return 0;
}
