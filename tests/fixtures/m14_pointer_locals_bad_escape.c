/* Address-taken locals require a memory / aliasing model. */
double m14_bad_ptr_escape(double (*fn)(double),double x) {
    double (*local)(double)=fn;
    double (**address)(double)=&local;
    return (*address)(x);
}
