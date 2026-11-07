#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
double m14_il_i32(int,double);
double m14_il_u32(unsigned int,double);
double m14_il_i64(long long,double);
double m14_il_u64(unsigned long long,double);
double m14_il_const_signed(double);
double m14_il_const_unsigned(double);
double m14_il_const_u64(double);
double m14_il_reassign(int,double);
double m14_il_if(int,double);
double m14_il_call(int,double);
double m14_il_index(double*,int);
double m14_il_index_store(double*,int,double);
double m14_integer_local_external(int n,double x) {return x+(double)n;}
static uint64_t b64(double x){uint64_t b;memcpy(&b,&x,sizeof b);return b;}
static void ck(int ok,const char *name){
    if(!ok){fprintf(stderr,"M14 INTEGER SSA mismatch: %s\n",name);exit(1);}
}
int main(void){
    double a[]={1.0,2.0,3.0,4.0};
    ck(b64(m14_il_i32(-15,1.5))==b64(-13.5),"signed I32");
    ck(b64(m14_il_u32(UINT32_MAX,1.5))==
        b64(1.5+(double)UINT32_MAX),"unsigned I32");
    ck(b64(m14_il_i64(INT64_MIN,2.0))==
        b64(2.0+(double)INT64_MIN),"signed I64");
    ck(b64(m14_il_u64(UINT64_MAX,2.0))==
        b64(2.0+(double)UINT64_MAX),"unsigned I64");
    ck(b64(m14_il_const_signed(1.0))==b64(-16.0),"I32 source constant");
    ck(b64(m14_il_const_unsigned(2.0))==
        b64(2.0+(double)UINT32_MAX),"U32 source constant");
    ck(b64(m14_il_const_u64(1.0))==
        b64(1.0+(double)UINT64_MAX),"U64 source constant");
    ck(b64(m14_il_reassign(9,1.5))==b64(10.5),"integer reassignment");
    ck(b64(m14_il_if(8,1.0))==b64(8.0),"branch assignment");
    ck(b64(m14_il_if(8,-1.0))==b64(3.0),"branch inherited constant");
    ck(b64(m14_il_call(-11,1.5))==b64(-9.5),"typed cross-object integer local");
    ck(b64(m14_il_index(a,2))==b64(3.0),"INTEGER SSA local indexed load");
    ck(b64(m14_il_index_store(a,1,-0.0))==
        UINT64_C(0x8000000000000000),"INTEGER SSA local indexed store");
    ck(b64(a[1])==UINT64_C(0x8000000000000000),"external memory observer");
    return 0;
}
