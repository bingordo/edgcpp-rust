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
    return (a >= b) + (a != b) * 2;
}
extern "C" long long edg_bits(long long a, long long b) {
    return (a & b) ^ (a | b);
}
