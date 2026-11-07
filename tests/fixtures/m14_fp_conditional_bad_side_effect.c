/* Assignment expression arms need a typed memory/sequence contract. */
extern double m14_ternary_sink;
double m14_ternary_bad_side_effect(double x,double a,double b) {
    return x ? (m14_ternary_sink=a) : b;
}
