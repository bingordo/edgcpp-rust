/* Qualified/volatile pointees require separate ordered memory effects. */
double m14_addr_volatile(double x) {
    volatile double local=x;
    volatile double *p=&local;
    *p=3.0;
    return local;
}
