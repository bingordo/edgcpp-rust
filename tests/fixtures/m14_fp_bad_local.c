/* M14 negative: nested FP branches need general typed CFG lowering. */
double m14_bad_local(double a) {
    double b = a + 1.0;
    if (a > 0.0) {
        if (a > 1.0) b = b + 2.0;
        else b = b - 2.0;
    }
    return b;
}
