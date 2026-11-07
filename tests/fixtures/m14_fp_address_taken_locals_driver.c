#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
double m14_addr_load(double);
double m14_addr_store(double,double);
double m14_addr_mixed(double);
double m14_addr_retarget(double,double);
double m14_addr_if(double);
double m14_addr_loop(double);
float m14_addr_float(float);
double m14_addr_deref_direct(double);
static uint64_t bits64(double x) { uint64_t b; memcpy(&b,&x,sizeof b);return b; }
static uint32_t bits32(float x) { uint32_t b; memcpy(&b,&x,sizeof b);return b; }
static void ck(int condition,const char *name) {
    if(!condition) { fprintf(stderr,"M14 addressed FP stack mismatch: %s\n",name);exit(1); }
}
int main(void) {
    ck(bits64(m14_addr_load(3.5))==bits64(3.5),"initial stack read");
    ck(bits64(m14_addr_store(1.0,-0.0))==UINT64_C(0x8000000000000000),"pointer store visible to direct local");
    ck(bits64(m14_addr_mixed(2.0))==bits64(5.0),"direct store followed by aliased store");
    ck(bits64(m14_addr_retarget(1.0,3.0))==bits64(6.0),"addressed two locals");
    ck(bits64(m14_addr_if(2.0))==bits64(7.0),"conditional pointer target");
    ck(bits64(m14_addr_if(-2.0))==bits64(-1.0),"else keeps target");
    ck(bits64(m14_addr_loop(4.0))==bits64(7.0),"loop address and memory");
    ck(bits32(m14_addr_float(17.0f))==UINT32_C(0x80000000),"F32 signed zero");
    ck(bits64(m14_addr_deref_direct(5.0))==bits64(6.0),"native direct address and dereference");
    return 0;
}
