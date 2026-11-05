/* Continue bypasses the assignment that the increment reads. */
double m14_bad_for_continue(double a,double b) {
    double i=a,step;
    for(;i<b;i=step) {
        if(i>0.0) continue;
        step=i+1.0;
    }
    return i;
}
