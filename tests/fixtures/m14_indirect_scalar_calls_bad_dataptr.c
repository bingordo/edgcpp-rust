/* A data-pointer-to-function-pointer conversion is not a callable formal. */
double m14_bad_indirect_dataptr(void *p,double x) {
    return ((double (*)(double))p)(x);
}
