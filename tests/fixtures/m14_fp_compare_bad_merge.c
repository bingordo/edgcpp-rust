/* Reject a read after a merge unless every predecessor initialized it. */
double m14_cmp_bad_merge(double a,double b) {
    double x;
    double y;
    if (a > b) x = a + 1.0; else y = b + 1.0;
    return x;
}
