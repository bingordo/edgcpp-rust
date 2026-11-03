/* A C11 atomic multiply needs a CAS loop, not a split load/store. */
extern _Atomic int m12_counter;
int m12_atomic_bad(void) { return m12_counter *= 3; }
