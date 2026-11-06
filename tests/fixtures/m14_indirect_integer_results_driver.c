#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int m14_indirect_i32(int(*)(int,double),int,double);
unsigned int m14_indirect_u32(unsigned int(*)(unsigned int,float),
    unsigned int,float);
long m14_indirect_i64(long(*)(long,double),long,double);
unsigned long m14_indirect_u64(unsigned long(*)(unsigned long,double),
    unsigned long,double);
int m14_indirect_branch(int(*)(double),double,double);
long m14_indirect_loop(long(*)(double),double,double);
long m14_indirect_spilled(int,double,unsigned int,float,long,double,
    unsigned long,float,int,double,unsigned int,float,long,double,
    double,double,long(*)(int,double,unsigned int,float,long,double,
        unsigned long,float,int,double,unsigned int,float,long,double,
        double,double));
static int cb_i32(int a,double x) {return a+(int)x;}
static int cb_i32_other(int a,double x) {return a-(int)x;}
static unsigned int cb_u32(unsigned int a,float x) {
    return a+(unsigned int)x;
}
static long cb_i64(long a,double x) {return a-(long)x;}
static unsigned long cb_u64(unsigned long a,double x) {
    return a+(unsigned long)x;
}
static int cb_branch(double x) {return (int)x+7;}
static long cb_loop(double x) {return (long)x+40L;}
#define CHECK(GOT,WANT) do { \
    if ((GOT)!=(WANT)) { \
        fprintf(stderr,"indirect INTEGER return ABI mismatch at line %d\n",__LINE__); \
        exit(1); \
    } \
} while(0)
int main(void) {
    const double values[]={-123.5,-2.75,-0.0,0.0,2.5,99.125};
    for(size_t i=0;i<sizeof(values)/sizeof(values[0]);i++) {
        double x=values[i];
        CHECK(m14_indirect_i32(cb_i32,1234,x),cb_i32(1234,x));
        CHECK(m14_indirect_i32(cb_i32_other,1234,x),
              cb_i32_other(1234,x));
        CHECK(m14_indirect_i64(cb_i64,-10000000000L,x),
              cb_i64(-10000000000L,x));
        CHECK(m14_indirect_branch(cb_branch,x,5.5),
              cb_branch(x<5.5?x:5.5));
    }
    const float uvals[]={0.0f,1.5f,999.5f};
    for(size_t i=0;i<sizeof(uvals)/sizeof(uvals[0]);i++) {
        CHECK(m14_indirect_u32(cb_u32,4000000000U,uvals[i]),
              cb_u32(4000000000U,uvals[i]));
    }
    const unsigned long big=13000000000000000000UL;
    CHECK(m14_indirect_u64(cb_u64,big,19.0),cb_u64(big,19.0));
    CHECK(m14_indirect_u64(cb_u64,big,0.0),cb_u64(big,0.0));
    CHECK(m14_indirect_loop(cb_loop,-2.0,3.0),cb_loop(3.0));
    CHECK(m14_indirect_loop(cb_loop,4.0,3.0),cb_loop(4.0));
    CHECK(m14_indirect_spilled(-1,1.0,2U,3.0f,4L,5.0,6UL,7.0f,
           8,9.0,10U,11.0f,12L,13.0,14.0,15.0,cb_i64),
          cb_i64(12L,15.0));
    return 0;
}
