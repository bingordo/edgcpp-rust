/* M14 now accepts nonescaping, definitely initialized callable-pointer
 * locals. Taking the address of a pointer local requires memory/alias
 * tracking and therefore remains an explicitly rejected feature. */
double m14_bad_indirect_local(double (*fp)(double),double x) {
    double (*local)(double)=fp;
    double (**alias)(double)=&local;
    return (*alias)(x);
}
