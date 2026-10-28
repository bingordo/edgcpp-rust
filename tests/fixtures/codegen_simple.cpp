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
