/* Local pointer aliases and address-taking need a separate memory model. */
double m14_fp_pointer_bad_local(double x) {
    double local=x;
    double *p=&local;
    return *p;
}
