/* No mutable store through const-qualified C pointee. */
double m14_fp_pointer_bad_const_store(const double *p,double x) {
    *(double*)p=x;
    return *p;
}
