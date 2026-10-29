#include <stdio.h>
extern unsigned int m4_bss;
extern unsigned long long m4_initialized;
extern void m4_defined_step(unsigned int amount);
extern unsigned long long m4_defined_value(void);

int main(void)
{
    printf("%u %llu %llu\n", m4_bss, m4_initialized, m4_defined_value());
    m4_defined_step(5U);
    printf("%u %llu %llu\n", m4_bss, m4_initialized, m4_defined_value());
    m4_defined_step(7U);
    printf("%u %llu %llu\n", m4_bss, m4_initialized, m4_defined_value());
    return 0;
}
