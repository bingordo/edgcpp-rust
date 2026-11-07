/* C indeterminate INTEGER automatic local must never become SSA undef. */
double m14_il_bad_uninit(double x) {
    int n;
    return x+(double)n;
}
