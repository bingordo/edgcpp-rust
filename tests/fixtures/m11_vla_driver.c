/* GCC cross-compiler differential oracle for the bounded M11 VLA subset. */
#include <stdio.h>
#include <stdlib.h>

int m11_simple(int);
int m11_side_effect(int);
int m11_nested(int);
int m11_multidim(int, int);
int m11_sizeof(int);
int m11_early_return(int);
int m11_loop_exits(int);
int m11_loop_stress(int);

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
        int m = (n % 7) + 1;
        check("multidim", n, m11_multidim(n,m),
              7*n + 3*m + 6 + 7*(n/2) + 3*(m/2));
        check("sizeof", n, m11_sizeof(n), 2*(n+1));
        check("early return", n, m11_early_return(n),
              (n & 1) ? 3*n+4 : 2*n+5);
        int sum = 0;
        for (int k=1; k<=n; ++k) {
            if (k % 3 == 0) continue;
            if (k % 7 == 0) break;
            sum += 3*k+2;
        }
        check("loop exits", n, m11_loop_exits(n), sum);
        check("loop stress", n, m11_loop_stress(n), n*(n+1)/2);
    }
    for (int n=1; n<500; ++n) {
        check("repeat nested", n, m11_nested(n), 36);
        check("reentry", n, m11_loop_stress(n), n*(n+1)/2);
    }
    return 0;
}
