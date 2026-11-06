/* This slice handles INTEGER-returning indirect calls, not external direct. */
extern int m14_integer_external(double x);
int m14_bad_indirect_int_external(double x) {
    return m14_integer_external(x);
}
