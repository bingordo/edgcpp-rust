/* Qualified dereferences require a stronger memory-effect contract. */
double m14_index_bad_volatile(volatile double *p,int i) {return p[i];}
