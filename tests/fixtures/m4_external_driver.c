/* GCC-only driver for EDG/Cranelift external-data relocation test. */
#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>

unsigned int m4_counter = 100U;
signed short m4_delta = -3;
_Bool m4_enabled = 1;
unsigned long long m4_total = 0x8000000000000001ULL;
extern void m4_test(void);

int main(void)
{
    m4_test();
    printf("%u %d %u %llu\n", m4_counter, (int)m4_delta,
           (unsigned)m4_enabled, m4_total);
    m4_enabled = 0;
    m4_test();
    printf("%u %d %u %llu\n", m4_counter, (int)m4_delta,
           (unsigned)m4_enabled, m4_total);
    return 0;
}
