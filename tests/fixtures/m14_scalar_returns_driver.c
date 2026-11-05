#include <math.h>
#include <stdint.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
int m14_fp_to_i32(float);
unsigned int m14_fp_to_u32(double);
long m14_fp_to_i64(double);
unsigned long m14_fp_to_u64(double);
int m14_mixed_ret(int,double);
int m14_cond_ret(double,double);
long m14_loop_ret(double,double);
unsigned long m14_large_u64(double);
#define CHECK(A,B) do { if ((A)!=(B)) { \
    fprintf(stderr,"M14 integer return ABI mismatch at line %d\n",__LINE__); \
    exit(1); } } while(0)
int main(void) {
    const double x[]={-155.75,-4.5,-0.0,0.0,0.75,12.5,1800.25};
    for(size_t i=0;i<sizeof(x)/sizeof(x[0]);i++) {
        const double v=x[i];
        CHECK(m14_fp_to_i32((float)v),(int)(float)v);
        CHECK(m14_fp_to_i64(v),(long)v);
        CHECK(m14_mixed_ret(-7,v),(int)((double)-7+v));
        CHECK(m14_cond_ret(v,4.75),v<4.75?(int)v:(int)4.75);
    }
    const double u[]={0.0,0.25,1.75,4294967295.0,4294967296.0,1.3e19};
    for(size_t i=0;i<sizeof(u)/sizeof(u[0]);i++) {
        double v=u[i];
        if(v<4294967296.0) CHECK(m14_fp_to_u32(v),(unsigned int)v);
        CHECK(m14_fp_to_u64(v),(unsigned long)v);
    }
    CHECK(m14_large_u64(1.3e19),(unsigned long)1.3e19);
    CHECK(m14_loop_ret(-2.0,3.0),(long)3);
    CHECK(m14_loop_ret(5.0,3.0),(long)5);
    return 0;
}
