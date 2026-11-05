/* M14 negative: volatile FP locals require observable memory effects.
 * Nested nonvolatile FP branches are now supported and tested separately. */
double m14_bad_local(double a) {
    volatile double b=a+1.0;
    if (a>0.0) b=b+2.0;
    return b;
}
