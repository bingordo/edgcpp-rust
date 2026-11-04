#include <stdio.h>
#include <stdlib.h>
double m14_double_add(double,double);
double m14_double_sub(double,double);
double m14_double_mul(double,double);
double m14_double_div(double,double);
double m14_double_neg(double);
float m14_float_add(float,float);
float m14_float_mul(float,float);
float m14_float_div(float,float);
double m14_double_promote(double,float);
float m14_float_demote(float,double);
double m14_double_stack(double,double,double,double,double,double,double,double,double,double);
static void test(int ok,const char*name){
    if(!ok){fprintf(stderr,"M14 FP failure: %s\n",name);exit(1);}
}
int main(void){
    test(m14_double_add(1.5,2.25)==3.75,"double+");
    test(m14_double_sub(7.0,3.25)==3.75,"double-");
    test(m14_double_mul(2.5,3.0)==7.5,"double*");
    test(m14_double_div(7.5,2.5)==3.0,"double/");
    test(m14_double_neg(2.25)==-2.25,"double neg");
    test(m14_float_add(2.5f,3.25f)==5.75f,"float+");
    test(m14_float_mul(1.25f,2.0f)==2.5f,"float*");
    test(m14_float_div(7.5f,2.5f)==3.0f,"float/");
    test(m14_double_promote(3.0,2.5f)==5.5,"float promote");
    test(m14_float_demote(7.25f,2.5)==4.75f,"double demote");
    test(m14_double_stack(1,2,3,4,5,6,7,8,9.25,10.5)==19.75,"fp 8-register exhaustion");
    return 0;
}
