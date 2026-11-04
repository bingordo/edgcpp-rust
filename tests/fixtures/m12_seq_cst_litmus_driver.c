#define _XOPEN_SOURCE 700
#include <stdatomic.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
int m12_sb_left(_Atomic int *, _Atomic int *);
int m12_sb_right(_Atomic int *, _Atomic int *);
static _Atomic int x, y;
static pthread_barrier_t before, after;
static int result[2];
enum { ITERATIONS=10000 };
static void *worker(void *context) {
    int right=(int)(long)context;
    for(int i=0;i<ITERATIONS;i++) {
        int status=pthread_barrier_wait(&before);
        if(status!=0 && status!=PTHREAD_BARRIER_SERIAL_THREAD) abort();
        result[right]=right ? m12_sb_right(&x,&y) : m12_sb_left(&x,&y);
        status=pthread_barrier_wait(&after);
        if(status!=0 && status!=PTHREAD_BARRIER_SERIAL_THREAD) abort();
    }
    return NULL;
}
static void barrier(void) {
    int status=pthread_barrier_wait(&before);
    if(status!=0 && status!=PTHREAD_BARRIER_SERIAL_THREAD) abort();
}
static void finish(void) {
    int status=pthread_barrier_wait(&after);
    if(status!=0 && status!=PTHREAD_BARRIER_SERIAL_THREAD) abort();
}
int main(void) {
    /* pthread_barrier_wait may return PTHREAD_BARRIER_SERIAL_THREAD, which
     * need not be positive; use an explicit status check. */
    if(pthread_barrier_init(&before,NULL,3)!=0 ||
       pthread_barrier_init(&after,NULL,3)!=0) abort();
    pthread_t threads[2];
    if(pthread_create(&threads[0],NULL,worker,(void *)0)!=0 ||
       pthread_create(&threads[1],NULL,worker,(void *)1)!=0) abort();
    for(int i=0;i<ITERATIONS;i++) {
        atomic_store_explicit(&x,0,memory_order_seq_cst);
        atomic_store_explicit(&y,0,memory_order_seq_cst);
        barrier();
        finish();
        if(result[0]==0 && result[1]==0) {
            fprintf(stderr,"seq-cst store buffering: forbidden both-zero at iteration %d\n",i);
            return 1;
        }
    }
    for(int i=0;i<2;i++)
        if(pthread_join(threads[i],NULL)!=0) abort();
    if(pthread_barrier_destroy(&before)!=0 ||
       pthread_barrier_destroy(&after)!=0) abort();
    return 0;
}
