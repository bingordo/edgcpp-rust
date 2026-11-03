#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
int m12_indirect_volatile_read(volatile int *);
int m12_indirect_volatile_write(volatile int *,int);
short m12_indirect_volatile_short(volatile short *,short);
int m12_indirect_volatile_index(volatile int *,int,int);
int m12_indirect_atomic_get(_Atomic int *);
void m12_indirect_atomic_put(_Atomic int *,int);
int m12_indirect_atomic_twice(_Atomic int *);
static void check(const char *what,long actual,long expected) {
    if(actual!=expected) {
        fprintf(stderr,"%s got=%ld expected=%ld\n",what,actual,expected);
        exit(1);
    }
}
int main(void) {
    volatile int v[4]={3,5,7,9};
    volatile short h=1;
    _Atomic int a=0;
    for(int n=-29;n<=29;n+=7) {
        v[0]=n;
        check("volatile double",m12_indirect_volatile_read(v),2*n);
        check("volatile writes",m12_indirect_volatile_write(v,n),n+4);
        check("volatile written",v[0],n+4);
        check("volatile short",m12_indirect_volatile_short(&h,(short)n),n);
        check("volatile indexed",m12_indirect_volatile_index(v,2,n),2*n+4);
        check("volatile indexed storage",v[2],n);
        m12_indirect_atomic_put(&a,n);
        check("atomic pointed read",m12_indirect_atomic_get(&a),n);
        check("atomic pointed repeated",m12_indirect_atomic_twice(&a),2*n);
        check("atomic pointed stored",atomic_load_explicit(&a,memory_order_seq_cst),n);
    }
    return 0;
}
