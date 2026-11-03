/* Qualified C lvalues through explicit pointer parameters.  All storage
 * belongs to an independently GCC-compiled driver, never to our ELF. */
int m12_indirect_volatile_read(volatile int *p) {
    int a=*p;
    int b=*p;
    return a+b;
}
int m12_indirect_volatile_write(volatile int *p,int x) {
    *p=x;
    *p=x+4;
    return *p;
}
short m12_indirect_volatile_short(volatile short *p,short x) {
    *p=x;
    return *p;
}
int m12_indirect_volatile_index(volatile int *p,int offset,int x) {
    p[offset]=x;
    return p[offset]+p[0];
}
int m12_indirect_atomic_get(_Atomic int *p) {return *p;}
void m12_indirect_atomic_put(_Atomic int *p,int value) {*p=value;}
int m12_indirect_atomic_twice(_Atomic int *p) {
    int a=*p;
    int b=*p;
    return a+b;
}
