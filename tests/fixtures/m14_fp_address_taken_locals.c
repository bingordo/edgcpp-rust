/* M14 source-checked address-taken ordinary FP stack objects. */
double m14_addr_load(double x) {
    double local=x;
    double *p=&local;
    return *p;
}
double m14_addr_store(double x,double y) {
    double local=x;
    double *p=&local;
    *p=y;
    return local;  /* Must reload from actual stack, not stale SSA. */
}
double m14_addr_mixed(double x) {
    double local=x;
    double *p=&local;
    local=local+1.0;
    *p=*p+2.0;
    return local;
}
double m14_addr_retarget(double x,double y) {
    double a=x,b=y;
    double *p=&a;
    p=&b;
    *p=*p+2.0;
    return a+b;
}
double m14_addr_if(double x) {
    double a=x,b=x+1.0;
    double *p=&a;
    if(x>0.0) p=&b;
    *p=*p+2.0;
    return a+b;
}
double m14_addr_loop(double x) {
    double local=x, i=0.0;
    double *p=&local;
    while(i<3.0) {
        *p=*p+1.0;
        i=i+1.0;
    }
    return local;
}
float m14_addr_float(float x) {
    float local=x;
    float *p=&local;
    *p=-0.0f;
    return local;
}
double m14_addr_deref_direct(double x) {
    double local=x;
    *(&local)=local+1.0;
    return *(&local);
}
