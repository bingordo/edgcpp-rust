#include <stdio.h>
#include <stdint.h>

short m6_external(short x, unsigned char y) { return (short)(x * 2 + y); }
void m6_touch(unsigned int *p) { *p ^= 0x5au; }
unsigned int m6_zero(void) { return 19u; }
extern long long m6_execute(int);

int main(void)
{
    printf("%lld %lld %lld\n", m6_execute(-11),
           m6_execute(0), m6_execute(71));
    return 0;
}
