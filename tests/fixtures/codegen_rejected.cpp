// ABI rejection cases: the reference backend accepts only signed 64-bit
// parameters and signed 64-bit results. Exactly one routine per translation
// unit keeps diagnostics deterministic.
#if defined(TEST_NARROW_PARAM)
extern "C" long long rejected(long long a, int b) { return a + b; }
#elif defined(TEST_UNSIGNED_RETURN)
extern "C" unsigned long long rejected(unsigned long long a) { return a + 1ULL; }
#elif defined(TEST_FLOAT_RETURN)
extern "C" double rejected(double a) { return a; }
#elif defined(TEST_POINTER_PARAM)
extern "C" long long rejected(long long* p) { return p ? 12LL : 13LL; }
#else
#error Define an unsupported ABI fixture variant
#endif
