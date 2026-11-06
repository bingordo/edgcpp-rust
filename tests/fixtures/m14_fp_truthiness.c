/* Native C floating-point truthiness is x != 0.0, not ordered x>0. */
double m14_truth_if_f64(double x) {
    if(x) return 11.0;
    return 22.0;
}
float m14_truth_if_f32(float x) {
    if(x) return 3.0f;
    return -3.0f;
}
double m14_truth_if_not(double x) {
    if(!x) return 11.0;
    return 22.0;
}
double m14_truth_if_expr(double x) {
    if(x+0.0) return 5.0;
    return 7.0;
}
double m14_truth_while(double x) {
    double value=x;
    double sum=0.0;
    while(value) {
        sum=sum+1.0;
        value=value-1.0;
    }
    return sum;
}
double m14_truth_do_while(double x) {
    double value=x;
    double sum=0.0;
    do {
        sum=sum+1.0;
        value=value-1.0;
    } while(value);
    return sum;
}
