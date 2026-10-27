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
