/* Never manufacture an undefined pointer SSA value. */
double m14_ptrlocal_bad_uninit(double x) {
    double *copy;
    return *copy+x;
}
