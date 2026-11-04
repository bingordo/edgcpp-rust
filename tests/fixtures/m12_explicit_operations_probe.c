/* Characterization only: inspect EDG's owned IL before implementing
 * source-visible explicit-order atomics. This is NOT executable M12 coverage. */
#include <stdatomic.h>
int m12_probe_acquire(_Atomic int *object) {
    return atomic_load_explicit(object,memory_order_acquire);
}
void m12_probe_release(_Atomic int *object,int desired) {
    atomic_store_explicit(object,desired,memory_order_release);
}
int m12_probe_exchange(_Atomic int *object,int desired) {
    return atomic_exchange_explicit(object,desired,memory_order_acq_rel);
}
int m12_probe_compare_exchange(_Atomic int *object,int desired) {
    int expected=7;
    return atomic_compare_exchange_strong_explicit(
        object,&expected,desired,memory_order_acq_rel,memory_order_acquire);
}
int m12_probe_fetch(_Atomic int *object,int operand) {
    return atomic_fetch_add_explicit(object,operand,memory_order_relaxed);
}
void m12_probe_fence(void) {
    atomic_thread_fence(memory_order_seq_cst);
}
