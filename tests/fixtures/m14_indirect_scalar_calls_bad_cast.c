/* Source cast to a different callable prototype is not a verified formal. */
double m14_bad_indirect_cast(double (*fp)(double),double x) {
    return ((double (*)(float))fp)((float)x);
}
