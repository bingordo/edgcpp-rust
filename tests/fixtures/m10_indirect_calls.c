/* M10 indirect-call ABI differential fixture.
 * Compile this translation unit with EDG and GCC independently; link each
 * with m10_indirect_driver.c compiled by GCC. No undefined behavior. */
typedef long (*m10_callback)(long, long);
extern long m10_external(long, long);
static long m10_local(long x, long y) { return x * 3 + y; }

long m10_invoke(m10_callback fn, long a, long b) {
    return fn(a, b);
}
long m10_external_address(long a, long b) {
    m10_callback fn = &m10_external;
    return fn(a, b);
}
long m10_local_address(long a, long b) {
    m10_callback fn = m10_local;
    return fn(a, b);
}
long m10_choose(int choose_external, long a, long b) {
    m10_callback fn = choose_external ? m10_external : m10_local;
    return fn(a, b);
}
long m10_callback_twice(m10_callback fn, long a) {
    long first = fn(a, 7);
    return first + fn(first, -3);
}
