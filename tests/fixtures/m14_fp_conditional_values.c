/* M14 source-checked lazy FP-valued C conditional expressions. */
extern double m14_ternary_yes(double x);
extern double m14_ternary_no(double x);
double m14_ternary_truth(double cond,double a,double b) {
    return cond ? a : b;
}
double m14_ternary_compare(double x,double y,double a,double b) {
    return x<y ? a : b;
}
double m14_ternary_nested(double x,double y,double a,double b,double c) {
    return x ? (y ? a : b) : c;
}
float m14_ternary_f32(float x,float a,float b) {
    return x ? a : b;
}
double m14_ternary_call(double x,double a,double b) {
    return x ? m14_ternary_yes(a) : m14_ternary_no(b);
}
double m14_ternary_arithmetic(double x,double a,double b) {
    return 1.0+(x ? a : b);
}
