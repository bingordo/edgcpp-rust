/* M14: SSA join after if/else with per-arm scalar FP assignment. */
double m14_join_absdiff(double a,double b) {
    double v;
    if (a < b) v = b-a; else v = a-b;
    return v;
}
double m14_join_ordered(double a,double b) {
    double x = a;
    double y = b;
    if (a != b) {
        x = a+b;
        y = x*2.0;
    } else {
        y = a-b;
        x = y+1.0;
    }
    return x-y;
}
double m14_join_sequential(double a,double b,double c) {
    double v = a;
    if (a > b) v = a-b; else v = b-a;
    if (v > c) v = v-c; else v = v+c;
    return v;
}
float m14_join_f32(float a,float b) {
    float v;
    if (a > b) v = a; else v = b;
    return v;
}
double m14_join_existing(double a,double b) {
    double x = a;
    double y = b;
    if (a >= b) x = a+1.0; else y = b+2.0;
    return x+y;
}
double m14_join_nan(double a,double b) {
    double v;
    if (a != b) v = 10.0; else v = 20.0;
    return v;
}

/* M14: absent else is a real false CFG edge, carrying the incoming SSA value. */
double m14_if_no_else(double a,double b) {
    double v = b;
    if (a > b) v = a + 2.0;
    return v;
}
double m14_if_no_else_chain(double a,double b,double c) {
    double x = a;
    double y = b;
    if (a > b) {
        x = x+b;
        y = x*2.0;
    }
    if (x < c) x = x-c;
    return x+y;
}
float m14_if_no_else_f32(float a,float b) {
    float v = b;
    if (a != b) v = a;
    return v;
}
double m14_if_empty_else(double a,double b) {
    double v = b;
    if (a < b) v = a+b; else {}
    return v;
}
double m14_if_empty_then(double a,double b) {
    double v = a;
    if (a > b) {} else v = b+1.0;
    return v;
}

/* Nested assignment-only conditions use two levels of SSA joins. */
double m14_join_nested(double a,double b) {
    double v=a;
    if (a>b) {
        if (a>0.0) v=a+1.0; else v=a-1.0;
    } else v=b;
    return v;
}
double m14_join_nested_no_else(double a,double b) {
    double v=b;
    if (a>b) {
        if (a>0.0) v=a+2.0;
    }
    return v;
}
double m14_join_nested_both_arms(double a,double b) {
    double v=a;
    if (a!=b) {
        if (a<b) v=b-a; else v=a-b;
    } else {
        if (a>0.0) v=a+1.0; else v=b-1.0;
    }
    return v;
}
