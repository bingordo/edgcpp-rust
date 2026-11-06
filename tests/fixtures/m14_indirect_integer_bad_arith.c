/* INTEGER return arithmetic cannot be silently interpreted as an FP tree. */
int m14_bad_indirect_int_arith(int (*fn)(double),double x) {
    return fn(x)+1;
}
