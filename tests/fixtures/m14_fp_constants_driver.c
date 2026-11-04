#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
float m14_c_f32_round(void);
double m14_c_f64_round(void);
float m14_c_f32_subnormal(void);
double m14_c_f64_subnormal(void);
float m14_c_f32_max(void);
double m14_c_f64_max(void);
float m14_c_f32_negzero(void);
double m14_c_f64_negzero(void);
float m14_c_f32_plus(float);
double m14_c_f64_plus(double);
float m14_c_f32_local(float);
double m14_c_f64_local(double);
static uint32_t bits32(float v) { uint32_t b; memcpy(&b,&v,sizeof(b));return b; }
static uint64_t bits64(double v) { uint64_t b;memcpy(&b,&v,sizeof(b));return b; }
static void check(int ok,const char *case_name) {
    if(!ok) {fprintf(stderr,"M14 constant mismatch: %s\n",case_name);exit(1);}
}
int main(void) {
    check(bits32(m14_c_f32_round()) == UINT32_C(0x3f800001),"f32 last-bit round");
    check(bits64(m14_c_f64_round()) == UINT64_C(0x3ff0000000000001),"f64 last-bit round");
    check(bits32(m14_c_f32_subnormal()) == UINT32_C(0x00000001),"f32 min subnormal");
    check(bits64(m14_c_f64_subnormal()) == UINT64_C(0x0000000000000001),"f64 min subnormal");
    check(bits32(m14_c_f32_max()) == UINT32_C(0x7f7fffff),"f32 max finite");
    check(bits64(m14_c_f64_max()) == UINT64_C(0x7fefffffffffffff),"f64 max finite");
    check(bits32(m14_c_f32_negzero()) == UINT32_C(0x80000000),"f32 negative zero");
    check(bits64(m14_c_f64_negzero()) == UINT64_C(0x8000000000000000),"f64 negative zero");
    check(bits32(m14_c_f32_plus(1.25f)) == bits32(5.75f),"f32 mixed parameter + literal");
    check(bits64(m14_c_f64_plus(1.25)) == bits64(7.25),"f64 mixed parameter + literal");
    check(bits32(m14_c_f32_local(2.0f)) == bits32(9.0f),"f32 constant local");
    check(bits64(m14_c_f64_local(1.5)) == bits64(4.5),"f64 constant local");
    return 0;
}
