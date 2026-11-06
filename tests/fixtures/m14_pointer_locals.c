/* Native EDG function-pointer locals as nonescaping, checked SSA values. */
double m14_ptr_initial(double (*fp)(double),double x) {
    double (*local)(double)=fp;
    return local(x);
}
double m14_ptr_chain(double (*fp)(double),double x) {
    double (*first)(double)=fp;
    double (*second)(double)=first;
    return second(x);
}
double m14_ptr_reassign(double (*a)(double),double (*b)(double),double x) {
    double (*local)(double)=a;
    local=b;
    return local(x);
}
double m14_ptr_branch(double (*a)(double),double (*b)(double),double x,double t) {
    double (*local)(double)=a;
    if(t>0.0) local=b;
    return local(x);
}
double m14_ptr_loop(double (*fp)(double),double x,double bound) {
    double (*local)(double)=fp;
    double value=x;
    while(value<bound) value=local(value);
    return value;
}
long m14_ptr_integer(long (*fp)(double),double x) {
    long (*local)(double)=fp;
    return local(x);
}
