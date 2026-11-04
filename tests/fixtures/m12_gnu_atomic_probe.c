/* Header-independent characterization of EDG's GCC atomic builtin IL.
 * These builtins implement the operations commonly used by C11 stdatomic.h,
 * but are NOT a claim that codegen supports their memory-order semantics.
 * __ATOMIC_* enumerators: relaxed=0, acquire=2, release=3,
 * acq_rel=4, seq_cst=5. */
int m12_probe_builtin_load(_Atomic int *p) {
    return __atomic_load_n(p, 2);
}
void m12_probe_builtin_store(_Atomic int *p, int value) {
    __atomic_store_n(p, value, 3);
}
int m12_probe_builtin_exchange(_Atomic int *p, int value) {
    return __atomic_exchange_n(p, value, 4);
}
int m12_probe_builtin_compare_exchange(_Atomic int *p, int desired) {
    int expected = 7;
    return __atomic_compare_exchange_n(p, &expected, desired, 0, 4, 2);
}
int m12_probe_builtin_fetch(_Atomic int *p, int value) {
    return __atomic_fetch_add(p, value, 0);
}
void m12_probe_builtin_fence(void) {
    __atomic_thread_fence(5);
}
