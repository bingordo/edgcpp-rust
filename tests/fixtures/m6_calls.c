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

/* An omitted C for-condition must not synthesize a false exit edge. */
long m6_for_ever_break(int n)
{
    long sum = 0;
    for (;;) {
        if (n <= 0) break;
        sum += n;
        n--;
    }
    return sum;
}

/* C accepts a non-void function when an unconditional loop returns. */
int m6_for_ever_return(int n)
{
    for (;;) {
        return n + 7;
    }
}

/* A continue in for(;;) must still reach the increment expression. */
int m6_for_ever_continue(int limit)
{
    int n = 0;
    for (;; n++) {
        if (n >= limit) break;
        if ((n & 1) != 0) continue;
    }
    return n;
}

/* M9: source labels are not synthetic break/continue targets. */
int m9_goto_forward(int n)
{
    int result = 1;
    if (n < 0) goto negative;
    result += n * 3;
    goto done;
negative:
    result += -n * 7;
done:
    return result;
}

/* Backedge to an earlier C label, with scalar state updated on each trip. */
int m9_goto_backedge(int limit)
{
    int sum = 0;
    int i = 0;
again:
    if (i >= limit) goto done;
    sum += i * 5 + 3;
    i++;
    goto again;
done:
    return sum;
}
