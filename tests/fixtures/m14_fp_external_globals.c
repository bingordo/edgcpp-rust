/* Source-verified ordinary extern float/double storage, not owned data. */
extern double m14_fp_shared_d;
extern float m14_fp_shared_f;
double m14_external_d_add(double x) {
    return m14_fp_shared_d+x;
}
float m14_external_f_add(float x) {
    return m14_fp_shared_f+x;
}
double m14_external_d_twice(double x) {
    return m14_fp_shared_d+m14_fp_shared_d+x;
}
double m14_external_d_cond(double x) {
    if(m14_fp_shared_d>x) return m14_fp_shared_d;
    return x;
}
double m14_external_d_loop(double x) {
    double value=x;
    while(value<m14_fp_shared_d) value=value+1.0;
    return value;
}
