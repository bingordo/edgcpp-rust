#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
int m12_explicit_load(_Atomic int *);
void m12_explicit_store(_Atomic int *,int);
int m12_explicit_exchange(_Atomic int *,int);
int m12_explicit_fetch(_Atomic int *,int);
int m12_explicit_cas(_Atomic int *,int *,int);
int m12_explicit_weak_cas(_Atomic int *,int *,int);
void m12_explicit_fence(void);
void m12_explicit_publish(_Atomic int *,int *,int);
int m12_explicit_read(_Atomic int *,int *,int);
static void check(const char *name,int got,int expected) {
    if(got!=expected) {fprintf(stderr,"%s: got %d expected %d\n",name,got,expected);exit(1);}
}
static _Atomic int ready;
static int payload;
static void *publish(void *unused) {
    (void)unused;
    m12_explicit_publish(&ready,&payload,731);
    return NULL;
}
int main(void) {
    _Atomic int value=3;
    check("load",m12_explicit_load(&value),3);
    m12_explicit_store(&value,7);
    check("store",atomic_load(&value),7);
    check("exchange old",m12_explicit_exchange(&value,11),7);
    check("exchange new",atomic_load(&value),11);
    int expected=11;
    check("cas true",m12_explicit_cas(&value,&expected,22),1);
    check("cas success expected unchanged",expected,11);
    check("cas success value",atomic_load(&value),22);
    expected=11;
    check("cas false",m12_explicit_cas(&value,&expected,33),0);
    check("cas failure expected updated",expected,22);
    check("cas failure object unchanged",atomic_load(&value),22);
    expected=22;
    check("weak cas",m12_explicit_weak_cas(&value,&expected,33),1);
    check("weak cas value",atomic_load(&value),33);
    m12_explicit_fence();
    int before=atomic_load(&value);
    int result=m12_explicit_fetch(&value,3);
    int a=before,b=before+3,c=b-2,d=c^0x13,e=d&255;
    check("fetch aggregate old results",result,a+b+c+d+e);
    check("fetch final",atomic_load(&value),e|0x40);
    pthread_t thread;
    atomic_store(&ready,0);
    if(pthread_create(&thread,NULL,publish,NULL)!=0) abort();
    check("release/acquire publication",m12_explicit_read(&ready,&payload,731),731);
    if(pthread_join(thread,NULL)!=0) abort();
    return 0;
}
