/* GCC cross-compiler differential oracle for the bounded M11 VLA subset. */
#include <stdio.h>
#include <stdlib.h>

int m11_simple(int);
int m11_side_effect(int);
int m11_nested(int);

static void check(const char *name, int n, int got, int expected)
{
    if (got != expected) {
        fprintf(stderr, "%s(%d) got=%d expected=%d\n", name, n, got, expected);
        exit(1);
    }
}
int main(void)
{
    const int bounds[] = {1, 2, 3, 7, 31, 1023};
    for (unsigned i = 0; i < sizeof(bounds)/sizeof(bounds[0]); ++i) {
        int n = bounds[i];
        check("simple", n, m11_simple(n), 3*n+1);
        check("side_effect", n, m11_side_effect(n), 2*n+5);
        check("nested", n, m11_nested(n), 36);
    }
    for (int n=1; n<500; ++n) {
        check("repeat nested", n, m11_nested(n), 36);
    }
    return 0;
}
