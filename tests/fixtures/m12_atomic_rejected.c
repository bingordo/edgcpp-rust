/* A split load/store cannot implement a C11 atomic read-modify-write. */
extern _Atomic int m12_counter;
int m12_atomic_bad(void) { return m12_counter++; }
