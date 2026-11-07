/* The FP conditional-value path accepts native float/double result arms.
 * Integer-valued conditional expressions with explicit C int conversions
 * belong to the separate typed integer expression lowering path. */
double m14_truth_bad_ternary(double x,double y,double z) {
    if (x ? (int)y : (int)z) return 1.0;
    return 0.0;
}
