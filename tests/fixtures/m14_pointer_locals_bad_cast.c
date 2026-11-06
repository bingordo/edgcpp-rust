/* No unverified function-pointer cast can populate an automatic local. */
double m14_bad_ptr_cast(double (*fn)(double),double x) {
    double (*local)(float)=(double (*)(float))fn;
    return local((float)x);
}
