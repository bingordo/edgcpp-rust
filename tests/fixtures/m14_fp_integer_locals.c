/* Exact I32/U32/I64/U64 C automatic SSA locals in FP expressions/calls. */
extern double m14_integer_local_external(int,double);
double m14_il_i32(int n,double x) {
    int copy=n;
    return x+(double)copy;
}
double m14_il_u32(unsigned int n,double x) {
    unsigned int copy=n;
    return x+(double)copy;
}
double m14_il_i64(long long n,double x) {
    long long copy=n;
    return x+(double)copy;
}
double m14_il_u64(unsigned long long n,double x) {
    unsigned long long copy=n;
    return x+(double)copy;
}
double m14_il_const_signed(double x) {
    int value=-17;
    return x+(double)value;
}
double m14_il_const_unsigned(double x) {
    unsigned int value=4294967295U;
    return x+(double)value;
}
double m14_il_const_u64(double x) {
    unsigned long long value=18446744073709551615ULL;
    return x+(double)value;
}
double m14_il_reassign(int n,double x) {
    int copy=n;
    copy=n;
    return x+(double)copy;
}
double m14_il_if(int n,double x) {
    int copy=3;
    if(x>0.0) copy=n;
    return (double)copy;
}
double m14_il_call(int n,double x) {
    int copy=n;
    return m14_integer_local_external(copy,x);
}
double m14_il_index(double *p,int i) {
    int copy=i;
    return p[copy];
}
double m14_il_index_store(double *p,int i,double v) {
    int copy=i;
    p[copy]=v;
    return p[copy];
}
