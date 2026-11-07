/* Only exact unmodified pointer formals, not pointer arithmetic. */
double m14_fp_pointer_bad_arith(double *p) {
    return *(p+1);
}
