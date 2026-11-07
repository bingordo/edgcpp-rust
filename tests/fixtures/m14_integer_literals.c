/* M14: source-checked 32/64-bit C integer literal -> FP conversions and
 * native INTEGER/SSE calls. A 64-bit unsigned literal must retain its bits. */
extern double m14_literal_external64(unsigned long long, double);
extern double m14_literal_external32(unsigned int, double);
double m14_literal_signed32(void) { return (double)2147483647; }
double m14_literal_unsigned32(void) { return (double)4294967295U; }
double m14_literal_signed64(void) { return (double)9223372036854775807LL; }
double m14_literal_unsigned64(void) { return (double)18446744073709551615ULL; }
float m14_literal_float32(void) { return (float)4294967295U; }
float m14_literal_float64(void) { return (float)18446744073709551615ULL; }
double m14_literal_mix(double x) { return x + (double)4294967295U; }
double m14_literal_nested(double x) {
    return x + (double)18446744073709551615ULL;
}
double m14_literal_external_call(double x) {
    return m14_literal_external64(18446744073709551615ULL, x)
        + m14_literal_external32(4294967295U, x);
}
