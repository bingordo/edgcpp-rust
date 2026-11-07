#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
double m14_literal_signed32(void);
double m14_literal_unsigned32(void);
double m14_literal_signed64(void);
double m14_literal_unsigned64(void);
float m14_literal_float32(void);
float m14_literal_float64(void);
double m14_literal_mix(double);
double m14_literal_nested(double);
double m14_literal_external_call(double);
static uint64_t bits64(double x) {
    uint64_t n;
    memcpy(&n, &x, sizeof n);
    return n;
}
static uint32_t bits32(float x) {
    uint32_t n;
    memcpy(&n, &x, sizeof n);
    return n;
}
static void check(int cond, const char *where) {
    if (!cond) {
        fprintf(stderr, "M14 integer literal mismatch: %s\n", where);
        exit(1);
    }
}
/* Independently compiled GCC call targets, linked against both object sets. */
double m14_literal_external64(unsigned long long v, double bias) {
    return (double)v + bias;
}
double m14_literal_external32(unsigned int v, double bias) {
    return (double)v + bias;
}
int main(void) {
    check(bits64(m14_literal_signed32()) == bits64((double)INT32_MAX), "i32");
    check(bits64(m14_literal_unsigned32()) == bits64((double)UINT32_MAX), "u32");
    check(bits64(m14_literal_signed64()) == bits64((double)INT64_MAX), "i64");
    check(bits64(m14_literal_unsigned64()) == bits64((double)UINT64_MAX), "u64");
    check(bits32(m14_literal_float32()) == bits32((float)UINT32_MAX), "u32/f32");
    check(bits32(m14_literal_float64()) == bits32((float)UINT64_MAX), "u64/f32");
    check(bits64(m14_literal_mix(0.5)) ==
          bits64(0.5+(double)UINT32_MAX), "mixed FP");
    check(bits64(m14_literal_nested(-2.0)) ==
          bits64(-2.0+(double)UINT64_MAX), "nested FP");
    check(bits64(m14_literal_external_call(4.0)) ==
          bits64(m14_literal_external64(UINT64_MAX,4.0)+
                 m14_literal_external32(UINT32_MAX,4.0)), "typed external");
    return 0;
}
