/* M14: EDG-verified direct calls to routines defined in this module.
 * Mixed INTEGER and SSE registers must retain their independent ABI classes.
 */
double m14_call_sum(int a,double x,unsigned long b,float y);
float m14_call_f32(float a,float b);
double m14_call_step(double x);
double m14_call_many(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,int a4,double x4,
    unsigned int a5,float x5,long a6,double x6,double x7,double x8);

double m14_call_forward(int a,double x,unsigned long b,float y) {
    return m14_call_sum(a,x,b,y);
}
double m14_call_nested(int a,double x,unsigned long b,float y) {
    return m14_call_sum(a,m14_call_sum(a,x,b,y),b,y);
}
float m14_call_f32_wrap(float a,float b) {
    return m14_call_f32(a,b);
}
double m14_call_loop(int a,double x) {
    double v=x;
    while(v<(double)a) v=m14_call_step(v);
    return v;
}
int m14_call_to_int(double x) {
    return (int)m14_call_step(x);
}
double m14_call_many_wrap(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,int a4,double x4,
    unsigned int a5,float x5,long a6,double x6,double x7,double x8) {
    return m14_call_many(a0,x0,a1,x1,a2,x2,a3,x3,a4,x4,a5,x5,a6,x6,x7,x8);
}
double m14_call_sum(int a,double x,unsigned long b,float y) {
    return (double)a+x+(double)b+(double)y;
}
float m14_call_f32(float a,float b) {
    return a+b;
}
double m14_call_step(double x) {
    return x+1.0;
}
double m14_call_many(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,int a4,double x4,
    unsigned int a5,float x5,long a6,double x6,double x7,double x8) {
    return (double)a0+x0+(double)a1+(double)x1+
           (double)a2+x2+(double)a3+(double)x3+
           (double)a4+x4+(double)a5+(double)x5+
           (double)a6+x6+x7+x8;
}
