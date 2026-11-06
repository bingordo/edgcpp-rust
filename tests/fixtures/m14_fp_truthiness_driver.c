#include <float.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_truth_if_f64(double);
float m14_truth_if_f32(float);
double m14_truth_if_not(double);
double m14_truth_if_expr(double);
double m14_truth_while(double);
double m14_truth_do_while(double);
static void check(double got,double expected,const char *label) {
    if(!(isnan(expected)?isnan(got):got==expected)) {
        fprintf(stderr,"M14 FP truthiness mismatch %s %.17g vs %.17g\n",
                label,got,expected);
        exit(1);
    }
}
int main(void) {
    const double values[]={-INFINITY,-3.5,-DBL_MIN,-0.0,0.0,
        DBL_MIN,3.5,INFINITY,NAN};
    for(size_t i=0;i<sizeof(values)/sizeof(values[0]);i++) {
        double x=values[i];
        check(m14_truth_if_f64(x),x!=0.0?11.0:22.0,"f64");
        check(m14_truth_if_not(x),x==0.0?11.0:22.0,"logical !");
        check(m14_truth_if_expr(x),x+0.0!=0.0?5.0:7.0,"expression");
        check((double)m14_truth_if_f32((float)x),
              (float)x!=0.0f?3.0:-3.0,"f32");
    }
    check(m14_truth_while(0.0),0.0,"zero iteration");
    check(m14_truth_while(4.0),4.0,"loop bool-cast");
    check(m14_truth_do_while(2.0),2.0,"do/while bool-cast");
    check(m14_truth_do_while(1.0),1.0,"do/while once");
    return 0;
}
