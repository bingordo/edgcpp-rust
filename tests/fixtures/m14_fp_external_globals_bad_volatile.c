/* Volatile observable global loads are not ordinary SSA memory effects. */
extern volatile double m14_fp_volatile;
double m14_bad_fp_volatile(double x) {
    return m14_fp_volatile+x;
}
