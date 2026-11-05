/* The first do-while test is reachable without initialization. */
double m14_bad_do_test(double a,double b) {
    double v;
    do {
        if(a<b) continue;
        v=a;
    } while(v<b);
    return a;
}
