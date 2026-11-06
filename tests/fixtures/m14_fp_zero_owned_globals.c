/* Native F32/F64 C tentative and internal definitions: actual zeroed storage. */
double m14_zero_d;
float m14_zero_f;
static double m14_zero_static_d;
double m14_zero_read_d(void) {return m14_zero_d;}
float m14_zero_read_f(void) {return m14_zero_f;}
double m14_zero_store_d(double x) {
    m14_zero_d=x;
    return m14_zero_d;
}
float m14_zero_store_f(float x) {
    m14_zero_f=x;
    return m14_zero_f;
}
double m14_zero_add_d(double x) {
    m14_zero_d=m14_zero_d+x;
    return m14_zero_d;
}
double m14_zero_branch(double x,double limit) {
    if(x>limit) m14_zero_d=x;
    return m14_zero_d;
}
double m14_zero_static(double x) {
    m14_zero_static_d=x;
    return m14_zero_static_d;
}
