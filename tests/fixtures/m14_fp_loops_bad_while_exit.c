/* A while's condition can be false before its first body assignment. */
double m14_bad_while_exit(double a,double b) {
    double result;
    while(a<b) { result=a; a=a+1.0; }
    return result;
}
