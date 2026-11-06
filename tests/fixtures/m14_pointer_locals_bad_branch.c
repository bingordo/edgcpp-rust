/* Initialization on one predecessor does not dominate the call. */
double m14_bad_ptr_branch(double (*fn)(double),double x) {
    double (*local)(double);
    if(x>0) local=fn;
    return local(x);
}
