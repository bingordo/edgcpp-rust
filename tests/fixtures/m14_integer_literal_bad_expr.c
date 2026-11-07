/* INTEGER arithmetic is not yet part of the checked FP expression tree. */
double m14_bad_int_arithmetic(unsigned long long x) {
    return (double)(x + 1ULL);
}
