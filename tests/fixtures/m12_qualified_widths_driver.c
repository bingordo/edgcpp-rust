#include <stdio.h>
#include <stdlib.h>
static void check(const char *name,int n,int got,int expected) {
    if(got!=expected) {
        fprintf(stderr,"%s(%d) got=%d expected=%d\n",name,n,got,expected);
        exit(1);
    }
}
int m12_volatile_i8(int);
int m12_atomic_i8(int);
int m12_volatile_u8(int);
int m12_atomic_u8(int);
int m12_volatile_i16(int);
int m12_atomic_i16(int);
int m12_volatile_u16(int);
int m12_atomic_u16(int);
int m12_volatile_i32(int);
int m12_atomic_i32(int);
int m12_volatile_u32(int);
int m12_atomic_u32(int);
int m12_volatile_i64(int);
int m12_atomic_i64(int);
int m12_volatile_u64(int);
int m12_atomic_u64(int);
int main(void) {
    for(int n=-30; n<=30; ++n) {
        check("volatile i8",n,m12_volatile_i8(n),101*n+2);
        check("atomic i8",n,m12_atomic_i8(n),101*n+2);
    }
    for(int n=0; n<=30; ++n) {
        check("volatile u8",n,m12_volatile_u8(n),101*n+2);
        check("atomic u8",n,m12_atomic_u8(n),101*n+2);
    }
    for(int n=-30; n<=30; ++n) {
        check("volatile i16",n,m12_volatile_i16(n),101*n+2);
        check("atomic i16",n,m12_atomic_i16(n),101*n+2);
    }
    for(int n=0; n<=30; ++n) {
        check("volatile u16",n,m12_volatile_u16(n),101*n+2);
        check("atomic u16",n,m12_atomic_u16(n),101*n+2);
    }
    for(int n=-30; n<=30; ++n) {
        check("volatile i32",n,m12_volatile_i32(n),101*n+2);
        check("atomic i32",n,m12_atomic_i32(n),101*n+2);
    }
    for(int n=0; n<=30; ++n) {
        check("volatile u32",n,m12_volatile_u32(n),101*n+2);
        check("atomic u32",n,m12_atomic_u32(n),101*n+2);
    }
    for(int n=-30; n<=30; ++n) {
        check("volatile i64",n,m12_volatile_i64(n),101*n+2);
        check("atomic i64",n,m12_atomic_i64(n),101*n+2);
    }
    for(int n=0; n<=30; ++n) {
        check("volatile u64",n,m12_volatile_u64(n),101*n+2);
        check("atomic u64",n,m12_atomic_u64(n),101*n+2);
    }
    return 0;
}
