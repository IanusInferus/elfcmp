#include <stdio.h>

extern int elfcmp_dependency_value(void);

void elfcmp_library_hello(void) {
    if (elfcmp_dependency_value() == 42) {
        puts("hello from the shared-library bundle");
    }
}
