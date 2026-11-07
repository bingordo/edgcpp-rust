/* Narrow INTEGER locals require separate source promotions and ABI. */
double m14_il_bad_narrow(short n,double x) {
    short copy=n;
    return x+(double)copy;
}
