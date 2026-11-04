/* Native EDG/Cranellift FP reference: no unmodeled locals, globals or
 * imports, so signatures and FP instructions are directly testable. */
double m14_double_add(double a,double b){return a+b;}
double m14_double_sub(double a,double b){return a-b;}
double m14_double_mul(double a,double b){return a*b;}
double m14_double_div(double a,double b){return a/b;}
double m14_double_neg(double a){return -a;}
float m14_float_add(float a,float b){return a+b;}
float m14_float_mul(float a,float b){return a*b;}
float m14_float_div(float a,float b){return a/b;}
double m14_double_promote(double a,float b){return a+(double)b;}
float m14_float_demote(float a,double b){return a-(float)b;}
double m14_double_stack(double a,double b,double c,double d,double e,double f,double g,double h,double i,double j){return i+j;}
