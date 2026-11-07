/* M14 source-checked ordinary memory via F32/F64 pointer formals. */
double m14_fp_ptr_read(double *p) {
    return *p;
}
float m14_fp_ptr_f32(float *p) {
    return *p;
}
double m14_fp_ptr_store(double *p,double x) {
    *p=x;
    return *p;
}
float m14_fp_ptr_store_f32(float *p,float x) {
    *p=x;
    return *p;
}
double m14_fp_ptr_alias(double *p,double *q,double x) {
    *p=x;
    return *q;
}
double m14_fp_ptr_accum(double *p,double x) {
    *p=*p+x;
    return *p;
}
double m14_fp_ptr_branch(double *p,double x,double threshold) {
    if(x>threshold) *p=x;
    return *p;
}
double m14_fp_ptr_loop(double *p,double x,double bound) {
    double value=x;
    while(value<bound) {
        *p=value;
        value=value+1.0;
    }
    return *p;
}
double m14_fp_ptr_eight_sse(double a0,double a1,double a2,
    double a3,double a4,double a5,double a6,double a7,
    double *p,double v) {
    *p=v;
    return *p+a0+a1+a2+a3+a4+a5+a6+a7;
}
