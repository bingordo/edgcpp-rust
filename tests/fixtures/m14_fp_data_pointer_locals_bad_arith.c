/* Pointer arithmetic requires a separate provenance/alias model. */
double m14_ptrlocal_bad_arith(double *p) {
    double *copy=p+1;
    return *copy;
}
