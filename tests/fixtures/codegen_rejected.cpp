// These constructs remain explicitly unsupported by the narrow integer
// reference backend. Do not reject narrow/unsigned mutable locals here:
// they are covered by positive execution tests.
#if defined(TEST_DIVISION)
extern "C" int rejected(int value) { return value / 3; }
#elif defined(TEST_SHIFT)
extern "C" unsigned long long rejected(unsigned long long value) {
    return value >> 2;
}
#elif defined(TEST_FLOAT_RETURN)
extern "C" double rejected(double a) { return a; }
#elif defined(TEST_POINTER_PARAM)
extern "C" long long rejected(long long* p) { return p ? 12LL : 13LL; }
#else
#error Define an unsupported fixture variant
#endif
