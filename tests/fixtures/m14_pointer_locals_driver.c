#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_ptr_initial(double(*)(double),double);
double m14_ptr_chain(double(*)(double),double);
double m14_ptr_reassign(double(*)(double),double(*)(double),double);
double m14_ptr_branch(double(*)(double),double(*)(double),double,double);
double m14_ptr_loop(double(*)(double),double,double);
long m14_ptr_integer(long(*)(double),double);
static double plus(double x) {return x+1.0;}
static double minus(double x) {return x-1.0;}
static long to_long(double x) {return (long)x+12345L;}
static void eq(double got,double expected,const char *test) {
    if(!(isnan(expected)?isnan(got):got==expected)) {
        fprintf(stderr,"M14 callable pointer local failure %s: %.17g vs %.17g\n",
            test,got,expected);
        exit(1);
    }
}
int main(void) {
    const double numbers[]={-4.0,-0.0,0.0,1.5,4.0,NAN};
    for(size_t i=0;i<sizeof(numbers)/sizeof(numbers[0]);i++) {
        double x=numbers[i];
        eq(m14_ptr_initial(plus,x),plus(x),"initial");
        eq(m14_ptr_chain(minus,x),minus(x),"chained local");
        eq(m14_ptr_reassign(plus,minus,x),minus(x),"reassigned");
        eq(m14_ptr_branch(plus,minus,x,2.0),minus(x),"taken branch");
        eq(m14_ptr_branch(plus,minus,x,-2.0),plus(x),"untaken branch");
    }
    eq(m14_ptr_loop(plus,-2.0,3.0),3.0,"loop");
    eq(m14_ptr_loop(plus,4.0,3.0),4.0,"empty loop");
    if(m14_ptr_integer(to_long,2.5)!=to_long(2.5)) return 1;
    return 0;
}
