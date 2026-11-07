/* Implicit C integer conversion of a scalar local needs explicit semantics. */
double m14_il_bad_promotion(int n,double x) {
    unsigned long long wide=n;
    return x+(double)wide;
}
