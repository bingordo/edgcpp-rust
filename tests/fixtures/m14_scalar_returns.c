/* M14: scalar result ABI uses XMM for float and RAX for native C integer. */
int m14_fp_to_i32(float a) {return (int)a;}
unsigned int m14_fp_to_u32(double a) {return (unsigned int)a;}
long m14_fp_to_i64(double a) {return (long)a;}
unsigned long m14_fp_to_u64(double a) {return (unsigned long)a;}
int m14_mixed_ret(int a,double b) {return (int)((double)a+b);}
int m14_cond_ret(double a,double b) {
    if(a<b) return (int)a;
    return (int)b;
}
long m14_loop_ret(double a,double b) {
    double x=a;
    while(x<b) x=x+1.0;
    return (long)x;
}
unsigned long m14_large_u64(double a) {return (unsigned long)a;}
