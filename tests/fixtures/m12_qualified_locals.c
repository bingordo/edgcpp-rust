/* Qualified automatic scalar objects must be observable RAM accesses, not
 * ordinary SSA locals. Keep local initialization, direct lvalue updates, and
 * address-taken accesses distinct from unqualified storage. */
int m12_auto_volatile(int seed) {
    volatile int value = seed;
    int before = value;
    value = before + 5;
    int after = value;
    value *= 2;
    return before * 10000 + after * 100 + value;
}
int m12_auto_atomic(int seed) {
    _Atomic int value = seed;
    int old = value++;
    int added = (value += 5);
    int multiplied = (value *= 3);
    return old * 10000 + added * 100 + multiplied + value;
}
int m12_auto_atomic_pointer(int seed) {
    _Atomic int value = seed;
    _Atomic int *p = &value;
    int old = (*p)++;
    return old * 100 + value;
}
