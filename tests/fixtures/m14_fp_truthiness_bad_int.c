/* Integer-valued C scalar conditions are owned by a different typed path. */
double m14_truth_bad_int(double x) {
    if((int)x) return x;
    return 0.0;
}
