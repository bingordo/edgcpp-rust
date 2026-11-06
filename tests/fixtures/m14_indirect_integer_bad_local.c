/* Pointer locals still require memory-backed lifetime and initialization. */
int m14_bad_indirect_int_local(int (*fn)(double),double x) {
    int (*stored)(double)=fn;
    return stored(x);
}
