/* A returning if without else still has a fallthrough edge.
 * This edge must not invent an initialized local. */
double m14_bad_if_return_fallthrough(double a,double b) {
    double v;
    if (a>b) return a;
    return v;
}
