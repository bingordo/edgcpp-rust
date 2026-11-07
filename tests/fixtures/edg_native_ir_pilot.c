/* Direct EDG -> compiler-ir -> native x86 codegen, no Cranelift. */
unsigned int edg_native_sum(unsigned int x,unsigned int y) {
    return x+y;
}
unsigned int edg_native_mix(unsigned int x,unsigned int y,unsigned int z) {
    return x*y+z;
}
unsigned int edg_native_neg(unsigned int x) {return -x;}
int edg_native_sub(int x,int y) {return x-y;}
long long edg_native_i64(long long x,long long y) {return x*y-y;}
unsigned long long edg_native_u64(unsigned long long x,unsigned long long y) {
    return x*y+7ULL;
}
unsigned int edg_native_parens(unsigned int x) {return ((x));}
unsigned int edg_native_const(void) {return 3141592653U;}
