/* General C ?: expressions require typed control/select lowering. */
double m14_truth_bad_ternary(double x,double y,double z) {
    if(x ? y : z) return 1.0;
    return 0.0;
}
