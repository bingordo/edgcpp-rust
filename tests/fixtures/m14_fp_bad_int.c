/* Mixed INTEGER/SSE formals are accepted, but general integer expression
 * semantics must not be guessed by the FP-only expression lowering. */
double m14_bad_mixed(int a,double b) {
    return (double)(a*2)+b;
}
