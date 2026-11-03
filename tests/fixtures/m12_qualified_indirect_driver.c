#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
int m12_indirect_volatile_read(volatile int *);
int m12_indirect_volatile_write(volatile int *,int);
short m12_indirect_volatile_short(volatile short *,short);
int m12_indirect_volatile_index(volatile int *,int,int);
int m12_indirect_atomic_get(_Atomic int *);
void m12_indirect_atomic_put(_Atomic int *,int);
int m12_indirect_atomic_twice(_Atomic int *);
int m12_indirect_atomic_post(_Atomic int *);
int m12_indirect_atomic_pre(_Atomic int *);
int m12_indirect_atomic_sub(_Atomic int *,int);
int m12_indirect_atomic_add(_Atomic int *,int);
int m12_indirect_atomic_xor(_Atomic int *,int);
int m12_indirect_atomic_and(_Atomic int *,int);
int m12_indirect_atomic_or(_Atomic int *,int);
int m12_indirect_volatile_post(volatile int *);
int m12_indirect_volatile_pre(volatile int *);
int m12_indirect_volatile_add(volatile int *,int);
short m12_indirect_atomic_short_post(_Atomic short *);
int m12_indirect_atomic_mul(_Atomic int *,int);
int m12_indirect_atomic_div(_Atomic int *,int);
int m12_indirect_atomic_rem(_Atomic int *,int);
int m12_indirect_atomic_shl(_Atomic int *,int);
int m12_indirect_atomic_shr(_Atomic int *,int);
int m12_indirect_atomic_mul_rhs(_Atomic int *);
static _Atomic int rhs_calls;
int m12_rhs_once(void) {
    atomic_fetch_add_explicit(&rhs_calls,1,memory_order_seq_cst);
    return 1;
}
static _Atomic int concurrent_counter;
static void *worker(void *arg) {
    (void)arg;
    for (int i=0;i<10000;++i) (void)m12_indirect_atomic_post(&concurrent_counter);
    return NULL;
}
static void *cas_worker(void *arg) {
    (void)arg;
    for (int i=0;i<10000;++i) (void)m12_indirect_atomic_mul_rhs(&concurrent_counter);
    return NULL;
}
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
    _Atomic short small=100;
    for(int n=-29;n<=29;n+=7) {
        v[0]=n;
        check("volatile double",m12_indirect_volatile_read(v),2*n);
        check("volatile writes",m12_indirect_volatile_write(v,n),n+4);
        check("volatile written",v[0],n+4);
        check("volatile short",m12_indirect_volatile_short(&h,(short)n),n);
        check("volatile indexed",m12_indirect_volatile_index(v,2,n),2*n+4);
        check("volatile indexed storage",v[2],n);
        v[0]=n;
        check("volatile post old",m12_indirect_volatile_post(v),n);
        check("volatile post store",v[0],n+1);
        check("volatile pre new",m12_indirect_volatile_pre(v),n);
        check("volatile pre store",v[0],n);
        check("volatile compound new",m12_indirect_volatile_add(v,5),n+5);
        check("volatile compound store",v[0],n+5);
        m12_indirect_atomic_put(&a,n);
        check("atomic pointed read",m12_indirect_atomic_get(&a),n);
        check("atomic pointed repeated",m12_indirect_atomic_twice(&a),2*n);
        check("atomic pointed stored",atomic_load_explicit(&a,memory_order_seq_cst),n);
        check("atomic post old",m12_indirect_atomic_post(&a),n);
        check("atomic post store",m12_indirect_atomic_get(&a),n+1);
        check("atomic pre new",m12_indirect_atomic_pre(&a),n+2);
        check("atomic sub new",m12_indirect_atomic_sub(&a,3),n-1);
        check("atomic add new",m12_indirect_atomic_add(&a,7),n+6);
        int value=(n+6)^0x55;
        check("atomic xor new",m12_indirect_atomic_xor(&a,0x55),value);
        value &= 15;
        check("atomic and new",m12_indirect_atomic_and(&a,15),value);
        value |= 0x40;
        check("atomic or new",m12_indirect_atomic_or(&a,0x40),value);
        check("atomic RMW stored",m12_indirect_atomic_get(&a),value);
    }
    m12_indirect_atomic_put(&a,120);
    check("CAS multiply",m12_indirect_atomic_mul(&a,3),360);
    check("CAS divide",m12_indirect_atomic_div(&a,6),60);
    check("CAS remainder",m12_indirect_atomic_rem(&a,7),4);
    check("CAS shift left",m12_indirect_atomic_shl(&a,2),16);
    check("CAS shift right",m12_indirect_atomic_shr(&a,1),8);
    check("CAS stored",m12_indirect_atomic_get(&a),8);
    check("atomic short old",m12_indirect_atomic_short_post(&small),100);
    check("atomic short stored",atomic_load_explicit(&small,memory_order_seq_cst),101);
    atomic_store_explicit(&concurrent_counter,0,memory_order_seq_cst);
    pthread_t threads[4];
    for (int i=0;i<4;++i) {
        if(pthread_create(&threads[i],NULL,worker,NULL)!=0) abort();
    }
    for (int i=0;i<4;++i) {
        if(pthread_join(threads[i],NULL)!=0) abort();
    }
    check("four-thread indirect atomic RMW",
          atomic_load_explicit(&concurrent_counter,memory_order_seq_cst),40000);
    atomic_store_explicit(&concurrent_counter,0,memory_order_seq_cst);
    for (int i=0;i<4;++i) {
        if(pthread_create(&threads[i],NULL,
                          i<2?worker:cas_worker,NULL)!=0) abort();
    }
    for (int i=0;i<4;++i) {
        if(pthread_join(threads[i],NULL)!=0) abort();
    }
    check("contended atomic CAS retry",
          atomic_load_explicit(&concurrent_counter,memory_order_seq_cst),20000);
    check("CAS RHS evaluated once per source expression",
          atomic_load_explicit(&rhs_calls,memory_order_seq_cst),20000);
    return 0;
}
