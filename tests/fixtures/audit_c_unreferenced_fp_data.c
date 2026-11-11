/* These IEEE globals are never referenced in this translation unit.
 * The sole definition of the function uses only integer C semantics. */
float audit_c_fp_negative_zero = -0.0f;
double audit_c_fp_two_point_five = 2.5;
float audit_c_fp_tentative;
extern double audit_c_fp_extern_initialized = -3.5;

int audit_c_fp_integer_only(void) { return 11; }
