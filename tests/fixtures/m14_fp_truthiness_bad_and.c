/* Short-circuit C conditions require a separate side-effect/evaluation model. */
double m14_truth_bad_and(double x,double y) {
    if(x && y) return 1.0;
    return 0.0;
}
