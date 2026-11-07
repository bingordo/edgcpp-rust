/* Source-verified IEEE FP .data and .bss native ELF declarations. */
float m14_init_global_f32 = 0x1.000002p+0f;
double m14_init_global_f64 = -0.0;
static double m14_init_static_f64 = 0x1.4p+3;
/* This exported global is deliberately never referenced in this TU. */
double m14_init_unreferenced_f64 = 0x1.8p+7;
float m14_init_unreferenced_f32 = -0.0f;
double m14_init_zero_bss;
float m14_init_read_f32(void) { return m14_init_global_f32; }
double m14_init_read_f64(void) { return m14_init_global_f64; }
double m14_init_read_static(void) { return m14_init_static_f64; }
double m14_init_store(double x) {
    m14_init_global_f64=x;
    return m14_init_global_f64;
}
double m14_init_sum(double x) {
    return m14_init_zero_bss + m14_init_static_f64 + x;
}
