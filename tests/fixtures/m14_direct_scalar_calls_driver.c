#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_call_forward(int,double,unsigned long,float);
double m14_call_nested(int,double,unsigned long,float);
float m14_call_f32_wrap(float,float);
double m14_call_loop(int,double);
int m14_call_to_int(double);
double m14_call_many_wrap(int,double,unsigned int,float,long,double,
    unsigned long,float,int,double,unsigned int,float,long,double,double,double);
static void eq(double actual,double expected,const char *name) {
    if(!(isnan(expected)?isnan(actual):actual==expected)) {
        fprintf(stderr,"M14 direct scalar ABI mismatch %s: %.17g vs %.17g\n",
                name,actual,expected);
        exit(1);
    }
}
int main(void) {
    const double values[]={-3.0,-0.0,0.0,2.25,5.75,NAN};
    for(size_t i=0;i<sizeof(values)/sizeof(values[0]);i++) {
        const double x=values[i];
        const double one=(double)-7+x+(double)13ul+(double)1.5f;
        const double two=(double)-7+one+(double)13ul+(double)1.5f;
        eq(m14_call_forward(-7,x,13ul,1.5f),one,"direct forward");
        eq(m14_call_nested(-7,x,13ul,1.5f),two,"nested direct calls");
        eq((double)m14_call_f32_wrap((float)x,1.25f),
           (double)((float)x+1.25f),"F32 returned directly");
        eq(m14_call_many_wrap(-7,x,2u,1.5f,-9l,2.0,13ul,3.0f,
           4,5.0,6u,7.0f,8l,9.0,10.0,11.0),
           (double)-7+x+(double)2u+(double)1.5f+
           (double)-9l+2.0+(double)13ul+(double)3.0f+
           (double)4+5.0+(double)6u+(double)7.0f+
           (double)8l+9.0+10.0+11.0,"mixed direct 7 GPR/9 XMM");
    }
    eq(m14_call_loop(4,1.5),4.5,"call inside loop SSA backedge");
    eq(m14_call_loop(1,3.5),3.5,"zero iteration");
    if(m14_call_to_int(2.75)!=(int)3.75) exit(1);
    return 0;
}
