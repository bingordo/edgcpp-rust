// Only genuinely unsupported C cases belong in the negative ABI suite.
// The reference now supports direct-return narrow/unsigned C arithmetic.
#if defined(TEST_NARROW_PARAM)
extern "C" long long rejected(long long a, int b) {
    int mutable_local = b;
    mutable_local = mutable_local + 1;
    return a + mutable_local;
}
#elif defined(TEST_UNSIGNED_RETURN)
extern "C" unsigned long long rejected(unsigned long long a) {
    unsigned long long mutable_local = a;
    mutable_local = mutable_local + 1ULL;
    return mutable_local;
}
#elif defined(TEST_FLOAT_RETURN)
extern "C" double rejected(double a) { return a; }
#elif defined(TEST_POINTER_PARAM)
extern "C" long long rejected(long long* p) { return p ? 12LL : 13LL; }
#else
#error Define an unsupported ABI fixture variant
#endif
