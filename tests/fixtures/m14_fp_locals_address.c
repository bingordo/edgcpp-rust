/* Address-taken floating point locals require actual memory/lvalue handling. */
double m14_address(double a) {
    double t = a;
    double *p = &t;
    return *p;
}
