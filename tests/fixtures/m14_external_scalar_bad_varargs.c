/* Variadic external ABI classification must not be guessed. */
extern double m14_ext_varargs(double x,...);
double m14_wrap_external_bad_varargs(double x) {
    return m14_ext_varargs(x,1);
}
