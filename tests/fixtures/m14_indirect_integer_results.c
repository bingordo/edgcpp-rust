/* M14 native INTEGER return class from an exact EDG callable formal.
 * No FP-to-integer conversions are implied at the call boundary.
 */
int m14_indirect_i32(int (*fn)(int,double),int a,double x) {
    return fn(a,x);
}
unsigned int m14_indirect_u32(unsigned int (*fn)(unsigned int,float),
                             unsigned int a,float x) {
    return (*fn)(a,x);
}
long m14_indirect_i64(long (*fn)(long,double),long a,double x) {
    return fn(a,x);
}
unsigned long m14_indirect_u64(unsigned long (*fn)(unsigned long,double),
                               unsigned long a,double x) {
    return (*fn)(a,x);
}
int m14_indirect_branch(int (*fn)(double),double x,double y) {
    if(x<y) return fn(x);
    return fn(y);
}
long m14_indirect_loop(long (*fn)(double),double x,double bound) {
    double v=x;
    while(v<bound) v=v+1.0;
    return fn(v);
}
long m14_indirect_spilled(
    int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,
    int a4,double x4,unsigned int a5,float x5,
    long a6,double x6,double x7,double x8,
    long (*fn)(int,double,unsigned int,float,long,double,unsigned long,float,
               int,double,unsigned int,float,long,double,double,double)) {
    return fn(a0,x0,a1,x1,a2,x2,a3,x3,a4,x4,a5,x5,a6,x6,x7,x8);
}
