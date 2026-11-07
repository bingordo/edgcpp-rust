/* Non-FP automatic memory/SSA is deliberately not fabricated. */
double m14_bad_integer_local(double x) {
    int n = 7;
    return x + (double)n;
}
