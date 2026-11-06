/* A source declaration without a full C prototype is *not* a verified
 * external-call ABI and must fail closed, even if linkable by C rules. */
extern double m14_not_defined_here();
double m14_bad_external(double x) {return m14_not_defined_here(x);}
