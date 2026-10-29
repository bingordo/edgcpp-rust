/* M6 GNU statement-expression semantics: compare object output against GCC.
 * Compile as GNU C11 (-std=gnu11), not strict ISO C11.
 */
int m6_gnu_simple(int x) {
    return ({ int y = x + 3; y * 2; });
}
int m6_gnu_nested(int x) {
    int y = x;
    return ({ int z = ({ int t = y + 1; t * 2; }); y += 3; z + y; });
}
int m6_gnu_order(int x) {
    int n = x;
    int value = ({ n += 2; n *= 3; n - 1; });
    return value + n;
}
int m6_gnu_short_circuit(int x) {
    int n = 0;
    int y = x && ({ n += 1; x > 2; });
    return y + 10 * n;
}
int m6_gnu_conditional(int x) {
    int n = 1;
    int y = x ? ({ n += 2; n; }) : ({ n += 4; n; });
    return y * 10 + n;
}
