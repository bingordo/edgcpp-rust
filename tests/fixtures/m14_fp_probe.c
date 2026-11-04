/* M14 pinned EDG: inspect native float types and operation trees. */
double m14_double_add(double a, double b) { return a+b; }
double m14_double_sub(double a, double b) { return a-b; }
double m14_double_mul(double a, double b) { return a*b; }
double m14_double_div(double a, double b) { return a/b; }
double m14_double_neg(double a) { return -a; }
float m14_float_add(float a, float b) { return a+b; }
float m14_float_mul(float a, float b) { return a*b; }
float m14_float_div(float a, float b) { return a/b; }
double m14_double_mixed(double a, float b) { return a+(double)b; }
double m14_double_const(double a) { return a+1.25; }
float m14_float_const(float a) { return a+2.5f; }
double m14_double_cast_int(int a) { return (double)a; }
int m14_double_compare(double a,double b) {return a<b;}
