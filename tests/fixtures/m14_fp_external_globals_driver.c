#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_fp_shared_d=0.0;
float m14_fp_shared_f=0.0f;
double m14_external_d_add(double);
float m14_external_f_add(float);
double m14_external_d_twice(double);
double m14_external_d_cond(double);
double m14_external_d_loop(double);
static void check(double got,double expected,const char *tag) {
    if(!(isnan(expected)?isnan(got):got==expected)) {
        fprintf(stderr,"M14 FP imported data mismatch %s: %.17g vs %.17g\n",
            tag,got,expected);
        exit(1);
    }
}
int main(void) {
    const double values[]={-3.0,-0.0,0.0,1.25,4.0,NAN};
    for(size_t i=0;i<sizeof(values)/sizeof(values[0]);i++) {
        double x=values[i];
        m14_fp_shared_d=2.5;
        m14_fp_shared_f=1.25f;
        check(m14_external_d_add(x),m14_fp_shared_d+x,"double extern");
        check((double)m14_external_f_add((float)x),
              (double)(m14_fp_shared_f+(float)x),"float extern");
        check(m14_external_d_twice(x),m14_fp_shared_d+m14_fp_shared_d+x,
              "two ordinary extern loads");
        check(m14_external_d_cond(x),
            m14_fp_shared_d>x?m14_fp_shared_d:x,"extern condition");
        m14_fp_shared_d=-4.0;
        m14_fp_shared_f=-2.0f;
        check(m14_external_d_add(x),m14_fp_shared_d+x,"reloaded double");
        check((double)m14_external_f_add((float)x),
            (double)(m14_fp_shared_f+(float)x),"reloaded float");
        check(m14_external_d_twice(x),m14_fp_shared_d+m14_fp_shared_d+x,
            "reloaded twice");
    }
    m14_fp_shared_d=3.0;
    check(m14_external_d_loop(-2.0),3.0,"FP loop extern test");
    m14_fp_shared_d=NAN;
    check(m14_external_d_add(2.0),NAN,"NaN external global");
    check(m14_external_d_cond(2.0),2.0,"NaN external comparison");
    return 0;
}
