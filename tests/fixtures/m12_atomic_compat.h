/* Test-only C11 API shim for the pinned EDG build that has no stdatomic.h.
 * It is not installed as a system C standard library implementation.
 * GNU atomics retain literal C11 memory orders in EDG's native call IL. */
#ifndef EDGCPP_M12_ATOMIC_COMPAT_H
#define EDGCPP_M12_ATOMIC_COMPAT_H
typedef enum {
    memory_order_relaxed=0, memory_order_consume=1,
    memory_order_acquire=2, memory_order_release=3,
    memory_order_acq_rel=4, memory_order_seq_cst=5
} memory_order;
#define atomic_load_explicit(P,O) __atomic_load_n((P),(O))
#define atomic_store_explicit(P,V,O) __atomic_store_n((P),(V),(O))
#define atomic_exchange_explicit(P,V,O) __atomic_exchange_n((P),(V),(O))
#define atomic_compare_exchange_strong_explicit(P,E,V,S,F) \
    __atomic_compare_exchange_n((P),(E),(V),0,(S),(F))
#define atomic_compare_exchange_weak_explicit(P,E,V,S,F) \
    __atomic_compare_exchange_n((P),(E),(V),1,(S),(F))
#define atomic_fetch_add_explicit(P,V,O) __atomic_fetch_add((P),(V),(O))
#define atomic_fetch_sub_explicit(P,V,O) __atomic_fetch_sub((P),(V),(O))
#define atomic_fetch_and_explicit(P,V,O) __atomic_fetch_and((P),(V),(O))
#define atomic_fetch_or_explicit(P,V,O) __atomic_fetch_or((P),(V),(O))
#define atomic_fetch_xor_explicit(P,V,O) __atomic_fetch_xor((P),(V),(O))
#define atomic_thread_fence(O) __atomic_thread_fence((O))
#define atomic_signal_fence(O) __atomic_signal_fence((O))
#endif
