/* No-condition for: one break edge skips initialization. */
double m14_bad_break_exit(double a) {
    double v;
    for(;;) {
        if(a>0.0) break;
        v=a;
        break;
    }
    return v;
}
