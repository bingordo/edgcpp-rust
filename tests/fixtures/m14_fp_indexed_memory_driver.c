#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
double m14_index_load(double*,int);
double m14_index_reverse(double*,int);
double m14_index_negative(double*,int);
double m14_index_store(double*,unsigned int,double);
double m14_index_local(double*,int,double);
double m14_index_const(double*);
double m14_index_i64(double*,long long);
double m14_index_u64(double*,unsigned long long);
float m14_index_f32(float*,unsigned long long,float);
double m14_index_addressed(double,double);
static uint64_t bits64(double x){uint64_t bits;memcpy(&bits,&x,sizeof bits);return bits;}
static uint32_t bits32(float x){uint32_t bits;memcpy(&bits,&x,sizeof bits);return bits;}
static void ck(int okay,const char *where) {
    if(!okay){fprintf(stderr,"M14 indexed FP mismatch: %s\n",where);exit(1);}
}
int main(void) {
    double a[]={1.0,2.0,3.0,4.0,5.0}, b[]={-2.0,-4.0,8.0};
    float c[]={1.5f,2.5f,3.5f,4.5f};
    ck(bits64(m14_index_load(a,2))==bits64(3.0),"I32 signed load");
    ck(bits64(m14_index_reverse(a,3))==bits64(4.0),"i[p] operand order");
    ck(bits64(m14_index_negative(a+3,-2))==bits64(2.0),"negative index sign extension");
    ck(bits64(m14_index_const(a))==bits64(3.0),"exact integer literal offset");
    ck(bits64(m14_index_i64(a+4,-3))==bits64(2.0),"signed I64 index");
    ck(bits64(m14_index_u64(a,4ULL))==bits64(5.0),"unsigned I64 index");
    ck(bits64(m14_index_store(a,1U,-0.0))==UINT64_C(0x8000000000000000),"store signed zero");
    ck(bits64(a[1])==UINT64_C(0x8000000000000000),"caller observes indexed store");
    ck(bits64(m14_index_local(b,2,-0.0))==UINT64_C(0x8000000000000000),"pointer SSA local alias");
    ck(bits64(b[2])==UINT64_C(0x8000000000000000),"local pointer writes backing array");
    ck(bits32(m14_index_f32(c,3ULL,-0.0f))==UINT32_C(0x80000000),"F32 unsigned I64 store");
    ck(bits32(c[3])==UINT32_C(0x80000000),"caller observes F32 store");
    ck(bits64(m14_index_addressed(5.0,-0.0))==UINT64_C(0x8000000000000000),"addressed stack local index zero");
    return 0;
}
