/* M14 C11 typed local SSA: initializer value, repeated update, and return. */
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
double m14_local_stack(double a,double b,double c,double d,double e,
                       double f,double g,double h,double i,double j) {
    double t = i;
    t = t + j;
    return t;
}
float m14_local_demote(float a, double b) {
    float t = (float)b;
    t = t - a;
    return t;
}
