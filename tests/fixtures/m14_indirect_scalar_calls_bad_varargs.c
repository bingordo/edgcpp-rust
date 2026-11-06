/* Callable pointer varargs cannot be guessed as ordinary scalar ABI. */
double m14_bad_indirect_varargs(double (*fp)(double,...),double x) {
    return fp(x,1);
}
