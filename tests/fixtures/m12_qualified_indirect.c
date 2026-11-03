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

/* Increment/decrement and supported compounds through qualified pointers:
 * an atomic operator must execute as one RMW, not a load/store pair. */
int m12_indirect_atomic_post(_Atomic int *p) { return (*p)++; }
int m12_indirect_atomic_pre(_Atomic int *p) { return ++(*p); }
int m12_indirect_atomic_sub(_Atomic int *p,int n) { return *p -= n; }
int m12_indirect_atomic_add(_Atomic int *p,int n) { return *p += n; }
int m12_indirect_atomic_xor(_Atomic int *p,int n) { return *p ^= n; }
int m12_indirect_atomic_and(_Atomic int *p,int n) { return *p &= n; }
int m12_indirect_atomic_or(_Atomic int *p,int n) { return *p |= n; }
int m12_indirect_volatile_post(volatile int *p) { return (*p)++; }
int m12_indirect_volatile_pre(volatile int *p) { return --(*p); }
int m12_indirect_volatile_add(volatile int *p,int n) { return *p += n; }
short m12_indirect_atomic_short_post(_Atomic short *p) { return (*p)++; }

/* Arithmetic not representable by native fetch-add/bitwise RMW must use an
 * atomic CAS retry loop. Each RHS is evaluated exactly once. */
int m12_indirect_atomic_mul(_Atomic int *p,int n) { return *p *= n; }
int m12_indirect_atomic_div(_Atomic int *p,int n) { return *p /= n; }
int m12_indirect_atomic_rem(_Atomic int *p,int n) { return *p %= n; }
int m12_indirect_atomic_shl(_Atomic int *p,int n) { return *p <<= n; }
int m12_indirect_atomic_shr(_Atomic int *p,int n) { return *p >>= n; }
/* The external RHS has an observable effect; CAS retries must not replay it. */
extern int m12_rhs_once(void);
int m12_indirect_atomic_mul_rhs(_Atomic int *p) {
    return *p *= m12_rhs_once();
}
