/* Pointer truthiness is outside the checked FP expression subset. */
double m14_ternary_bad_pointer(const double *p,double x,double y) {
    return p ? x : y;
}
