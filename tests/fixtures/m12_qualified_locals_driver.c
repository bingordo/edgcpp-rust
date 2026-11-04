#include <stdio.h>
#include <stdlib.h>
int m12_auto_volatile(int);
int m12_auto_atomic(int);
int m12_auto_atomic_pointer(int);
static void check(const char *name,int x,int got,int expected) {
    if (got != expected) {
        fprintf(stderr, "%s(%d): got=%d expected=%d\n", name,x,got,expected);
        exit(1);
    }
}
int main(void) {
    for (int n=-100;n<=100;n++) {
        check("volatile local",n,m12_auto_volatile(n),
              n*10000 + (n+5)*100 + 2*(n+5));
        check("atomic local",n,m12_auto_atomic(n),
              n*10000 + (n+6)*100 + 6*(n+6));
        check("address-taken atomic",n,m12_auto_atomic_pointer(n),
              n*100 + (n+1));
    }
    return 0;
}
