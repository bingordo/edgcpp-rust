/* Width/sign matrix for naturally aligned qualified automatic integers.
 * Bounds avoid source signed-overflow and implementation-defined conversion. */
#define M12_WIDTH(NAME, TYPE) \
int m12_volatile_##NAME(int seed) { \
    volatile TYPE value=(TYPE)seed; \
    int before=(int)value; \
    value+=(TYPE)2; \
    return before*100+(int)value; \
} \
int m12_atomic_##NAME(int seed) { \
    _Atomic TYPE value=(TYPE)seed; \
    int before=(int)value; \
    value+=(TYPE)2; \
    return before*100+(int)value; \
}
M12_WIDTH(i8, signed char)
M12_WIDTH(u8, unsigned char)
M12_WIDTH(i16, short)
M12_WIDTH(u16, unsigned short)
M12_WIDTH(i32, int)
M12_WIDTH(u32, unsigned int)
M12_WIDTH(i64, long long)
M12_WIDTH(u64, unsigned long long)
