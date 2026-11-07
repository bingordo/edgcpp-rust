/* An external C function may retain the pointer: no storage escape model. */
extern void m14_addr_unknown_external(double *);
double m14_addr_escape(double x) {
    double local=x;
    m14_addr_unknown_external(&local);
    return local;
}
