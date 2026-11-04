#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_join_absdiff(double,double);
double m14_join_ordered(double,double);
double m14_join_sequential(double,double,double);
float m14_join_f32(float,float);
double m14_join_existing(double,double);
double m14_join_nan(double,double);
static void check(int yes,const char *where) {
    if (!yes) {fprintf(stderr,"M14 FP SSA merge mismatch: %s\n",where);exit(1);}
}
static void eqd(double actual,double expected,const char *name) {
    check(isnan(expected) ? isnan(actual) : actual==expected,name);
}
static void eqf(float actual,float expected,const char *name) {
    check(isnan(expected) ? isnan(actual) : actual==expected,name);
}
int main(void) {
    const double pairs[][2]={
        {1.25,3.0},{3.0,1.25},{2.0,2.0},{-0.0,+0.0},
        {NAN,2.0},{2.0,NAN},{NAN,NAN},
        {INFINITY,2.0},{-INFINITY,INFINITY},
    };
    for(size_t i=0;i<sizeof(pairs)/sizeof(pairs[0]);++i) {
        const double a=pairs[i][0],b=pairs[i][1];
        eqd(m14_join_absdiff(a,b),(a<b ? b-a : a-b),"absdiff");
        const double ordered=(a!=b) ? -(a+b) : 1.0;
        eqd(m14_join_ordered(a,b),ordered,"ordered sequential arm updates");
        const double first=(a>b) ? a-b : b-a;
        eqd(m14_join_sequential(a,b,1.25),
            (first>1.25 ? first-1.25 : first+1.25),
            "sequential merge");
        const float x=(float)a,y=(float)b;
        eqf(m14_join_f32(x,y),x>y ? x : y,"f32 phi");
        eqd(m14_join_existing(a,b),
            (a>=b ? a+1.0+b : a+b+2.0),
            "one sided update of initialized locals");
        eqd(m14_join_nan(a,b),a!=b ? 10.0 : 20.0,"NaN unordered");
    }
    return 0;
}
