#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_cmp_eq(double,double);
double m14_cmp_ne(double,double);
double m14_cmp_lt(double,double);
double m14_cmp_le(double,double);
double m14_cmp_gt(double,double);
double m14_cmp_ge(double,double);
float m14_cmp_f32(float,float);
double m14_cmp_local(double,double);
static void check(int yes,const char*msg) {
  if(!yes){fprintf(stderr,"M14 FP compare mismatch: %s\n",msg);exit(1);}
}
int main(void) {
    const double x[][2]={
        {1.0,2.0},{2.0,1.0},{2.0,2.0},{-0.0,+0.0},
        {NAN,1.0},{1.0,NAN},{NAN,NAN},{INFINITY,INFINITY},
        {-INFINITY,INFINITY},{INFINITY,-INFINITY},
    };
    const size_t count=sizeof(x)/sizeof(x[0]);
    for(size_t i=0;i<count;i++){
        const double a=x[i][0],b=x[i][1];
        check(m14_cmp_eq(a,b)==((a==b)?11.0:22.0),"eq/NaN");
        check(m14_cmp_ne(a,b)==((a!=b)?11.0:22.0),"ne/unordered");
        check(m14_cmp_lt(a,b)==((a< b)?11.0:22.0),"lt/NaN");
        check(m14_cmp_le(a,b)==((a<=b)?11.0:22.0),"le/NaN");
        check(m14_cmp_gt(a,b)==((a> b)?11.0:22.0),"gt/NaN");
        check(m14_cmp_ge(a,b)==((a>=b)?11.0:22.0),"ge/NaN");
        const float f=(float)a,g=(float)b;
        check(m14_cmp_f32(f,g)==((f!=g)?11.0f:22.0f),"f32 ne");
    }
    check(m14_cmp_local(1.25,2.0)==2.25,"local conditional yes");
    check(m14_cmp_local(-3.0,2.0)==2.0,"local conditional no");
    return 0;
}
