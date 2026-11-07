#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern float m14_init_global_f32;
extern double m14_init_global_f64;
extern double m14_init_unreferenced_f64;
extern float m14_init_unreferenced_f32;
extern double m14_init_zero_bss;
float m14_init_read_f32(void);
double m14_init_read_f64(void);
double m14_init_read_static(void);
double m14_init_store(double);
double m14_init_sum(double);
static uint32_t bits32(float v) { uint32_t n; memcpy(&n,&v,sizeof n); return n; }
static uint64_t bits64(double v) { uint64_t n; memcpy(&n,&v,sizeof n); return n; }
static void check(int ok,const char *reason) {
    if(!ok){fprintf(stderr,"M14 FP data image failure: %s\n",reason); exit(1);}
}
int main(void) {
    check(bits32(m14_init_global_f32)==UINT32_C(0x3f800001),"f32 .data");
    check(bits32(m14_init_read_f32())==UINT32_C(0x3f800001),"f32 read");
    check(bits64(m14_init_global_f64)==UINT64_C(0x8000000000000000),"f64 -zero .data");
    check(bits64(m14_init_read_f64())==UINT64_C(0x8000000000000000),"f64 -zero read");
    check(bits64(m14_init_read_static())==bits64(10.0),"internal static .data");
    check(bits64(m14_init_unreferenced_f64)==bits64(192.0),"unreferenced exported f64");
    check(bits32(m14_init_unreferenced_f32)==UINT32_C(0x80000000),"unreferenced f32 -zero");
    check(bits64(m14_init_zero_bss)==0,".bss zero");
    check(bits64(m14_init_sum(1.5))==bits64(11.5),"global mixed .data/.bss");
    m14_init_global_f64=3.5;
    check(bits64(m14_init_read_f64())==bits64(3.5),"GCC-side global write");
    check(bits64(m14_init_store(-0.0))==UINT64_C(0x8000000000000000),"generated -zero store");
    check(bits64(m14_init_global_f64)==UINT64_C(0x8000000000000000),"C-side observe store");
    return 0;
}
