/* M14 native EDG IEEE binary32/binary64 constant bit-preserving imports. */
float m14_c_f32_round(void) { return 0x1.000002p+0f; }
double m14_c_f64_round(void) { return 0x1.0000000000001p+0; }
float m14_c_f32_subnormal(void) { return 0x1p-149f; }
double m14_c_f64_subnormal(void) { return 0x1p-1074; }
float m14_c_f32_max(void) { return 0x1.fffffep+127f; }
double m14_c_f64_max(void) { return 0x1.fffffffffffffp+1023; }
float m14_c_f32_negzero(void) { return -0.0f; }
double m14_c_f64_negzero(void) { return -0.0; }
float m14_c_f32_plus(float x) { return x + 0x1.2p+2f; }
double m14_c_f64_plus(double x) { return x + 0x1.8p+2; }
float m14_c_f32_local(float x) {
    float temp = 0x1.2p+2f;
    temp = temp * x;
    return temp;
}
double m14_c_f64_local(double x) {
    double temp = 0x1.8p+2;
    temp = temp - x;
    return temp;
}
