/* Early returns inside nested conditional updates are still unsupported. */
double m14_join_bad_nested(double a,double b) {
    double v=a;
    if (a>b) {
        if (a>0.0) return a+1.0;
        v=a-1.0;
    } else v=b;
    return v;
}
