#include <math.h>
#include <stdio.h>
#include <stdlib.h>
extern double m14_zero_d;
extern float m14_zero_f;
double m14_zero_read_d(void);
float m14_zero_read_f(void);
double m14_zero_store_d(double);
float m14_zero_store_f(float);
double m14_zero_add_d(double);
double m14_zero_branch(double,double);
double m14_zero_static(double);
static void check(double a,double b,const char *name) {
    if(!(isnan(b)?isnan(a):a==b)) {
        fprintf(stderr,"M14 owned FP zero-global mismatch %s %.17g != %.17g\n",
                name,a,b);
        exit(1);
    }
}
int main(void) {
    check(m14_zero_d,0.0,"BSS double zero");
    check((double)m14_zero_f,0.0,"BSS float zero");
    check(m14_zero_read_d(),0.0,"read zero");
    check((double)m14_zero_read_f(),0.0,"read F32 zero");
    check(m14_zero_store_d(-0.0),-0.0,"store signed zero");
    if(!signbit(m14_zero_read_d())) {fputs("lost signed zero\n",stderr);return 1;}
    check((double)m14_zero_store_f(-1.25f),-1.25,"store F32");
    check(m14_zero_add_d(2.0),2.0,"read modify write");
    check(m14_zero_branch(4.5,1.0),4.5,"taken branch store");
    check(m14_zero_branch(-4.0,1.0),4.5,"untaken branch retains");
    check(m14_zero_static(3.5),3.5,"static storage");
    check(m14_zero_static(-2.25),-2.25,"static reassignment");
    check(m14_zero_store_d(NAN),NAN,"NaN stored");
    check(m14_zero_read_d(),NAN,"NaN reloaded");
    check(m14_zero_d,NAN,"exported symbol observed by GCC");
    m14_zero_d=8.25;
    check(m14_zero_read_d(),8.25,"GCC mutation observed by Cranelift");
    m14_zero_f=2.5f;
    check(m14_zero_read_f(),2.5,"GCC F32 mutation observed");
    return 0;
}
