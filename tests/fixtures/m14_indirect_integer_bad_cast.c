/* Cast-target indirection is not an exact native function-pointer formal. */
int m14_bad_indirect_int_cast(long (*fn)(double),double x) {
    return ((int (*)(double))fn)(x);
}
