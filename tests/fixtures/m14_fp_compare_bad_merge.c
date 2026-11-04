/* Not supported: local assignments in a conditional need typed CFG merges. */
double m14_cmp_bad_merge(double a,double b) {
    double local = a;
    if (a > b) local=a+1.0; else local=b+1.0;
    return local;
}
