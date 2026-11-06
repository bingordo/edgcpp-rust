#include <math.h>
#include <stdio.h>
#include <stdlib.h>
double m14_indirect_f64(double(*)(double,double),double,double);
float m14_indirect_f32(float(*)(float,float),float,float);
double m14_indirect_mixed(double(*)(int,double,unsigned long,float),
    int,double,unsigned long,float);
double m14_indirect_spill(int,double,unsigned int,float,long,double,
    unsigned long,float,int,double,unsigned int,float,long,double,double,double,
    double(*)(int,double,unsigned int,float,long,double,unsigned long,float,
        int,double,unsigned int,float,long,double,double,double));
double m14_indirect_loop(double(*)(double),double,double);
int m14_indirect_integer_result(double(*)(double),double);

static double df_add(double a,double b) {return a+b;}
static double df_sub(double a,double b) {return a-b;}
static float ff_add(float a,float b) {return a+b;}
static double df_step(double x) {return x+1.0;}
static double df_mixed(int a,double x,unsigned long b,float y) {
    return (double)a+x+(double)b+(double)y;
}
static double df_spill(int a0,double x0,unsigned int a1,float x1,
    long a2,double x2,unsigned long a3,float x3,int a4,double x4,
    unsigned int a5,float x5,long a6,double x6,double x7,double x8) {
    return (double)a0+x0+(double)a1+(double)x1+
           (double)a2+x2+(double)a3+(double)x3+
           (double)a4+x4+(double)a5+(double)x5+
           (double)a6+x6+x7+x8;
}
static void eq(double got,double wanted,const char *name) {
    if (!(isnan(wanted)?isnan(got):got==wanted)) {
        fprintf(stderr,"M14 indirect scalar call failure %s got %.17g expected %.17g\n",
            name,got,wanted);
        exit(1);
    }
}
int main(void) {
    const double values[]={-3.0,-0.0,0.0,0.5,2.25,NAN};
    for (size_t i=0;i<sizeof(values)/sizeof(values[0]);i++) {
        double v=values[i];
        eq(m14_indirect_f64(df_add,v,1.5),df_add(v,1.5),"F64 add pointer");
        eq(m14_indirect_f64(df_sub,v,1.5),df_sub(v,1.5),"F64 subtract pointer");
        eq((double)m14_indirect_f32(ff_add,(float)v,1.25f),
            (double)ff_add((float)v,1.25f),"F32 pointer call");
        eq(m14_indirect_mixed(df_mixed,-7,v,13ul,1.5f),
            df_mixed(-7,v,13ul,1.5f),"mixed INTEGER/SSE pointer");
        eq(m14_indirect_spill(-7,v,2u,1.5f,-9l,2.0,13ul,3.0f,
            4,5.0,6u,7.0f,8l,9.0,10.0,11.0,df_spill),
            df_spill(-7,v,2u,1.5f,-9l,2.0,13ul,3.0f,
                4,5.0,6u,7.0f,8l,9.0,10.0,11.0),
            "7-GPR/9-XMM overflow plus pointer stack formal");
    }
    eq(m14_indirect_loop(df_step,-2.0,3.0),3.0,"loop-carried SSA indirect call");
    eq(m14_indirect_loop(df_step,4.0,3.0),4.0,"zero iteration");
    if (m14_indirect_integer_result(df_step,2.75)!=(int)3.75) return 1;
    return 0;
}
