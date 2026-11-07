#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
extern unsigned int edg_native_sum(unsigned int,unsigned int);
extern unsigned int edg_native_mix(unsigned int,unsigned int,unsigned int);
extern unsigned int edg_native_neg(unsigned int);
extern int edg_native_sub(int,int);
extern long long edg_native_i64(long long,long long);
extern unsigned long long edg_native_u64(unsigned long long,unsigned long long);
extern unsigned int edg_native_parens(unsigned int);
extern unsigned int edg_native_const(void);
static void check(int ok,const char *name) {
    if(!ok) {fprintf(stderr,"direct EDG compiler-ir differential: %s\n",name);exit(1);}
}
int main(void){
    const uint32_t values[]={0,1,7,17,1024,UINT32_C(0x80000000),UINT32_MAX};
    for(unsigned i=0;i<sizeof values/sizeof values[0];++i) {
        uint32_t x=values[i];
        check(edg_native_parens(x)==x,"parameter roundtrip");
        check(edg_native_neg(x)==(uint32_t)(0-x),"unsigned unary negation");
        for(unsigned j=0;j<sizeof values/sizeof values[0];++j) {
            uint32_t y=values[j];
            check(edg_native_sum(x,y)==(uint32_t)(x+y),"I32 modular add");
            check(edg_native_mix(x,y,19)==(uint32_t)(x*y+19),"I32 modular expression");
        }
    }
    check(edg_native_sub(300,-17)==317,"signed I32 sub");
    check(edg_native_sub(-300,17)==-317,"signed I32 negative");
    check(edg_native_i64(1234567,76543)==(1234567LL*76543-76543),"I64 multiply/subtract");
    check(edg_native_u64(UINT64_MAX,3)==(UINT64_MAX*3+7ULL),"U64 modulo multiply/add");
    check(edg_native_const()==UINT32_C(3141592653),"source U32 literal");
    return 0;
}
