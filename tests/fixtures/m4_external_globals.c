/* M4: compile this translation unit with EDG/Cranelift, not GCC.
 * Globals are defined in m4_external_driver.c, which GCC compiles.
 * No pointers, arrays, function calls or signed overflow are needed.
 */
extern unsigned int m4_counter;
extern signed short m4_delta;
extern _Bool m4_enabled;
extern unsigned long long m4_total;

void m4_test(void)
{
    if (m4_enabled) {
        m4_counter += (unsigned int)(unsigned short)m4_delta;
        m4_total ^= (unsigned long long)m4_counter;
        m4_delta = (short)(m4_delta - 3);
    } else {
        m4_counter = 7U;
        m4_total = 0x123456789abcdef0ULL;
    }
    m4_enabled = (m4_counter != 0U);
}
