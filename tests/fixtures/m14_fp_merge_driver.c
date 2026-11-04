#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_join_absdiff(double,double);
double m14_join_ordered(double,double);
double m14_join_sequential(double,double,double);
float m14_join_f32(float,float);
double m14_join_existing(double,double);
double m14_join_nan(double,double);
double m14_if_no_else(double,double);
double m14_if_no_else_chain(double,double,double);
float m14_if_no_else_f32(float,float);
double m14_if_empty_else(double,double);
double m14_if_empty_then(double,double);
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
        double expected_x=a,expected_y=b;
        if(a!=b) {
            expected_x=a+b;
            expected_y=expected_x*2.0;
        } else {
            expected_y=a-b;
            expected_x=expected_y+1.0;
        }
        eqd(m14_join_ordered(a,b),expected_x-expected_y,
            "ordered sequential arm updates");
        const double first=(a>b) ? a-b : b-a;
        eqd(m14_join_sequential(a,b,1.25),
            (first>1.25 ? first-1.25 : first+1.25),
            "sequential merge");
        const float x=(float)a,y=(float)b;
        eqf(m14_join_f32(x,y),x>y ? x : y,"f32 phi");
        double existing_x=a,existing_y=b;
        if(a>=b) existing_x=a+1.0;
        else existing_y=b+2.0;
        eqd(m14_join_existing(a,b),existing_x+existing_y,
            "one sided update of initialized locals");
        eqd(m14_join_nan(a,b),a!=b ? 10.0 : 20.0,"NaN unordered");
        double expected_no_else=b;
        if (a>b) expected_no_else=a+2.0;
        eqd(m14_if_no_else(a,b),expected_no_else,"no else live-in join");
        double chain_x=a,chain_y=b;
        if(a>b) {
            chain_x=chain_x+b;
            chain_y=chain_x*2.0;
        }
        if(chain_x<1.25) chain_x=chain_x-1.25;
        eqd(m14_if_no_else_chain(a,b,1.25),chain_x+chain_y,
            "sequential no-else joins");
        float expected_f32=y;
        if(x!=y) expected_f32=x;
        eqf(m14_if_no_else_f32(x,y),expected_f32,"f32 no-else merge");
        double empty_else=b;
        if(a<b) empty_else=a+b;
        eqd(m14_if_empty_else(a,b),empty_else,"explicit empty else");
        double empty_then=a;
        if(!(a>b)) empty_then=b+1.0;
        eqd(m14_if_empty_then(a,b),empty_then,"explicit empty then");
    }
    return 0;
}
