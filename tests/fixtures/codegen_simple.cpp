// Integer-only functions for EDG -> Cranelift executable comparison.
// C linkage is required for the subset backend's unmangled exports.
extern "C" long long edg_add(long long a, long long b) {
    return a + b;
}
extern "C" long long edg_affine(long long a, long long b) {
    return a * 7 - b;
}
extern "C" long long edg_constant() {
    return 42;
}

extern "C" long long edg_choose(long long a, long long b) {
    if (a < b) {
        return a + 11;
    } else {
        return b - 13;
    }
}
extern "C" long long edg_absolute(long long a) {
    if (a < 0) {
        return -a;
    } else {
        return a;
    }
}
extern "C" long long edg_compare(long long a, long long b) {
    return (long long)(a >= b) + (long long)(a != b) * 2LL;
}
extern "C" long long edg_bits(long long a, long long b) {
    return (a & b) ^ (a | b);
}

extern "C" long long edg_locals(long long a, long long b) {
    long long tmp = a * 3 + b;
    tmp = tmp ^ (a - b);
    long long total = tmp + a;
    return total * 2 - b;
}

extern "C" long long edg_locals_choose(long long a, long long b) {
    long long intermediate = a * 3 + b;
    if (a < b) {
        return intermediate + a;
    } else {
        return intermediate - b;
    }
}

extern "C" long long edg_mutable_if(long long a, long long b) {
    long long current = a + b;
    if (a < b) {
        current = current * 2;
    } else {
        current = current - a;
    }
    return current + b;
}
extern "C" long long edg_no_else(long long a, long long b) {
    long long current = a + b;
    if (a < b) {
        current = current * 3;
    }
    return current - b;
}
extern "C" long long edg_while(long long a, long long b) {
    long long i = a;
    long long result = b + 1;
    while (i < b) {
        result = result + i;
        i = i + 1;
    }
    return result + i;
}

extern "C" long long edg_for(long long a, long long b) {
    long long i = a;
    long long sum = 2;
    for (i = a; i < b; i = i + 1) {
        sum = sum + i;
    }
    return sum + i;
}
extern "C" long long edg_do_while(long long a, long long b) {
    long long total = b;
    long long i = a;
    do {
        total = total + i;
        i = i + 1;
    } while (i < b);
    return total + i;
}

extern "C" long long edg_for_jumps(long long a, long long b) {
    long long i = a;
    long long sum = b;
    for (i = a; i < b; i = i + 1) {
        if (i == a + 1) continue;
        if (i > a + 3) break;
        sum = sum + i;
    }
    return sum + i;
}
extern "C" long long edg_while_jumps(long long a, long long b) {
    long long i = a;
    long long sum = b;
    while (i < b) {
        i = i + 1;
        if (i == a + 1) continue;
        if (i > a + 3) break;
        sum = sum + i;
    }
    return sum + i;
}
extern "C" long long edg_do_jumps(long long a, long long b) {
    long long i = a;
    long long sum = b;
    do {
        i = i + 1;
        if (i == a + 1) continue;
        if (i > a + 3) break;
        sum = sum + i;
    } while (i < b);
    return sum + i;
}

extern "C" void edg_void_empty(void) {}
extern "C" void edg_void_branch(long long a) {
    if (a < 0) return;
    return;
}
extern "C" signed char edg_i8_identity(signed char a) { return a; }
extern "C" unsigned int edg_u32_identity(unsigned int a) { return a; }
extern "C" short edg_i16_constant(void) { return 123; }

extern "C" unsigned int edg_u32_wrap(unsigned int a) {
    return a + 0xf0000000U;
}
extern "C" signed char edg_i8_promote(signed char a) {
    return (signed char)((int)a + 20);
}
extern "C" unsigned long long edg_u64_order(unsigned long long a, unsigned long long b) {
    return a < b;
}
extern "C" unsigned int edg_u32_order(unsigned int a, unsigned int b) {
    return a >= b;
}
extern "C" unsigned int edg_u32_cast(unsigned long long a) {
    return (unsigned int)a;
}
extern "C" long long edg_signed_convert(unsigned int a) {
    return (long long)(int)a;
}
extern "C" unsigned long long edg_u64_top_constant() {
    return 18446744073709551615ULL;
}

extern "C" unsigned char edg_u8_mutable(unsigned char a) {
    unsigned char value = a;
    value = (unsigned char)(value + 250);
    value = (unsigned char)(value + 10);
    return value;
}
extern "C" short edg_i16_branch(short value, int choose) {
    short local = value;
    if (choose) {
        local = (short)(local + 200);
    } else {
        local = (short)(local - 100);
    }
    return local;
}
extern "C" unsigned int edg_u32_while(unsigned int start, unsigned int count) {
    unsigned int result = start;
    unsigned int i = 0U;
    while (i < count) {
        result = result + 0xf0000000U;
        i = i + 1U;
    }
    return result;
}
extern "C" unsigned short edg_u16_for(unsigned short n) {
    unsigned short sum = 0;
    unsigned short i = 0;
    for (i = 0; i < n; i = (unsigned short)(i + 1)) {
        sum = (unsigned short)(sum + i);
    }
    return sum;
}
extern "C" unsigned char edg_u8_merge(unsigned char a, unsigned char b, int select) {
    unsigned char result = a;
    if (select) {
        result = b;
    }
    return result;
}
