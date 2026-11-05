#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <limits.h>
double m14_mixed_i32(int,double,unsigned int,float);
float m14_mixed_f32(int,float);
double m14_mixed_i64(long,double,unsigned long,float);
double m14_mixed_many(int,double,unsigned int,float,long,double,
    unsigned long,float,int,double,unsigned int,float,long,double,double,double);
double m14_mixed_phi(int,double,double);
double m14_mixed_loop(int,double);
static void check(double got,double want,const char *name) {
    if(!(isnan(want)?isnan(got):got==want)) {
        fprintf(stderr,"mixed scalar ABI mismatch %s: %.17g vs %.17g\n",name,got,want);
        exit(1);
    }
}
int main(void) {
    const double values[]={-2.0,-0.0,0.0,1.25,3.5,NAN,INFINITY,-INFINITY};
    for(size_t i=0;i<sizeof(values)/sizeof(values[0]);i++) {
        double x=values[i];
        check(m14_mixed_i32(-7,x,13u,2.5f),
              (double)-7+x+(double)13u+(double)2.5f,"i32 mixed");
        check((double)m14_mixed_f32(7,(float)x),
              (double)((float)7+(float)x),"F32 result");
        check(m14_mixed_i64(LONG_MIN,x,ULONG_MAX,1.5f),
              (double)LONG_MIN+x+(double)ULONG_MAX+(double)1.5f,
              "i64 unsigned/signed conversion");
        check(m14_mixed_many(
              -7,x,2u,1.5f,-9L,2.0,13UL,3.0f,
              4,5.0,6u,7.0f,8L,9.0,10.0,11.0),
              (double)-7+x+(double)2u+(double)1.5f+
              (double)-9L+2.0+(double)13UL+(double)3.0f+
              (double)4+5.0+(double)6u+(double)7.0f+
              (double)8L+9.0+10.0+11.0,
              "7-GPR/9-XMM independent exhaustion");
        check(m14_mixed_phi(-3,x,2.0),
              x<2.0? (double)-3+2.0:(double)-3+x,"SSA merge after mixed cast");
    }
    check(m14_mixed_loop(4,1.25),4.25,"mixed loop comparison cast");
    check(m14_mixed_loop(-2,3.25),3.25,"mixed loop zero iteration");
    return 0;
}
