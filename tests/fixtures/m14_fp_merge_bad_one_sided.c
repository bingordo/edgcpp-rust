/* No else: this slice requires both explicit, verified CFG predecessors. */
double m14_join_bad_one_sided(double a,double b) {
    double v;
    if (a > b) v = a;
    return v;
}
