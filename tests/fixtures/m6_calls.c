/* M6: typed direct C calls, forward definitions and external linkage. */
extern short m6_external(short x, unsigned char y);
extern void m6_touch(unsigned int *p);
extern unsigned int m6_zero(void);

int m6_later(int v);
int m6_internal(int v) { return (v * 3) + 2; }
int m6_early(int v) { return m6_later(v + 4); }
int m6_later(int v) { return m6_internal(v) - 1; }

long long m6_execute(int x)
{
    short narrow = m6_external((short)(x - 3), (unsigned char)7);
    unsigned int local = (unsigned int)m6_internal(x);
    m6_touch(&local);
    return (long long)narrow + (long long)m6_early(x) +
           (long long)local + (long long)m6_zero();
}
