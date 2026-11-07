/* M14 source-checked nonescaping C float and double pointer locals. */
double m14_ptrlocal_load(double *p) {
    double *copy=p;
    return *copy;
}
double m14_ptrlocal_store(double *p,double x) {
    double *copy=p;
    *copy=x;
    return *p;
}
double m14_ptrlocal_reassign(double *p,double *q,double x) {
    double *copy=p;
    copy=q;
    *copy=x;
    return *copy;
}
double m14_ptrlocal_branch(double *p,double *q,double x) {
    double *copy;
    if (x>0.0) copy=p;
    else copy=q;
    *copy=x;
    return *p+*q;
}
double m14_ptrlocal_loop(double *p,double x) {
    double *copy=p;
    while (x>0.0) {
        *copy=*copy+1.0;
        x=x-1.0;
    }
    return *copy;
}
float m14_ptrlocal_f32(float *p,float x) {
    float *copy=p;
    *copy=x;
    return *copy;
}
