/* This subset forbids reading an uninitialized pointer local. */
double m14_bad_ptr_uninit(double (*fn)(double),double x) {
    double (*local)(double);
    return local(x);
}
