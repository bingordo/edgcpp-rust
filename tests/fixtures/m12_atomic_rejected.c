/* Atomic pointer objects need scaled pointer arithmetic and a pointer-
 * typed atomic ABI; do not silently reinterpret their values as integers. */
extern _Atomic(int *) m12_pointer;
int m12_atomic_bad(void) { return m12_pointer != 0; }
