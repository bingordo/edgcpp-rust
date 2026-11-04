/* Native EDG FP predicates: all six C operators, f32/f64, and FP locals.
 * Branch results are distinct constants to test NaN/unordered control flow. */
double m14_cmp_eq(double a,double b) { if (a==b) return 11.0; else return 22.0; }
double m14_cmp_ne(double a,double b) { if (a!=b) return 11.0; else return 22.0; }
double m14_cmp_lt(double a,double b) { if (a< b) return 11.0; else return 22.0; }
double m14_cmp_le(double a,double b) { if (a<=b) return 11.0; else return 22.0; }
double m14_cmp_gt(double a,double b) { if (a> b) return 11.0; else return 22.0; }
double m14_cmp_ge(double a,double b) { if (a>=b) return 11.0; else return 22.0; }
float m14_cmp_f32(float a,float b) { if (a!=b) return 11.0f; else return 22.0f; }
double m14_cmp_local(double a,double b) {
    double local = a + 1.0;
    if (local >= b) return local; else return b;
}
