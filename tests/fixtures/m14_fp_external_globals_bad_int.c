/* Global INTEGER-to-FP expression casts require a full typed memory path. */
extern int m14_fp_int_global;
double m14_bad_fp_global_integer(double x) {
    return (double)m14_fp_int_global+x;
}
