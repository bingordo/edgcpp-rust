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
