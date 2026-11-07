#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>
double m14_ternary_truth(double,double,double);
double m14_ternary_compare(double,double,double,double);
double m14_ternary_nested(double,double,double,double,double);
float m14_ternary_f32(float,float,float);
double m14_ternary_call(double,double,double);
double m14_ternary_arithmetic(double,double,double);
static int yes_count,no_count;
double m14_ternary_yes(double x) {yes_count++;return x+2.5;}
double m14_ternary_no(double x) {no_count++;return x-3.5;}
static void same64(double a,double b,const char *case_name) {
    uint64_t x,y;
    memcpy(&x,&a,sizeof(x));
    memcpy(&y,&b,sizeof(y));
    if(x!=y) {
        fprintf(stderr,"FP conditional mismatch %s bits %llx vs %llx\n",
            case_name,(unsigned long long)x,(unsigned long long)y);
        exit(1);
    }
}
static void same32(float a,float b,const char *case_name) {
    uint32_t x,y;
    memcpy(&x,&a,sizeof(x));
    memcpy(&y,&b,sizeof(y));
    if(x!=y) {
        fprintf(stderr,"F32 conditional mismatch %s bits %x vs %x\n",
            case_name,x,y);
        exit(1);
    }
}
int main(void) {
    const double conditions[]={0.0,-0.0,1.0,-1.0,NAN,INFINITY,-INFINITY};
    const double a=-0.0,b=42.25;
    for(size_t i=0;i<sizeof(conditions)/sizeof(conditions[0]);i++){
        double x=conditions[i];
        same64(m14_ternary_truth(x,a,b),x?a:b,"truth");
        same64(m14_ternary_compare(x,1.0,a,b),x<1.0?a:b,"compare");
        same64(m14_ternary_nested(x,-1.0,a,b,7.0),
            x?(-1.0?a:b):7.0,"nested");
        same64(m14_ternary_arithmetic(x,1.5,3.0),
            1.0+(x?1.5:3.0),"arithmetic");
        same32(m14_ternary_f32((float)x,-0.0f,2.5f),
            (float)x?-0.0f:2.5f,"F32");
        yes_count=0;no_count=0;
        double got=m14_ternary_call(x,12.5,3.25);
        double expected=x?15.0:-0.25;
        same64(got,expected,"lazy external calls");
        if(yes_count!=(x?1:0)||no_count!=(x?0:1)){
            fprintf(stderr,"FP conditional evaluated unselected call arm\n");
            return 1;
        }
    }
    return 0;
}
