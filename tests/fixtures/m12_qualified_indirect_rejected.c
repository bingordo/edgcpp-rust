/* Atomic indirect RMW needs one indivisible instruction.
 * The first pointer-dereference implementation must not lower to
 * separate load and store operations. */
int m12_indirect_atomic_bad(_Atomic int *p) { return (*p)++; }
