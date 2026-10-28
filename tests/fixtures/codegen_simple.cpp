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

extern "C" bool edg_bool_assign(int value) {
    bool flag = false;
    flag = (bool)value;
    return flag;
}
extern "C" bool edg_bool_branch(int value) {
    bool flag = (bool)value;
    if (value < 0) {
        flag = (bool)(value + 7);
    } else {
        flag = (bool)(value - 1);
    }
    return flag;
}
extern "C" bool edg_bool_identity(bool value) {
    bool local = value;
    return local;
}

extern "C" long long edg_sdiv64(long long a, long long b) { return a / b; }
extern "C" long long edg_srem64(long long a, long long b) { return a % b; }
extern "C" unsigned long long edg_udiv64(unsigned long long a, unsigned long long b) {
    return a / b;
}
extern "C" unsigned long long edg_urem64(unsigned long long a, unsigned long long b) {
    return a % b;
}
extern "C" int edg_sdiv32(int a, int b) { return a / b; }
extern "C" int edg_srem32(int a, int b) { return a % b; }
extern "C" unsigned int edg_udiv32(unsigned int a, unsigned int b) { return a / b; }
extern "C" unsigned int edg_urem32(unsigned int a, unsigned int b) { return a % b; }
extern "C" unsigned long long edg_shl64(unsigned long long a, unsigned int amount) {
    return a << amount;
}
extern "C" unsigned long long edg_ushr64(unsigned long long a, unsigned int amount) {
    return a >> amount;
}
extern "C" long long edg_sshr64(long long a, unsigned int amount) {
    return a >> amount;
}
extern "C" int edg_sshr32(int a, unsigned int amount) {
    return a >> amount;
}
extern "C" unsigned int edg_shl32(unsigned int a, int amount) {
    return a << amount;
}
extern "C" unsigned int edg_udiv_local(unsigned int a, unsigned int b) {
    unsigned int value = a;
    value = value / b;
    value = value % 137U;
    return value;
}
extern "C" unsigned int edg_shift_local(unsigned int a, int amount) {
    unsigned int value = a;
    value = value >> amount;
    value = value << amount;
    return value;
}

extern "C" int edg_short_and(int a) {
    int changed = 0;
    int answer = a && (changed = 7);
    return answer * 100 + changed;
}
extern "C" int edg_short_or(int a) {
    int changed = 0;
    int answer = a || (changed = 5);
    return answer * 100 + changed;
}
extern "C" int edg_conditional_mutation(int a) {
    int x = 2;
    int y = a ? (x += 3) : (x *= 4);
    return x + y * 10;
}
extern "C" int edg_comma_mutation() {
    int x = 1;
    int y = (x += 2, x *= 3);
    return x + y * 10;
}
extern "C" unsigned int edg_postfix_value() {
    unsigned char x = 250;
    unsigned char previous = x++;
    unsigned char before_decrement = x--;
    return previous + (unsigned int)before_decrement * 256U
           + (unsigned int)x * 65536U;
}
extern "C" unsigned short edg_compound_unsigned() {
    unsigned short x = 65532;
    x += 10;
    x ^= 0x00ff;
    x /= 3;
    x <<= 2;
    return x;
}
extern "C" int edg_for_postfix(int n) {
    int i = 0;
    int result = 0;
    for (i = 0; i < n; i++) result += i;
    return result;
}
extern "C" int edg_conditional_side_effect(int a) {
    int x = 0;
    if (a && (x = 3)) x += 2;
    return x;
}
extern "C" int edg_nested_ternary(int a, int b) {
    int x = 5;
    return a ? (b ? (x += 4) : (x -= 2)) : (x *= 3);
}
extern "C" int edg_unary_not_bits(int a) {
    return !a + (~a & 255);
}
