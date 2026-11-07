/* Reject source mixed unmodeled integer-global storage in FP module. */
int m14_init_int = 7;
double m14_init_wrong(double x){return x+(double)m14_init_int;}
