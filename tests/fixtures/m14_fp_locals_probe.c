/* M14: inspect the exact owned native statement/expression projection for
 * FP local declarations, initialization and reassignment. */
double m14_local_init(double a, double b) {
    double t = a;
    return t * b;
}
float m14_local_assign(float a, float b) {
    float t = a;
    t = t + b;
    return t;
}
double m14_local_two(double a, double b) {
    double x = a;
    double y = b;
    y = x - y;
    return y;
}
double m14_local_uninitialized(double a) {
    double x;
    return x + a; /* only for negative/UB audit, never execute */
}
