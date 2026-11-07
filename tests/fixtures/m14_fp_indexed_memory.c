/* Native C indexed ordinary F32/F64 memory, no guessed C casts. */
double m14_index_load(double *p,int i) {return p[i];}
double m14_index_reverse(double *p,int i) {return i[p];}
double m14_index_negative(double *p,int i) {return p[i];}
double m14_index_store(double *p,unsigned int i,double x) {
    p[i]=x;
    return p[i];
}
double m14_index_local(double *p,int i,double x) {
    double *copy=p;
    copy[i]=x;
    return p[i];
}
double m14_index_const(double *p) {return p[2];}
double m14_index_i64(double *p,long long i) {return p[i];}
double m14_index_u64(double *p,unsigned long long i) {return p[i];}
float m14_index_f32(float *p,unsigned long long i,float x) {
    p[i]=x;
    return i[p];
}
/* Addressed F64 local is allowed at index zero only; no bogus SSA copy. */
double m14_index_addressed(double x,double y) {
    double local=x;
    double *p=&local;
    p[0]=y;
    return local;
}
