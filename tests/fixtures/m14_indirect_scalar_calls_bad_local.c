/* Function-pointer locals require memory/definite-assignment support. */
double m14_bad_indirect_local(double (*fp)(double),double x) {
    double (*local)(double)=fp;
    return local(x);
}
