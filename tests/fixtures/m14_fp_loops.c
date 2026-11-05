/* M14: loop carried F64 SSA and structured native EDG control flow. */
double m14_while_sum(double a,double b) {
    double x=a, sum=0.0;
    while(x<b) {sum=sum+x; x=x+1.0;}
    return sum;
}
double m14_for_sum(double a,double b) {
    double sum=0.0;
    for(double i=a;i<b;i=i+1.0) {sum=sum+i;}
    return sum;
}
double m14_do_until(double a,double b) {
    double x=a;
    do {x=x+1.0;} while(x<b);
    return x;
}
double m14_nested_loop(double a,double b) {
    double sum=0.0, x=a;
    while(x<b) {
        double y=0.0;
        while(y<2.0) {sum=sum+x; y=y+1.0;}
        x=x+1.0;
    }
    return sum;
}
double m14_while_break_continue(double a,double b) {
    double x=a,sum=0.0;
    while(x<b) {
        x=x+1.0;
        if(x<0.0) continue;
        if(x>2.0) break;
        sum=sum+x;
    }
    return sum;
}
double m14_for_break_continue(double a,double b) {
    double sum=0.0;
    for(double x=a;x<b;x=x+1.0) {
        if(x<0.0) continue;
        if(x>2.0) break;
        sum=sum+x;
    }
    return sum;
}
double m14_do_break_continue(double a,double b) {
    double x=a,sum=0.0;
    do {
        x=x+1.0;
        if(x<0.0) continue;
        if(x>2.0) break;
        sum=sum+x;
    } while(x<b);
    return sum;
}
double m14_for_infinite_break(double a,double b) {
    double x=a;
    for(;;) {
        if(x>b) break;
        x=x+1.0;
    }
    return x;
}
double m14_do_definite_init(double a,double b) {
    double v;
    double x=a;
    do {
        v=x+1.0;
        x=v;
    } while(v<b);
    return v;
}
double m14_loop_early_return(double a,double b) {
    double x=a;
    while(x<b) {
        if(x>1.0) return x;
        x=x+1.0;
    }
    return x;
}

/* Check F32 phi nodes rather than relying on F64-only coverage. */
float m14_while_f32(float a,float b) {
    float x=a, sum=0.0f;
    while(x<b) {sum=sum+x; x=x+1.0f;}
    return sum;
}
float m14_for_f32(float a,float b) {
    float sum=0.0f;
    for(float x=a;x<b;x=x+1.0f) {
        if(x>0.0f) sum=sum+x;
    }
    return sum;
}
