/* FP-only backend must never silently drop a defined data symbol. */
double m14_global_value = 1.25;
double m14_bad_global(double x) {return x+m14_global_value;}
