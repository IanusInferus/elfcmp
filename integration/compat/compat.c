#include <stddef.h>

/* Deliberately exported without a GNU symbol version. */

void elfcmp_explicit_bzero(void *buffer, size_t length) {
    volatile unsigned char *cursor = buffer;
    while (length-- != 0) {
        *cursor++ = 0;
    }
}
