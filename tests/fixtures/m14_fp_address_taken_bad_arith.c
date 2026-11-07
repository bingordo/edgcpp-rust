/* Pointer arithmetic must never fabricate an unverified FP stack address. */
double m14_addr_arithmetic(double x) {
    double local=x;
    double *p=&local+1;
    return *p;
}
