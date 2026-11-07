/* Only one CFG predecessor initializes the pointer. */
double m14_ptrlocal_bad_branch(double *p,double x) {
    double *copy;
    if(x>0.0) copy=p;
    return *copy;
}
