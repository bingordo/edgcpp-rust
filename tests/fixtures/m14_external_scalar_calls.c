/* M14 checked external source declarations: no implementation in this TU. */
extern double m14_ext_adjust(double x,double y);
extern float m14_ext_f32(float x,float y);
extern double m14_ext_mixed(int a,double x,unsigned long b,float y);
extern double m14_ext_step(double x);
extern double m14_ext_many(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,int a4,double x4,
    unsigned int a5,float x5,long a6,double x6,double x7,double x8);
double m14_wrap_external_adjust(double x,double y) {
    return m14_ext_adjust(x,y);
}
double m14_wrap_external_nested(double x,double y) {
    return m14_ext_adjust(m14_ext_adjust(x,y),y);
}
float m14_wrap_external_float(float x,float y) {
    return m14_ext_f32(x,y);
}
double m14_wrap_external_mix(int a,double x,unsigned long b,float y) {
    return m14_ext_mixed(a,x,b,y);
}
double m14_wrap_external_many(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,int a4,double x4,
    unsigned int a5,float x5,long a6,double x6,double x7,double x8) {
    return m14_ext_many(a0,x0,a1,x1,a2,x2,a3,x3,a4,x4,a5,x5,a6,x6,x7,x8);
}
double m14_wrap_external_loop(double x,double bound) {
    double v=x;
    while(v<bound) v=m14_ext_step(v);
    return v;
}
int m14_wrap_external_int_return(double x) {
    return (int)m14_ext_step(x);
}
