/* Ordinary loads/stores may not erase observable volatile semantics. */
double m14_fp_pointer_bad_volatile(volatile double *p) {
    return *p;
}
