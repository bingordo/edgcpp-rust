/* Taking the address before an FP stack object's checked initialization
 * currently fails closed, including attempts to read indeterminate data. */
double m14_addr_uninit(void) {
    double local;
    double *p=&local;
    return *p;
}
