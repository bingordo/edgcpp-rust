/* Nested control flow is deliberately outside the first merge slice. */
double m14_join_bad_nested(double a,double b) {
    double v = a;
    if (a > b) {
        if (a > 0.0) v=a+1.0; else v=a-1.0;
    } else v=b;
    return v;
}
