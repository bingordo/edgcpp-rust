#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_ext_adjust(double,double);
float m14_ext_f32(float,float);
double m14_ext_mixed(int,double,unsigned long,float);
double m14_ext_step(double);
double m14_ext_many(int,double,unsigned int,float,long,double,unsigned long,float,
                   int,double,unsigned int,float,long,double,double,double);
double m14_wrap_external_adjust(double,double);
double m14_wrap_external_nested(double,double);
float m14_wrap_external_float(float,float);
double m14_wrap_external_mix(int,double,unsigned long,float);
double m14_wrap_external_many(int,double,unsigned int,float,long,double,unsigned long,float,
                   int,double,unsigned int,float,long,double,double,double);
double m14_wrap_external_loop(double,double);
int m14_wrap_external_int_return(double);
static void check(double a,double b,const char *name) {
    if(!(isnan(b)?isnan(a):a==b)) {
        fprintf(stderr,"M14 external scalar mismatch %s %.17g != %.17g\n",name,a,b);
        exit(1);
    }
}
int main(void) {
    const double values[]={-3.0,-0.0,0.0,1.25,2.5,NAN,INFINITY,-INFINITY};
    for(size_t i=0;i<sizeof(values)/sizeof(values[0]);i++) {
        double x=values[i];
        check(m14_wrap_external_adjust(x,1.5),m14_ext_adjust(x,1.5),"double");
        check(m14_wrap_external_nested(x,1.5),
            m14_ext_adjust(m14_ext_adjust(x,1.5),1.5),"nested");
        check((double)m14_wrap_external_float((float)x,1.5f),
            (double)m14_ext_f32((float)x,1.5f),"F32");
        check(m14_wrap_external_mix(-7,x,13ul,1.5f),
            m14_ext_mixed(-7,x,13ul,1.5f),"mixed GPR/XMM");
        check(m14_wrap_external_many(-7,x,2u,1.5f,-9l,2.0,13ul,3.0f,
            4,5.0,6u,7.0f,8l,9.0,10.0,11.0),
            m14_ext_many(-7,x,2u,1.5f,-9l,2.0,13ul,3.0f,
            4,5.0,6u,7.0f,8l,9.0,10.0,11.0),
            "7 GPR/9 XMM independent external call spills");
    }
    check(m14_wrap_external_loop(-2.0,3.0),3.0,"external loop");
    check(m14_wrap_external_loop(4.0,3.0),4.0,"external zero iteration");
    if(m14_wrap_external_int_return(2.75)!=(int)m14_ext_step(2.75)) return 1;
    return 0;
}
