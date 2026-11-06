/* Address-of global and aliasing not yet modeled in the FP source path. */
double m14_alias_zero;
double m14_bad_alias(double x) {
    double *p=&m14_alias_zero;
    *p=x;
    return m14_alias_zero;
}
