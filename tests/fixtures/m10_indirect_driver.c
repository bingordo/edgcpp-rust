#include <stdio.h>
#include <stdlib.h>

typedef long (*m10_callback)(long, long);
extern long m10_invoke(m10_callback, long, long);
extern long m10_external_address(long, long);
extern long m10_local_address(long, long);
extern long m10_choose(int, long, long);
extern long m10_callback_twice(m10_callback, long);

long m10_external(long x, long y) { return x - 2*y; }
static long driver_callback(long x, long y) { return x + 5*y; }

static void check(const char *name, long actual, long expected) {
    if (actual != expected) {
        fprintf(stderr, "%s: got %ld expected %ld\n", name, actual, expected);
        exit(1);
    }
}
int main(void) {
    const long values[] = {-101, -1, 0, 1, 27, 1001};
    for (unsigned i=0; i<sizeof(values)/sizeof(values[0]); ++i) {
        long x=values[i], y=values[(i+2)%6];
        check("callback",m10_invoke(driver_callback,x,y),x+5*y);
        check("external",m10_external_address(x,y),x-2*y);
        check("local",m10_local_address(x,y),3*x+y);
        check("choose external",m10_choose(1,x,y),x-2*y);
        check("choose local",m10_choose(0,x,y),3*x+y);
        long first=x+35;
        check("twice",m10_callback_twice(driver_callback,x),first+5*(-3)+first);
    }
    return 0;
}
