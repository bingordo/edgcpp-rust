/* M12: sequential volatile scalar effects and C11 SC atomic load/store.
 * Globals are defined by the independently GCC-built test driver. */
extern volatile signed char m12_v8;
extern volatile short m12_v16;
extern volatile int m12_v32;
extern volatile long m12_v64;
extern _Atomic int m12_a32;

int m12_v32_read_twice(void) {
    int first = m12_v32;
    int second = m12_v32;
    return first + second;
}
void m12_v32_write_pair(int x) {
    m12_v32 = x;
    m12_v32 = x + 1;
}
int m12_v32_post(void) { return m12_v32++; }
int m12_v32_compound(int n) { return m12_v32 += n; }
long m12_vary_widths(void) {
    m12_v8 = -4;
    m12_v16 = 200;
    m12_v64 = 1234567;
    return m12_v8 + m12_v16 + m12_v64;
}
int m12_atomic_get(void) { return m12_a32; }
void m12_atomic_put(int x) { m12_a32 = x; }
int m12_atomic_two_reads(void) {
    int x = m12_a32;
    int y = m12_a32;
    return x+y;
}
/* The read-modify-write must be one indivisible C11 operation. */
int m12_atomic_post(void) { return m12_a32++; }
int m12_atomic_add(int delta) { return m12_a32 += delta; }
int m12_atomic_dec(void) { return --m12_a32; }
