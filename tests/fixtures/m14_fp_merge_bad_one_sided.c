/* The implicit false edge cannot read v: only the true edge initializes it. */
double m14_join_bad_one_sided(double a,double b) {
    double v;
    if (a > b) v = a;
    return v;
}
