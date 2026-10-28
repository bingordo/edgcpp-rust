/* A real C11 scalar expression fixture, without C++ ABI syntax. */
int m3_c_lazy_and(int a) { int x=0; int b=a && (x=7); return x+b*100; }
int m3_c_lazy_or(int a) { int x=0; int b=a || (x=5); return x+b*100; }
int m3_c_ternary(int a) { int x=2; int y=a ? (x+=3):(x*=4); return x+10*y; }
int m3_c_comma(int a) { int x=1; return (x+=a, x*=3, x); }
int m3_c_postfix(int a) { int x=a; int prev=x++; return prev*100+x; }
int m3_c_prefix(int a) { int x=a; int current=++x; return current*100+x; }
unsigned int m3_c_compound(unsigned int a, int rhs) {
    unsigned int x=a; x%=rhs; return x;
}
int m3_c_for(int n) { int i=0; int sum=0; for(i=0;i<n;i++) sum+=i; return sum; }
_Bool m3_c_bool(int x) { _Bool b=x; b=b&&1; return b; }
