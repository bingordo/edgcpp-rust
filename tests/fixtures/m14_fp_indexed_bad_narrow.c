/* Narrow integer index requires explicit integer-promotion semantics. */
double m14_index_bad_narrow(double *p,short i) {return p[i];}
