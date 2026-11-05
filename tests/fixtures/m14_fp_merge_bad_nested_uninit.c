/* The inner false edge leaves v uninitialized, despite the outer else. */
double m14_join_bad_nested_uninit(double a,double b) {
    double v;
    if (a>b) {
        if (a>0.0) v=a;
    } else v=b;
    return v;
}
