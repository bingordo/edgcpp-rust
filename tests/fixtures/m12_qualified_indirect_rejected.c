/* Pointer-valued atomics need their own pointer ABI and CAS semantics. */
int m12_indirect_atomic_bad(_Atomic(int *) *p) { return *p != 0; }
