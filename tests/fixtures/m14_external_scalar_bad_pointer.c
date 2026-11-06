/* External pointer parameters need independent memory/pointer semantics. */
extern double m14_ext_pointer(const double *x,double y);
double m14_wrap_external_bad_pointer(double x,double y) {
    return m14_ext_pointer(&x,y);
}
