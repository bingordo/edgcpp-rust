/* Must fail closed: do not synthesize a default FP local value. */
double m14_uninit(double a) {
    double t;
    return t + a;
}
