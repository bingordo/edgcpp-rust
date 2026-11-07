#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
double m14_ptrlocal_load(double *);
double m14_ptrlocal_store(double *,double);
double m14_ptrlocal_reassign(double *,double *,double);
double m14_ptrlocal_branch(double *,double *,double);
double m14_ptrlocal_loop(double *,double);
float m14_ptrlocal_f32(float *,float);
static uint64_t b64(double x){ uint64_t b; memcpy(&b,&x,sizeof b);return b;}
static uint32_t b32(float x){ uint32_t b; memcpy(&b,&x,sizeof b);return b;}
static void ck(int okay,const char *why) {
    if(!okay){fprintf(stderr,"FP pointer local mismatch: %s\n",why); exit(1);}
}
int main(void) {
    double a=3.5,b=8.0;
    float c=2.0f;
    ck(b64(m14_ptrlocal_load(&a))==b64(3.5),"load from local alias");
    ck(b64(m14_ptrlocal_store(&a,-0.0))==UINT64_C(0x8000000000000000),"store -zero");
    ck(b64(a)==UINT64_C(0x8000000000000000),"observer alias");
    ck(b64(m14_ptrlocal_reassign(&a,&b,5.0))==b64(5.0),"reassign pointer");
    ck(b64(b)==b64(5.0),"reassigned target only");
    a=3.0;b=4.0;
    ck(b64(m14_ptrlocal_branch(&a,&b,2.0))==b64(6.0),"if positive target");
    ck(b64(a)==b64(2.0) && b64(b)==b64(4.0),"if positive storage");
    ck(b64(m14_ptrlocal_branch(&a,&b,-1.0))==b64(1.0),"if negative target");
    ck(b64(b)==b64(-1.0),"if negative storage");
    a=7.0;
    ck(b64(m14_ptrlocal_branch(&a,&a,2.0))==b64(4.0),"aliased targets");
    ck(b64(a)==b64(2.0),"aliased storage");
    ck(b64(m14_ptrlocal_loop(&a,3.0))==b64(5.0),"loop pointer read/write");
    ck(b32(m14_ptrlocal_f32(&c,-0.0f))==UINT32_C(0x80000000),"f32 -zero");
    ck(b32(c)==UINT32_C(0x80000000),"f32 observable store");
    return 0;
}
