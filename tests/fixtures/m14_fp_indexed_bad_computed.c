/* Integer arithmetic under the pointer index has no checked semantics yet. */
double m14_index_bad_computed(double *p,int i) {return p[i+1];}
