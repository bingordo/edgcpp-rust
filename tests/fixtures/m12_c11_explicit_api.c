/* The pinned EDG binary lacks the system stdatomic.h include path.
 * Exercise C11 surface spellings through a test-only macro compatibility
 * header which preserves native GNU builtin operation/order operands. */
#include "m12_atomic_compat.h"
int m12_explicit_load(_Atomic int *p) {
    return atomic_load_explicit(p,memory_order_acquire);
}
void m12_explicit_store(_Atomic int *p,int value) {
    atomic_store_explicit(p,value,memory_order_release);
}
int m12_explicit_exchange(_Atomic int *p,int value) {
    return atomic_exchange_explicit(p,value,memory_order_acq_rel);
}
int m12_explicit_fetch(_Atomic int *p,int value) {
    int a=atomic_fetch_add_explicit(p,value,memory_order_relaxed);
    int b=atomic_fetch_sub_explicit(p,2,memory_order_acq_rel);
    int c=atomic_fetch_xor_explicit(p,0x13,memory_order_seq_cst);
    int d=atomic_fetch_and_explicit(p,255,memory_order_acquire);
    int e=atomic_fetch_or_explicit(p,0x40,memory_order_release);
    return a+b+c+d+e;
}
int m12_explicit_cas(_Atomic int *p,int *expected,int desired) {
    return atomic_compare_exchange_strong_explicit(
        p,expected,desired,memory_order_acq_rel,memory_order_acquire);
}
int m12_explicit_weak_cas(_Atomic int *p,int *expected,int desired) {
    return atomic_compare_exchange_weak_explicit(
        p,expected,desired,memory_order_seq_cst,memory_order_acquire);
}
void m12_explicit_fence(void) {
    atomic_thread_fence(memory_order_seq_cst);
}
void m12_explicit_signal_fence(void) {
    atomic_signal_fence(memory_order_acquire);
}
void m12_explicit_publish(_Atomic int *ready,int *payload,int value) {
    *payload=value;
    atomic_store_explicit(ready,value,memory_order_release);
}
int m12_explicit_read(_Atomic int *ready,int *payload,int expected) {
    while(atomic_load_explicit(ready,memory_order_acquire)!=expected) {}
    return *payload;
}
