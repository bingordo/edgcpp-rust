/* No casts to fabricated pointer provenance. */
double m14_index_bad_pointer(double *p,int i) {
    float *q=(float*)p;
    return q[i];
}
