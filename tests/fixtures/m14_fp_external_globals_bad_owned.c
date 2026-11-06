/* FP global definitions need byte-accurate .data/.bss initialization. */
double m14_fp_owned=3.0;
double m14_bad_fp_owned(double x) {
    return m14_fp_owned+x;
}
