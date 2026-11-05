/* Nested branches can return early, but no fallthrough may read a
 * local that only one reachable path initialized. */
double m14_join_bad_nested(double a,double b) {
    double v;
    if (a>b) {
        if (a>0.0) return a+1.0;
        v=a-1.0;
    } else {
        if (b>0.0) return b;
    }
    return v;
}
