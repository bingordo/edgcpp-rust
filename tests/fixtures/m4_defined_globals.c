/* Own storage in the EDG/Cranelift-generated object. */
static signed short m4_private;
unsigned int m4_bss;
unsigned long long m4_initialized = 0x12345678ULL;

void m4_defined_step(unsigned int amount)
{
    m4_bss += amount;
    ++m4_private;
    m4_initialized += (unsigned long long)m4_bss;
}

unsigned long long m4_defined_value(void)
{
    return m4_initialized ^ ((unsigned long long)m4_bss << 8)
        ^ (unsigned long long)(unsigned short)m4_private;
}
