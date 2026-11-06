/* C11 f() is unspecified-parameter declaration, not a checked prototype. */
extern double m14_ext_unprototyped();
double m14_wrap_external_bad_unprototyped(double x) {
    return m14_ext_unprototyped(x);
}
