#include <math.h>
#include <stdio.h>
#include <stdlib.h>

double m14_while_sum(double,double);
double m14_for_sum(double,double);
double m14_do_until(double,double);
double m14_nested_loop(double,double);
double m14_while_break_continue(double,double);
double m14_for_break_continue(double,double);
double m14_do_break_continue(double,double);
double m14_for_infinite_break(double,double);
double m14_do_definite_init(double,double);
double m14_loop_early_return(double,double);

static void eq(double actual,double expected,const char *test) {
    if(!(isnan(expected) ? isnan(actual) : actual==expected)) {
        fprintf(stderr,"M14 loop SSA mismatch %s: got %.17g, wanted %.17g\n",
                test,actual,expected);
        exit(1);
    }
}
int main(void) {
    const double pairs[][2]={
        {-2.0,2.0},{0.0,4.0},{2.0,2.0},{3.0,1.0},
        {-0.0,+0.0},{NAN,2.0},{2.0,NAN},{NAN,NAN},
        {1.25,3.25},{-1.0,2.0},{0.5,1.5},
    };
    for(size_t k=0;k<sizeof(pairs)/sizeof(pairs[0]);++k) {
        const double a=pairs[k][0],b=pairs[k][1];
        double x=a,sum=0.0;
        while(x<b) {sum+=x;x+=1.0;}
        eq(m14_while_sum(a,b),sum,"while loop-carried accumulator");
        eq(m14_for_sum(a,b),sum,"for initializer and increment");
        double nested_expected=sum+sum;
        eq(m14_nested_loop(a,b),nested_expected,"nested loop joins");
        x=a;
        do{x=x+1.0;}while(x<b);
        eq(m14_do_until(a,b),x,"do while backedge");
        eq(m14_do_definite_init(a,b),x,"do while first-iteration definite init");
        x=a;sum=0.0;
        while(x<b) {
            x+=1.0;
            if(x<0.0)continue;
            if(x>2.0)break;
            sum+=x;
        }
        eq(m14_while_break_continue(a,b),sum,"while break/continue");
        x=a;sum=0.0;
        for(;x<b;x+=1.0) {
            if(x<0.0)continue;
            if(x>2.0)break;
            sum+=x;
        }
        eq(m14_for_break_continue(a,b),sum,"for continue reaches increment");
        x=a;sum=0.0;
        do {
            x+=1.0;
            if(x<0.0)continue;
            if(x>2.0)break;
            sum+=x;
        }while(x<b);
        eq(m14_do_break_continue(a,b),sum,"do continue reaches test");
        x=a;
        while(x<b) {
            if(x>1.0)break;
            x+=1.0;
        }
        eq(m14_loop_early_return(a,b),x,"nested early return");
        /* for(;;) must have a break; NaN would make this nonterminating. */
        if(!isnan(a)&&!isnan(b)) {
            x=a;for(;;){if(x>b)break;x+=1.0;}
            eq(m14_for_infinite_break(a,b),x,"for no condition break edge");
        }
    }
    return 0;
}
