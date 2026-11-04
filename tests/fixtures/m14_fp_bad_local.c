/* M14 negative: branch-dependent FP local updates need general typed CFG. */
double m14_bad_local(double a) {
    double b = a + 1.0;
    if (a > 0.0) b = b + 2.0;
    return b;
}
