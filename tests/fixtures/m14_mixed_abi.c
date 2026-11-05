/* Source-verified 32/64-bit mixed INTEGER/SSE SysV argument ABI.
 * All integer conversions must originate in EDG's real C cast nodes. */
double m14_mixed_i32(int a,double b,unsigned int c,float d) {
    return (double)a+b+(double)c+(double)d;
}
float m14_mixed_f32(int a,float b) {
    return (float)a+b;
}
double m14_mixed_i64(long a,double b,unsigned long c,float d) {
    return (double)a+b+(double)c+(double)d;
}
double m14_mixed_many(
    int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,
    int a4,double x4,unsigned int a5,float x5,
    long a6,double x6,double x7,double x8) {
    return (double)a0+x0+(double)a1+(double)x1+
           (double)a2+x2+(double)a3+(double)x3+
           (double)a4+x4+(double)a5+(double)x5+
           (double)a6+x6+x7+x8;
}
double m14_mixed_phi(int a,double x,double y) {
    double v=x;
    if(x<y) v=(double)a+y;
    else v=(double)a+x;
    return v;
}
double m14_mixed_loop(int n,double x) {
    double y=x;
    while(y<(double)n) y=y+1.0;
    return y;
}
