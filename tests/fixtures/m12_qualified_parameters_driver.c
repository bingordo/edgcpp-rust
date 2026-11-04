#include <stdio.h>
#include <stdlib.h>
int m12_formal_volatile(volatile int);
int m12_formal_atomic(_Atomic int);
short m12_formal_volatile_short(volatile short);
static void check(const char *name,int n,int got,int expected) {
    if(got!=expected) {
        fprintf(stderr,"%s(%d) got=%d expected=%d\n",name,n,got,expected);
        exit(1);
    }
}
int main(void) {
    for(int n=-100;n<=100;n++) {
        check("volatile formal",n,m12_formal_volatile(n),101*n+3);
        check("atomic formal",n,m12_formal_atomic(n),103*n+3);
        check("volatile short formal",n,m12_formal_volatile_short((short)n),2*n+2);
    }
    return 0;
}
