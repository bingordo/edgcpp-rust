/* M14: exact native callable pointer formals, not arbitrary pointer casts.
 * Caller and callee INTEGER/SSE register exhaustion is independent.
 */
double m14_indirect_f64(double (*fn)(double,double),double a,double b) {
    return fn(a,b);
}
float m14_indirect_f32(float (*fn)(float,float),float a,float b) {
    return (*fn)(a,b);
}
double m14_indirect_mixed(
    double (*fn)(int,double,unsigned long,float),
    int a,double x,unsigned long b,float y) {
    return fn(a,x,b,y);
}
double m14_indirect_spill(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,
    int a4,double x4,unsigned int a5,float x5,
    long a6,double x6,double x7,double x8,
    double (*fn)(int,double,unsigned int,float,long,double,
                 unsigned long,float,int,double,unsigned int,float,
                 long,double,double,double)) {
    return fn(a0,x0,a1,x1,a2,x2,a3,x3,a4,x4,a5,x5,a6,x6,x7,x8);
}
double m14_indirect_loop(double (*fn)(double),double x,double bound) {
    double v=x;
    while (v<bound) v=fn(v);
    return v;
}
int m14_indirect_integer_result(double (*fn)(double),double x) {
    return (int)fn(x);
}
