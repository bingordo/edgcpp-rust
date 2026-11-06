/* Compiled by GCC only: generated EDG ELF object must link to these. */
double m14_ext_adjust(double x,double y) {return x-y;}
float m14_ext_f32(float x,float y) {return x+y;}
double m14_ext_mixed(int a,double x,unsigned long b,float y) {
    return (double)a+x+(double)b+(double)y;
}
double m14_ext_step(double x) {return x+1.0;}
double m14_ext_many(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,int a4,double x4,
    unsigned int a5,float x5,long a6,double x6,double x7,double x8) {
    return (double)a0+x0+(double)a1+(double)x1+
           (double)a2+x2+(double)a3+(double)x3+
           (double)a4+x4+(double)a5+(double)x5+
           (double)a6+x6+x7+x8;
}
