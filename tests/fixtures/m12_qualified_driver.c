/* GCC native caller, qualified global definitions and differential oracle. */
#include <stdatomic.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
volatile signed char m12_v8;
volatile short m12_v16;
volatile int m12_v32;
volatile long m12_v64;
_Atomic int m12_a32;
int m12_v32_read_twice(void);
void m12_v32_write_pair(int);
int m12_v32_post(void);
int m12_v32_compound(int);
long m12_vary_widths(void);
int m12_atomic_get(void);
void m12_atomic_put(int);
int m12_atomic_two_reads(void);
int m12_atomic_post(void);
int m12_atomic_add(int);
int m12_atomic_dec(void);
static void expect(const char *label,long actual,long want) {
    if(actual!=want) {
        fprintf(stderr,"%s: got %ld expected %ld\n",label,actual,want);
        exit(1);
    }
}
static void *atomic_counter_worker(void *arg) {
    (void)arg;
    for (int i=0;i<10000;++i) m12_atomic_post();
    return NULL;
}
int main(void) {
    for (int x=-31;x<=31;x+=7) {
        m12_v32=x;
        expect("twice",m12_v32_read_twice(),2*x);
        m12_v32_write_pair(x);
        expect("pair",m12_v32,x+1);
        expect("post",m12_v32_post(),x+1);
        expect("post stored",m12_v32,x+2);
        expect("compound",m12_v32_compound(5),x+7);
        expect("compound stored",m12_v32,x+7);
        m12_atomic_put(x);
        expect("atomic get",m12_atomic_get(),x);
        expect("atomic two reads",m12_atomic_two_reads(),2*x);
        expect("atomic global",atomic_load_explicit(&m12_a32,memory_order_seq_cst),x);
        expect("atomic post",m12_atomic_post(),x);
        expect("atomic post stored",m12_atomic_get(),x+1);
        expect("atomic compound",m12_atomic_add(9),x+10);
        expect("atomic decrement",m12_atomic_dec(),x+9);
    }
    m12_atomic_put(0);
    pthread_t threads[4];
    for (int i=0;i<4;++i)
        if (pthread_create(&threads[i],NULL,atomic_counter_worker,NULL)) abort();
    for (int i=0;i<4;++i)
        if (pthread_join(threads[i],NULL)) abort();
    expect("four-thread atomic RMW",m12_atomic_get(),40000);
    expect("native widths",m12_vary_widths(),1234763);
    expect("8-bit",m12_v8,-4);
    expect("16-bit",m12_v16,200);
    expect("64-bit",m12_v64,1234567);
    return 0;
}
