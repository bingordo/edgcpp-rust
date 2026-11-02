#include <stdio.h>
#include <stdint.h>

short m6_external(short x, unsigned char y) { return (short)(x * 2 + y); }
void m6_touch(unsigned int *p) { *p ^= 0x5au; }
unsigned int m6_zero(void) { return 19u; }
extern long long m6_execute(int);


extern long m6_for_ever_break(int);
extern int m6_for_ever_return(int);
extern int m6_for_ever_continue(int);

int main(void)
{
    printf("%lld %lld %lld\n", m6_execute(-11),
           m6_execute(0), m6_execute(71));
    printf("%ld %ld %ld\n", m6_for_ever_break(0),
           m6_for_ever_break(5), m6_for_ever_break(17));
    printf("%d %d %d\n", m6_for_ever_return(-11),
           m6_for_ever_return(0), m6_for_ever_return(71));
    printf("%d %d %d\n", m6_for_ever_continue(0),
           m6_for_ever_continue(6), m6_for_ever_continue(9));
    return 0;
}
