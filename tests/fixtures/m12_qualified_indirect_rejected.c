/* Atomic multiplication requires a correctly ordered compare-exchange
 * loop; do not invent a split read/multiply/write sequence. */
int m12_indirect_atomic_bad(_Atomic int *p) { return (*p) *= 3; }
