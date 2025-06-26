#include <stdio.h>

// a function
int add(int a, int b) {
    return a + b;
}

static void my_static_func() {
    // do nothing
}

void no_return() {
    printf("hello");
}

// another function
char* get_name() {
    return "test";
}

int main(int argc, char** argv) {
    if (argc > 1) {
        for (int i = 0; i < argc; i++) {
            printf("%s\n", argv[i]);
        }
    }
    return 0;
}
