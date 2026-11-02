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

/* A forward jump bypasses valid but unreachable C statements. */
int m9_skip_dead(int x)
{
    goto done;
    x += 99;
done:
    return x + 9;
}

/* A source goto may exit a while loop without pretending to be break. */
int m9_goto_while_exit(int n)
{
    int sum = 0;
    while (n > 0) {
        if (n == 3) goto done;
        sum += n * 2;
        n--;
    }
done:
    return sum - n;
}

/* An unconditional for-loop can terminate only through its named label. */
int m9_goto_for_exit(int n)
{
    int sum = 0;
    for (;;) {
        if (n <= 0) goto done;
        sum += n;
        n--;
    }
done:
    return sum;
}

/* M9 switch: source order and fallthrough, not just independent ifs. */
int m9_switch_fallthrough(int x)
{
    int result = 7;
    switch (x) {
    case -2:
        result += 3;
        /* fall through */
    case 0:
        result += 11;
        break;
    case 4:
        result += 23;
        /* fall through */
    default:
        result += 37;
        break;
    }
    return result;
}
int m9_switch_no_default(int x)
{
    int result = 41;
    switch (x) {
    case -1: result += 5; break;
    case 3:  result += 13; break;
    }
    return result;
}
/* The selector must be evaluated only once, even without a match. */
int m9_switch_selector_once(int x)
{
    int n = x;
    int value = 2;
    switch (n++) {
    case -1: value = 5; break;
    case 2: value = 19; break;
    default: value = 37; break;
    }
    return value + 100 * n;
}

/* Nested switch breaks must not escape the outer switch, and the outer
 * switch's case table must not absorb any inner case constants. */
int m9_switch_nested(int x, int y)
{
    int result = 5;
    switch (x) {
    case 1:
        switch (y) {
        case 2:
            result += 7;
            break;
        case 3:
            result += 11;
            /* fall through */
        default:
            result += 13;
            break;
        }
        result += 17;
        break;
    case 4:
        result += 31;
        break;
    default:
        result += 41;
        break;
    }
    return result;
}
/* A default before later cases still falls through in source order. */
int m9_switch_default_middle(int x)
{
    int result = 1;
    switch (x) {
    case 0:
        result += 2;
        break;
    default:
        result += 3;
        /* fall through */
    case 2:
        result += 5;
        break;
    case 3:
        result += 7;
        break;
    }
    return result;
}

/* Every dispatch path returns: no synthetic post-switch return is required. */
int m9_switch_all_return(int x)
{
    switch (x) {
    case -3: return x + 7;
    case 0:  return 17;
    case 4:  return 41;
    default: return x * 5 - 9;
    }
}
int m9_switch_fallthrough_return(int x)
{
    switch (x) {
    case -1:
    case 0:
        return 13;
    case 3:
        if (x > 2) return 29;
        /* otherwise fall through */
    default:
        return 47;
    }
}
