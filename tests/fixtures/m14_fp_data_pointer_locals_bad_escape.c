/* Taking the address of an FP automatic object remains unsupported. */
double m14_ptrlocal_bad_escape(double x) {
    double local=x;
    double *copy=&local;
    return *copy;
}
