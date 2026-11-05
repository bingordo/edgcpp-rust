/* The first loop header reads an uninitialized local. */
double m14_bad_while_test(double a,double b) {
    double result;
    while(result<b) result=a;
    return a;
}
