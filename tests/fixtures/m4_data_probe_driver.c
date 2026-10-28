/* GCC driver owns storage; the generated object must import the symbol. */
#include <stdint.h>
#include <stdio.h>
uint32_t m4_counter = UINT32_MAX;
extern void m4_increment(void);
int main(void)
{
    m4_increment();
    printf("%u\n", m4_counter);
    m4_counter = 41;
    m4_increment();
    printf("%u\n", m4_counter);
    return 0;
}
