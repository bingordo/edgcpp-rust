/* M14: inspect EDG if/return branches and all six native FP predicates. */
double m14_cmp_lt(double a,double b) { if (a < b) return a; else return b; }
double m14_cmp_le(double a,double b) { if (a <= b) return a; else return b; }
double m14_cmp_gt(double a,double b) { if (a > b) return a; else return b; }
double m14_cmp_ge(double a,double b) { if (a >= b) return a; else return b; }
double m14_cmp_eq(double a,double b) { if (a == b) return a; else return b; }
double m14_cmp_ne(double a,double b) { if (a != b) return a; else return b; }
float m14_cmp_f32(float a,float b) { if (a != b) return a; else return b; }
