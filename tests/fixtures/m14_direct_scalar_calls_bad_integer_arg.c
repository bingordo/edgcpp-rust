/* The direct INTEGER argument cannot be arbitrary integer arithmetic. */
double m14_bad_mix_target(int a,double x) {return (double)a+x;}
double m14_bad_mix_caller(int a,double x) {
    return m14_bad_mix_target(a+1,x);
}
