#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
double m14_fp_ptr_read(double*);
float m14_fp_ptr_f32(float*);
double m14_fp_ptr_store(double*,double);
float m14_fp_ptr_store_f32(float*,float);
double m14_fp_ptr_alias(double*,double*,double);
double m14_fp_ptr_accum(double*,double);
double m14_fp_ptr_branch(double*,double,double);
double m14_fp_ptr_loop(double*,double,double);
double m14_fp_ptr_eight_sse(double,double,double,double,double,double,
                            double,double,double*,double);
static void eq64(double a,double b,const char *msg) {
    uint64_t aa,bb;memcpy(&aa,&a,8);memcpy(&bb,&b,8);
    if(aa!=bb) {
        fprintf(stderr,"pointer F64 memory %s mismatch: %llx != %llx\n",
            msg,(unsigned long long)aa,(unsigned long long)bb);
        exit(1);
    }
}
static void eq32(float a,float b,const char *msg) {
    uint32_t aa,bb;memcpy(&aa,&a,4);memcpy(&bb,&b,4);
    if(aa!=bb) {
        fprintf(stderr,"pointer F32 memory %s mismatch: %x != %x\n",msg,aa,bb);
        exit(1);
    }
}
int main(void) {
    double d=-0.0;
    eq64(m14_fp_ptr_read(&d),d,"read signed zero");
    eq64(m14_fp_ptr_store(&d,3.25),3.25,"write return");
    eq64(d,3.25,"write observable");
    eq64(m14_fp_ptr_alias(&d,&d,-5.75),-5.75,"alias read");
    eq64(d,-5.75,"alias write");
    eq64(m14_fp_ptr_accum(&d,2.5),-3.25,"load add store");
    eq64(m14_fp_ptr_branch(&d,1.0,0.0),1.0,"taken branch");
    eq64(m14_fp_ptr_branch(&d,9.0,10.0),1.0,"untaken branch");
    eq64(m14_fp_ptr_loop(&d,-2.0,2.0),1.0,"loop writes");
    eq64(d,1.0,"loop observable");
    eq64(m14_fp_ptr_eight_sse(1,2,3,4,5,6,7,8,&d,12.5),48.5,
         "stack-passed pointer and FP value");
    eq64(d,12.5,"stack pointer writes");
    d=NAN;eq64(m14_fp_ptr_read(&d),d,"NaN unchanged");
    float f=-0.0f;
    eq32(m14_fp_ptr_f32(&f),f,"F32 signed zero");
    eq32(m14_fp_ptr_store_f32(&f,2.75f),2.75f,"F32 write");
    eq32(f,2.75f,"F32 observable");
    return 0;
}
